//! Tunnels: saved port forwards per host, plus temporary "open through
//! tunnel" forwards. Each running tunnel is one task that connects (over the
//! host's pooled SSH connection), serves until the connection drops, then
//! reconnects with backoff until it's stopped.
//!
//! Auto-start tunnels start when the UI subscribes (app launch) and again
//! whenever their host gets a new connection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::ssh::client::{ConnectError, Connection};
use nightgrid_core::tunnels::forward::{self, Stats, StatsView};
use nightgrid_core::tunnels::{Tunnel, TunnelId, TunnelInput, TunnelKind};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::term_cmd::connect_host;

const BACKOFF: [u64; 5] = [2, 5, 10, 30, 60];

#[derive(Debug, Clone, Serialize, Type, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TunnelState {
    Stopped,
    Starting,
    Running {
        /// The port actually listening (useful when 0 = "any" was saved).
        bound_port: u16,
    },
    /// Lost or couldn't reach the host; trying again.
    Retrying {
        message: String,
        retry_secs: u32,
    },
    /// Won't retry on its own: port in use, server refused, or the host needs
    /// a login (open a terminal on it once, then start again).
    Failed {
        message: String,
        needs_login: bool,
    },
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TunnelEvent {
    State {
        id: TunnelId,
        state: TunnelState,
    },
    Stats {
        stats: Vec<TunnelStats>,
    },
    /// Temporary tunnels were added or removed: reload the list.
    Changed,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStats {
    pub id: TunnelId,
    pub stats: StatsView,
}

/// A tunnel as the UI lists it: saved ones, then temporary ones (negative ids).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TunnelView {
    #[serde(flatten)]
    pub tunnel: Tunnel,
    pub temporary: bool,
    pub spec: String,
    pub public: bool,
    pub state: TunnelState,
}

struct Running {
    task: tauri::async_runtime::JoinHandle<()>,
    stats: Arc<Stats>,
}

#[derive(Default)]
pub struct TunnelHub {
    channel: Mutex<Option<Channel<TunnelEvent>>>,
    running: Mutex<HashMap<TunnelId, Running>>,
    states: Mutex<HashMap<TunnelId, TunnelState>>,
    /// Temporary forwards (not saved), by negative id.
    temporary: Mutex<HashMap<TunnelId, Tunnel>>,
    next_temp: AtomicI32,
    /// Last connection seen per host, to auto-start once per new connection.
    seen: Mutex<HashMap<HostId, usize>>,
    ticker: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    autostarted: Mutex<bool>,
}

impl TunnelHub {
    fn send(&self, ev: TunnelEvent) {
        if let Some(ch) = self.channel.lock().expect("poisoned").as_ref() {
            let _ = ch.send(ev);
        }
    }

    fn set_state(&self, id: TunnelId, state: TunnelState) {
        self.states.lock().expect("poisoned").insert(id, state.clone());
        self.send(TunnelEvent::State { id, state });
    }

    fn state(&self, id: TunnelId) -> TunnelState {
        self.states.lock().expect("poisoned").get(&id).cloned().unwrap_or(TunnelState::Stopped)
    }

    fn is_running(&self, id: TunnelId) -> bool {
        self.running.lock().expect("poisoned").get(&id).is_some_and(|r| !r.task.inner().is_finished())
    }

    fn stop(&self, id: TunnelId) {
        if let Some(r) = self.running.lock().expect("poisoned").remove(&id) {
            r.task.abort();
        }
        self.set_state(id, TunnelState::Stopped);
    }
}

fn hub(app: &AppHandle) -> Arc<TunnelHub> {
    app.state::<AppState>().tunnels.clone()
}

fn find(app: &AppHandle, id: TunnelId) -> CmdResult<Tunnel> {
    if id < 0 {
        return hub(app)
            .temporary
            .lock()
            .expect("poisoned")
            .get(&id)
            .cloned()
            .ok_or_else(|| CommandError::msg("tunnel not found"));
    }
    Ok(app.state::<AppState>().store().get_tunnel(id)?)
}

fn needs_login(e: &ConnectError) -> bool {
    matches!(
        e,
        ConnectError::PasswordRequired { .. }
            | ConnectError::PassphraseRequired { .. }
            | ConnectError::HostKeyUnknown { .. }
            | ConnectError::HostKeyChanged { .. }
    )
}

