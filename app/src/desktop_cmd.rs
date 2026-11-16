//! VM desktops (M10n): the built-in VNC viewer and RDP tunnels.
//!
//! The VNC bytes ride the host's pooled SSH connection (a `direct-tcpip`
//! channel to the guest's loopback VNC port, a Unix-socket channel, or
//! `qm vncproxy` on an exec channel), so no port opens on either side.
//! Pixels are decoded here; the UI long-polls [`desktop_frame`] for binary
//! patches of what changed and draws them on a canvas.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::system::sudo::{self, SudoAuth};
use nightgrid_core::system::vms::{valid_id, VmKind};
use nightgrid_core::system::SystemError;
use nightgrid_core::tunnels::forward;
use nightgrid_core::vnc;
use nightgrid_core::vnc::rfb::{self, Shared, VncEvent, VncInput};
use nightgrid_core::vnc::target::{self, GuestAddrs};
use serde::Serialize;
use specta::Type;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, Manager as _, State};
use tauri_plugin_opener::OpenerExt as _;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_host;

struct Session {
    input: mpsc::Sender<VncInput>,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

struct Rdp {
    info: RdpTunnel,
    task: JoinHandle<()>,
}

/// An RDP tunnel: 127.0.0.1:`local_port` → the guest's RDP port via the host.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RdpTunnel {
    pub id: u32,
    pub host_id: HostId,
    pub guest: String,
    pub addr: String,
    pub local_port: u16,
}

#[derive(Default)]
pub struct Desktops {
    sessions: Mutex<HashMap<u32, Session>>,
    rdp: Mutex<HashMap<u32, Rdp>>,
    next: AtomicU32,
}

impl Desktops {
    fn id(&self) -> u32 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }
}

fn err(msg: impl std::fmt::Display) -> crate::system_cmd::SystemCmdError {
    SystemError::other(msg).into()
}

/// Opens a guest's screen. `elevate` asks virsh as root (needed when this
/// login isn't in the libvirt group); Proxmox always needs root.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn desktop_open(
    app: AppHandle,
    desktops: State<'_, Desktops>,
    host_id: HostId,
    kind: VmKind,
    id: String,
    elevate: bool,
    sudo: Option<SudoInput>,
    on_event: Channel<VncEvent>,
) -> SysResult<u32> {
    if !valid_id(kind, &id) {
        return Err(err(format!("invalid guest id {id:?}")));
    }
    if !target::has_display(kind) {
        return Err(err("containers have no screen: open a console instead"));
    }
    let conn = connect_host(&app, host_id).await?;
    let auth =
        if elevate || kind == VmKind::Qemu { resolve_sudo(&app, host_id, &conn, sudo).await? } else { SudoAuth::User };

    let opened = vnc::open(&conn, &auth, kind, &id).await?;

    let sid = desktops.id();
    let shared = Arc::new(Shared::default());
    let (tx, rx) = mpsc::channel(512);
    let events = on_event.clone();
    let task = {
        let shared = shared.clone();
        tokio::spawn(async move {
            let r = rfb::run(opened.stream, opened.password, shared, rx, move |e| {
                let _ = events.send(e);
            })
            .await;
            let reason = match r {
                Ok(()) => String::new(),
                Err(e) if kind == VmKind::Qemu && e.contains("closed") => {
                    format!("{e} (is the VM running, and may this login run qm as root?)")
                }
                Err(e) => e,
            };
            let _ = on_event.send(VncEvent::Closed { reason });
        })
    };
    tracing::info!(host = %host_id, guest = %id, ?kind, session = sid, "desktop opened");
    desktops.sessions.lock().expect("desktops").insert(sid, Session { input: tx, shared, task });
    Ok(sid)
}

/// The next patch of changed pixels (binary, see `Frame::take_patch`), waiting
/// up to a second for one. Empty when nothing changed or the session ended.
/// Not exported through specta: it returns raw bytes.
#[tauri::command]
pub async fn desktop_frame(desktops: State<'_, Desktops>, session: u32) -> Result<Response, String> {
    let Some(shared) = desktops.sessions.lock().expect("desktops").get(&session).map(|s| s.shared.clone()) else {
        return Ok(Response::new(Vec::new()));
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        let notified = shared.changed.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if let Some(p) = shared.frame.lock().expect("frame").take_patch() {
            return Ok(Response::new(p));
        }
        if shared.is_closed() {
            return Ok(Response::new(Vec::new()));
        }
        if tokio::time::timeout_at(deadline, notified).await.is_err() {
            return Ok(Response::new(Vec::new()));
        }
    }
}