/// Serves one connection's worth of the tunnel. `Ok(reason)` = the connection
/// ended (retry), `Err(message)` = a problem retrying won't fix.
async fn serve_once(hub: &TunnelHub, t: &Tunnel, conn: Arc<Connection>, stats: Arc<Stats>) -> Result<String, String> {
    match t.kind {
        TunnelKind::Local | TunnelKind::Dynamic => {
            let listener = forward::listen(&t.bind_addr, t.bind_port).await?;
            let port = listener.local_addr().map(|a| a.port()).unwrap_or(t.bind_port);
            hub.set_state(t.id, TunnelState::Running { bound_port: port });
            tracing::info!(tunnel = t.id, spec = %t.spec(), port, "tunnel up");
            Ok(if t.kind == TunnelKind::Local {
                forward::serve_local(conn, listener, t.dest_host.clone(), t.dest_port, stats).await
            } else {
                forward::serve_dynamic(conn, listener, stats).await
            })
        }
        TunnelKind::Remote => {
            let fwd = forward::request_remote(conn, &t.bind_addr, t.bind_port).await?;
            let port = u16::try_from(fwd.port).unwrap_or(t.bind_port);
            hub.set_state(t.id, TunnelState::Running { bound_port: port });
            tracing::info!(tunnel = t.id, spec = %t.spec(), port, "tunnel up");
            Ok(forward::serve_remote(fwd, t.dest_host.clone(), t.dest_port, stats).await)
        }
    }
}

fn start(app: &AppHandle, t: Tunnel) {
    let hub = hub(app);
    if hub.is_running(t.id) {
        return;
    }
    let stats = Arc::new(Stats::default());
    let app2 = app.clone();
    let hub2 = hub.clone();
    let stats2 = stats.clone();
    let id = t.id;
    hub.set_state(id, TunnelState::Starting);
    let task = tauri::async_runtime::spawn(async move {
        let mut attempt = 0usize;
        loop {
            let reason = match connect_host(&app2, t.host_id).await {
                Ok(conn) => match serve_once(&hub2, &t, conn, stats2.clone()).await {
                    Ok(reason) => {
                        attempt = 0;
                        reason
                    }
                    Err(message) => {
                        tracing::warn!(tunnel = t.id, "tunnel failed: {message}");
                        hub2.set_state(t.id, TunnelState::Failed { message, needs_login: false });
                        return;
                    }
                },
                Err(e) if needs_login(&e) => {
                    hub2.set_state(
                        t.id,
                        TunnelState::Failed {
                            message: format!(
                                "{e}. Open a terminal on the host once to log in, then start the tunnel again."
                            ),
                            needs_login: true,
                        },
                    );
                    return;
                }
                Err(e) => e.to_string(),
            };
            let wait = BACKOFF[attempt.min(BACKOFF.len() - 1)];
            attempt += 1;
            hub2.set_state(t.id, TunnelState::Retrying { message: reason, retry_secs: wait as u32 });
            tokio::time::sleep(Duration::from_secs(wait)).await;
            hub2.set_state(t.id, TunnelState::Starting);
        }
    });
    hub.running.lock().expect("poisoned").insert(id, Running { task, stats });
    start_ticker(app);
}

/// Sends traffic counters once a second while anything is running.
fn start_ticker(app: &AppHandle) {
    let hub = hub(app);
    let mut ticker = hub.ticker.lock().expect("poisoned");
    if ticker.as_ref().is_some_and(|t| !t.inner().is_finished()) {
        return;
    }
    let hub2 = hub.clone();
    *ticker = Some(tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let stats: Vec<TunnelStats> = {
                let mut running = hub2.running.lock().expect("poisoned");
                running.retain(|_, r| !r.task.inner().is_finished());
                running.iter().map(|(id, r)| TunnelStats { id: *id, stats: r.stats.view() }).collect()
            };
            if stats.is_empty() {
                break;
            }
            hub2.send(TunnelEvent::Stats { stats });
        }
    }));
}

/// Called after `connect_host` succeeds: starts the host's auto-start
/// tunnels once per new connection.
pub(crate) fn host_connected(app: &AppHandle, host_id: HostId, conn: &Arc<Connection>) {
    let hub = hub(app);
    let ptr = Arc::as_ptr(conn) as usize;
    if hub.seen.lock().expect("poisoned").insert(host_id, ptr) == Some(ptr) {
        return;
    }
    let tunnels = {
        let state = app.state::<AppState>();
        let store = state.store();
        if !AppSettings::load(&store).unwrap_or_default().feature("tunnels") {
            return;
        }
        store.list_tunnels().unwrap_or_default()
    };
    for t in tunnels.into_iter().filter(|t| t.host_id == host_id && t.auto_start) {
        if !hub.is_running(t.id) {
            start(app, t);
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn tunnels_subscribe(app: AppHandle, state: State<'_, AppState>, on_event: Channel<TunnelEvent>) -> CmdResult<()> {
    let hub = state.tunnels.clone();
    for (id, s) in hub.states.lock().expect("poisoned").iter() {
        let _ = on_event.send(TunnelEvent::State { id: *id, state: s.clone() });
    }
    *hub.channel.lock().expect("poisoned") = Some(on_event);
    let first = !std::mem::replace(&mut *hub.autostarted.lock().expect("poisoned"), true);
    if first {
        let autos: Vec<Tunnel> = state.store().list_tunnels()?.into_iter().filter(|t| t.auto_start).collect();
        for t in autos {
            start(&app, t);
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn tunnels_list(state: State<'_, AppState>) -> CmdResult<Vec<TunnelView>> {
    let hub = state.tunnels.clone();
    let view = |t: Tunnel, temporary: bool| TunnelView {
        spec: t.spec(),
        public: nightgrid_core::tunnels::is_public_bind(&t.bind_addr),
        state: hub.state(t.id),
        temporary,
        tunnel: t,
    };
    let mut out: Vec<TunnelView> = state.store().list_tunnels()?.into_iter().map(|t| view(t, false)).collect();
    let mut temp: Vec<Tunnel> = hub.temporary.lock().expect("poisoned").values().cloned().collect();
    temp.sort_by_key(|t| -t.id);
    out.extend(temp.into_iter().map(|t| view(t, true)));
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub fn tunnel_create(
    app: AppHandle,
    state: State<'_, AppState>,
    input: TunnelInput,
    start_now: bool,
) -> CmdResult<Tunnel> {
    let t = state.store().create_tunnel(&input)?;
    if start_now {
        start(&app, t.clone());
    }
    Ok(t)
}

/// Saves changes; a running tunnel restarts with them.
#[tauri::command]
#[specta::specta]
pub fn tunnel_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: TunnelId,
    input: TunnelInput,
) -> CmdResult<Tunnel> {
    let t = state.store().update_tunnel(id, &input)?;
    if state.tunnels.is_running(id) {
        state.tunnels.stop(id);
        start(&app, t.clone());
    }
    Ok(t)
}

#[tauri::command]
#[specta::specta]
pub fn tunnel_delete(state: State<'_, AppState>, id: TunnelId) -> CmdResult<()> {
    state.tunnels.stop(id);
    state.tunnels.states.lock().expect("poisoned").remove(&id);
    if id < 0 {
        state.tunnels.temporary.lock().expect("poisoned").remove(&id);
        state.tunnels.send(TunnelEvent::Changed);
        return Ok(());
    }
    state.store().delete_tunnel(id)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn tunnel_start(app: AppHandle, id: TunnelId) -> CmdResult<()> {
    let t = find(&app, id)?;
    start(&app, t);
    Ok(())
}

/// Stops a tunnel. Temporary ones are removed.
#[tauri::command]
#[specta::specta]
pub fn tunnel_stop(state: State<'_, AppState>, id: TunnelId) -> CmdResult<()> {
    state.tunnels.stop(id);
    if id < 0 {
        state.tunnels.states.lock().expect("poisoned").remove(&id);
        state.tunnels.temporary.lock().expect("poisoned").remove(&id);
        state.tunnels.send(TunnelEvent::Changed);
    }
    Ok(())
}

/// "Open through tunnel": forwards `dest_host:dest_port` (as the host sees
/// it) to a free local port and returns that port once it's listening. An
/// existing temporary tunnel to the same place is reused.
#[tauri::command]
#[specta::specta]
pub async fn tunnel_open_temporary(
    app: AppHandle,
    host_id: HostId,
    dest_host: String,
    dest_port: u16,
) -> CmdResult<u16> {
    let hub = hub(&app);
    let existing = hub
        .temporary
        .lock()
        .expect("poisoned")
        .values()
        .find(|t| t.host_id == host_id && t.dest_host == dest_host && t.dest_port == dest_port)
        .map(|t| t.id);
    let id = match existing {
        Some(id) => {
            if !hub.is_running(id) {
                start(&app, find(&app, id)?);
            }
            id
        }
        None => {
            // Validate through the same rules as saved tunnels.
            let input = TunnelInput {
                host_id,
                name: format!("{dest_host}:{dest_port}"),
                kind: TunnelKind::Local,
                bind_addr: "127.0.0.1".into(),
                bind_port: 0,
                dest_host,
                dest_port,
                auto_start: false,
            };
            let n = nightgrid_core::tunnels::normalize(&input)?;
            let id = -(hub.next_temp.fetch_add(1, Ordering::Relaxed) + 1);
            let t = Tunnel {
                id,
                host_id,
                name: n.name,
                kind: n.kind,
                bind_addr: n.bind_addr,
                bind_port: n.bind_port,
                dest_host: n.dest_host,
                dest_port: n.dest_port,
                auto_start: false,
            };
            hub.temporary.lock().expect("poisoned").insert(id, t.clone());
            hub.send(TunnelEvent::Changed);
            start(&app, t);
            id
        }
    };
    // Wait for it to listen (or fail).
    for _ in 0..150 {
        match hub.state(id) {
            TunnelState::Running { bound_port } => return Ok(bound_port),
            TunnelState::Failed { message, .. } => return Err(CommandError::msg(message)),
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    Err(CommandError::msg("the tunnel didn't come up in time; see Tunnels"))
}