/// Keyboard, pointer and clipboard events for the guest.
#[tauri::command]
#[specta::specta]
pub async fn desktop_input(desktops: State<'_, Desktops>, session: u32, events: Vec<VncInput>) -> Result<(), String> {
    let Some(tx) = desktops.sessions.lock().expect("desktops").get(&session).map(|s| s.input.clone()) else {
        return Err("this desktop is closed".into());
    };
    for e in events {
        tx.send(e).await.map_err(|_| "this desktop is closed".to_string())?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn desktop_close(desktops: State<'_, Desktops>, session: u32) {
    if let Some(s) = desktops.sessions.lock().expect("desktops").remove(&session) {
        s.shared.close();
        drop(s.input);
        let task = s.task;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            task.abort();
        });
    }
}

/// The guest's IP addresses (DHCP leases, guest agent), for RDP.
#[tauri::command]
#[specta::specta]
pub async fn desktop_guest_addrs(
    app: AppHandle,
    host_id: HostId,
    kind: VmKind,
    id: String,
    elevate: bool,
    sudo: Option<SudoInput>,
) -> SysResult<GuestAddrs> {
    let cmd = target::guest_addr_command(kind, &id)?;
    let conn = connect_host(&app, host_id).await?;
    let auth = if elevate || kind != VmKind::Libvirt {
        resolve_sudo(&app, host_id, &conn, sudo).await?
    } else {
        SudoAuth::User
    };
    let out = sudo::run(&conn, &auth, &cmd).await?;
    Ok(target::parse_guest_addrs(kind, &out.stdout))
}

/// Starts a tunnel from 127.0.0.1 (a free port) to `addr`:`port` through the
/// host, writes a `.rdp` file for it (no credentials in it) and opens that
/// with the system's RDP client.
#[tauri::command]
#[specta::specta]
pub async fn desktop_rdp_open(
    app: AppHandle,
    desktops: State<'_, Desktops>,
    host_id: HostId,
    guest: String,
    addr: String,
    port: Option<u16>,
) -> SysResult<RdpTunnel> {
    let addr = addr.trim().to_string();
    if !target::valid_addr(&addr) {
        return Err(err(format!("invalid address {addr:?}")));
    }
    let port = port.unwrap_or(3389);
    let conn = connect_host(&app, host_id).await?;
    let listener = forward::listen("127.0.0.1", 0).await.map_err(err)?;
    let local_port = listener.local_addr().map_err(err)?.port();
    let id = desktops.id();
    let info = RdpTunnel { id, host_id, guest: guest.chars().take(80).collect(), addr: addr.clone(), local_port };
    let task = tokio::spawn(async move {
        let why = forward::serve_local(conn, listener, addr, port, Arc::new(forward::Stats::default())).await;
        tracing::info!(tunnel = id, "rdp tunnel ended: {why}");
    });
    desktops.rdp.lock().expect("rdp").insert(id, Rdp { info: info.clone(), task });

    let dir = app.path().app_cache_dir().map_err(err)?.join("rdp");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let file = dir.join(format!("nightgrid-{local_port}.rdp"));
    std::fs::write(&file, target::rdp_file(local_port)).map_err(err)?;
    if let Err(e) = app.opener().open_path(file.to_string_lossy(), None::<&str>) {
        tracing::warn!("couldn't open the .rdp file: {e}");
        return Err(err(format!(
            "the tunnel is up on 127.0.0.1:{local_port}, but no RDP client opened ({e}); point one at that address"
        )));
    }
    tracing::info!(host = %host_id, tunnel = id, local_port, "rdp tunnel opened");
    Ok(info)
}

#[tauri::command]
#[specta::specta]
pub fn desktop_rdp_list(desktops: State<'_, Desktops>) -> Vec<RdpTunnel> {
    let mut v: Vec<RdpTunnel> =
        desktops.rdp.lock().expect("rdp").values().filter(|r| !r.task.is_finished()).map(|r| r.info.clone()).collect();
    v.sort_by_key(|r| r.id);
    v
}

#[tauri::command]
#[specta::specta]
pub fn desktop_rdp_close(desktops: State<'_, Desktops>, id: u32) {
    if let Some(r) = desktops.rdp.lock().expect("rdp").remove(&id) {
        r.task.abort();
    }
}
