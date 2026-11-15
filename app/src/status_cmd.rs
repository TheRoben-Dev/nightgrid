//! Phone status page and static dashboard export (M10h).
//!
//! The status page is a tiny read-only HTTP server in the app: off by
//! default, bound to this machine's LAN or Tailscale address (or every
//! interface when asked), and locked by a random token kept in the OS
//! keychain. It serves one page (see `nightgrid_core::status_page`) and
//! nothing else: no actions, no API, no assets. Wrong tokens are counted per
//! client address and a client that keeps guessing gets locked out for a while.
//!
//! The dashboard export writes the same page, without the refresh, to a file
//! picked with a native save dialog.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nightgrid_core::alerts::Severity;
use nightgrid_core::secrets::{Secret, SecretKind, SecretRef, SecretStore};
use nightgrid_core::settings::{AppSettings, StatusPageSettings};
use nightgrid_core::status_page::{
    self, render_html, response, route, token_matches, AlertLine, HostStatus, RenderOpts, Route, ServiceLine,
    StatusSnapshot,
};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

use crate::commands::{CmdResult, CommandError};
use crate::metrics_cmd::PollState;
use crate::state::AppState;

/// Seconds between page refreshes on the phone.
const REFRESH_SECS: u32 = 30;
/// Most connections handled at once.
const MAX_CONN: usize = 16;
/// Wrong tokens from one address before it's locked out.
const BAD_MAX: u32 = 10;
const LOCKOUT: Duration = Duration::from_secs(600);
const HEAD_MAX: usize = 8 * 1024;

fn token_ref() -> SecretRef {
    SecretRef::new(SecretKind::IntegrationToken, "status-page")
}

#[derive(Default)]
pub struct StatusServer {
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// Where it listens (`None` when off) and why it couldn't start.
    bound: Mutex<(Vec<SocketAddr>, Option<String>)>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StatusPageInfo {
    pub settings: StatusPageSettings,
    pub running: bool,
    /// Page links with the token in them (masked in the UI until revealed).
    pub urls: Vec<String>,
    pub error: Option<String>,
}

/// This machine's address on the default route (no packet is sent).
fn route_addr(probe: Ipv4Addr, port: u16) -> Option<Ipv4Addr> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    s.connect((probe, port)).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v) if !v.is_loopback() && !v.is_unspecified() => Some(v),
        _ => None,
    }
}

fn lan_addr() -> Option<Ipv4Addr> {
    // TEST-NET-1: never routed anywhere, but picks the default interface.
    route_addr(Ipv4Addr::new(192, 0, 2, 1), 9).filter(|v| !is_tailscale(*v))
}

fn tailscale_addr() -> Option<Ipv4Addr> {
    // Tailscale's MagicDNS address only routes over the tailnet interface.
    route_addr(Ipv4Addr::new(100, 100, 100, 100), 53).filter(|v| is_tailscale(*v))
}

/// 100.64.0.0/10 (CGNAT range Tailscale uses).
fn is_tailscale(v: Ipv4Addr) -> bool {
    let o = v.octets();
    o[0] == 100 && (64..128).contains(&o[1])
}

fn bind_addrs(s: &StatusPageSettings) -> Result<Vec<IpAddr>, String> {
    match s.bind.as_str() {
        "lan" => lan_addr().map(|a| vec![IpAddr::V4(a)]).ok_or_else(|| "No LAN address found on this machine.".into()),
        "tailscale" => tailscale_addr()
            .map(|a| vec![IpAddr::V4(a)])
            .ok_or_else(|| "Tailscale doesn't look connected on this machine (no 100.x address).".into()),
        _ => Ok(vec![IpAddr::V4(Ipv4Addr::UNSPECIFIED)]),
    }
}

/// Addresses a phone can use for a listener.
fn reachable(addr: SocketAddr) -> Vec<SocketAddr> {
    if addr.ip().is_unspecified() {
        [lan_addr(), tailscale_addr()]
            .into_iter()
            .flatten()
            .map(|a| SocketAddr::new(IpAddr::V4(a), addr.port()))
            .collect()
    } else {
        vec![addr]
    }
}

fn token(state: &AppState, create: bool) -> CmdResult<Option<String>> {
    match state.keychain.get(&token_ref()) {
        Ok(Some(t)) if !t.expose().is_empty() => Ok(Some(t.expose().to_owned())),
        Ok(_) if create => {
            let t = status_page::new_token();
            state.keychain.set(&token_ref(), &Secret::new(t.clone())).map_err(|e| CommandError::msg(e.to_string()))?;
            Ok(Some(t))
        }
        Ok(_) => Ok(None),
        Err(e) => Err(CommandError::msg(format!("Keychain: {e}"))),
    }
}

fn info(app: &AppHandle) -> CmdResult<StatusPageInfo> {
    let state = app.state::<AppState>();
    let settings = AppSettings::load(&state.store())?.status_page;
    let (bound, error) = state.status_server.bound.lock().expect("poisoned").clone();
    let running = !bound.is_empty();
    let urls = if running {
        let t = token(&state, false)?.unwrap_or_default();
        bound.iter().flat_map(|a| reachable(*a)).map(|a| format!("http://{a}/s/{t}")).collect()
    } else {
        Vec::new()
    };
    Ok(StatusPageInfo { settings, running, urls, error })
}

/// Starts (or restarts, or stops) the server to match the settings.
pub fn apply(app: &AppHandle) {
    let state = app.state::<AppState>();
    let srv = state.status_server.clone();
    if let Some(t) = srv.task.lock().expect("poisoned").take() {
        t.abort();
    }
    *srv.bound.lock().expect("poisoned") = (Vec::new(), None);
    let settings = match AppSettings::load(&state.store()) {
        Ok(s) => s.status_page,
        Err(_) => return,
    };
    if !settings.enabled {
        return;
    }
    let expected = match token(&state, true) {
        Ok(Some(t)) => t,
        Ok(None) => return,
        Err(e) => {
            srv.bound.lock().expect("poisoned").1 = Some(e.message);
            return;
        }
    };
    let ips = match bind_addrs(&settings) {
        Ok(a) => a,
        Err(e) => {
            srv.bound.lock().expect("poisoned").1 = Some(e);
            return;
        }
    };
    let app2 = app.clone();
    let srv2 = srv.clone();
    let task = tauri::async_runtime::spawn(async move {
        let mut listeners = Vec::new();
        for ip in ips {
            match TcpListener::bind((ip, settings.port)).await {
                Ok(l) => listeners.push(l),
                Err(e) => {
                    srv2.bound.lock().expect("poisoned").1 =
                        Some(format!("Can't listen on {ip}:{}: {e}", settings.port));
                    return;
                }
            }
        }
        let addrs: Vec<SocketAddr> = listeners.iter().filter_map(|l| l.local_addr().ok()).collect();
        tracing::info!(?addrs, "status page listening");
        *srv2.bound.lock().expect("poisoned") = (addrs, None);
        let shared =
            Arc::new(Shared { app: app2, token: expected, bad: Mutex::default(), slots: Semaphore::new(MAX_CONN) });
        let mut tasks = Vec::new();
        for l in listeners {
            let sh = shared.clone();
            tasks.push(tokio::spawn(async move {
                loop {
                    let Ok((sock, peer)) = l.accept().await else { continue };
                    let sh = sh.clone();
                    tokio::spawn(async move {
                        let Ok(_slot) = sh.slots.try_acquire() else { return };
                        let _ = tokio::time::timeout(Duration::from_secs(10), serve(&sh, sock, peer.ip())).await;
                    });
                }
            }));
        }
        // Aborting this task drops the guard, which stops the accept loops.
        struct AbortAll(Vec<tokio::task::JoinHandle<()>>);
        impl Drop for AbortAll {
            fn drop(&mut self) {
                for t in &self.0 {
                    t.abort();
                }
            }
        }
        let _guard = AbortAll(tasks);
        std::future::pending::<()>().await;
    });
    *srv.task.lock().expect("poisoned") = Some(task);
}

struct Shared {
    app: AppHandle,
    token: String,
    /// Wrong tokens per client: count and when the count started.
    bad: Mutex<HashMap<IpAddr, (u32, Instant)>>,
    slots: Semaphore,
}

impl Shared {
    fn locked_out(&self, ip: IpAddr) -> bool {
        let mut bad = self.bad.lock().expect("poisoned");
        bad.retain(|_, (_, t)| t.elapsed() < LOCKOUT);
        bad.get(&ip).is_some_and(|(n, _)| *n >= BAD_MAX)
    }

    fn strike(&self, ip: IpAddr) {
        let mut bad = self.bad.lock().expect("poisoned");
        if bad.len() > 4096 {
            bad.clear();
        }
        let e = bad.entry(ip).or_insert((0, Instant::now()));
        e.0 += 1;
        if e.0 == BAD_MAX {
            tracing::warn!(%ip, "status page: too many wrong tokens, locked out for 10 minutes");
        }
    }
}

async fn serve(sh: &Shared, mut sock: TcpStream, ip: IpAddr) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < HEAD_MAX {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let head_only = head.starts_with("HEAD ");
    let out = if sh.locked_out(ip) {
        response(
            "429 Too Many Requests",
            "text/plain; charset=utf-8",
            "Too many wrong links. Try again later.\n",
            head_only,
        )
    } else {
        match route(&head) {
            Route::Page { token } if token_matches(&token, &sh.token) => {
                let snap = snapshot(&sh.app, false);
                let html =
                    render_html(&snap, &RenderOpts { refresh_secs: Some(REFRESH_SECS), utc_offset_min: utc_offset() });
                response("200 OK", "text/html; charset=utf-8", &html, head_only)
            }
            Route::Page { .. } => {
                sh.strike(ip);
                response("404 Not Found", "text/plain; charset=utf-8", "Not found.\n", head_only)
            }
            Route::Favicon => response("404 Not Found", "text/plain; charset=utf-8", "", head_only),
            Route::BadMethod => {
                response("405 Method Not Allowed", "text/plain; charset=utf-8", "Read-only.\n", head_only)
            }
            Route::NotFound => response("404 Not Found", "text/plain; charset=utf-8", "Not found.\n", head_only),
        }
    };
    sock.write_all(&out).await?;
    sock.shutdown().await
}

fn utc_offset() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}

/// Hosts, active alerts and service checks as the app sees them right now.
pub fn snapshot(app: &AppHandle, with_addresses: bool) -> StatusSnapshot {
    let state = app.state::<AppState>();
    let (hosts, services) = {
        let store = state.store();
        (store.list_hosts().unwrap_or_default(), store.list_services().unwrap_or_default())
    };
    let polls = state.metrics.poll_states();
    let hosts = hosts
        .into_iter()
        .map(|h| {
            let sample = state.metrics.latest(h.id);
            let st = match polls.get(&h.id).map(|p| &p.0) {
                Some(PollState::Ok) => "ok",
                Some(PollState::Connecting) if sample.is_some() => "ok",
                Some(PollState::Offline) => "offline",
                Some(PollState::NeedsLogin) => "auth",
                Some(PollState::Suspended | PollState::Rebooting | PollState::ShuttingDown | PollState::Asleep) => {
                    "sleep"
                }
                _ => "off",
            };
            let mut hs = HostStatus {
                name: h.name.clone(),
                address: with_addresses.then(|| h.hostname.clone()),
                state: st.into(),
                cpu: None,
                mem: None,
                disk: None,
                temp: None,
                uptime_secs: None,
            };
            if let (Some(s), "ok") = (sample, st) {
                hs.cpu = s.cpu;
                hs.mem = (s.mem_total > 0).then(|| s.mem_used as f32 * 100.0 / s.mem_total as f32);
                hs.disk = s
                    .mounts
                    .iter()
                    .filter(|m| m.total > 0)
                    .map(|m| (m.used as f32 * 100.0 / m.total as f32, m.mount.clone()))
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                hs.temp = s.cpu_temp;
                hs.uptime_secs = Some(s.uptime_secs);
            }
            hs
        })
        .collect();
    let alerts = state
        .alerts
        .active()
        .into_iter()
        .map(|a| AlertLine {
            critical: a.severity == Severity::Critical,
            label: a.kind.label().into(),
            title: a.title,
            detail: a.detail,
            since: a.since as f64,
        })
        .collect();
    let latest = state.services.latest();
    let services = services
        .into_iter()
        .filter(|s| s.check_enabled)
        .map(|s| {
            let r = latest.get(&s.id);
            ServiceLine {
                name: s.name,
                state: match r {
                    Some(r) if r.result.up => "up",
                    Some(_) => "down",
                    None => "unknown",
                }
                .into(),
                latency_ms: r.and_then(|r| r.result.latency_ms),
            }
        })
        .collect();
    StatusSnapshot {
        generated_at: nightgrid_core::services::check::now_unix() as f64,
        app_version: app.package_info().version.to_string(),
        hosts,
        alerts,
        services,
    }
}

#[tauri::command]
#[specta::specta]
pub fn status_page_get(app: AppHandle) -> CmdResult<StatusPageInfo> {
    info(&app)
}

/// Saves the status page settings and restarts the server to match.
#[tauri::command]
#[specta::specta]
pub async fn status_page_set(app: AppHandle, settings: StatusPageSettings) -> CmdResult<StatusPageInfo> {
    {
        let state = app.state::<AppState>();
        let store = state.store();
        let mut s = AppSettings::load(&store)?;
        s.status_page = settings;
        s.validate()?;
        s.save(&store)?;
    }
    apply(&app);
    // Give the listener a moment to bind so the answer has the links or the error.
    for _ in 0..20 {
        let done = {
            let st = app.state::<AppState>();
            let b = st.status_server.bound.lock().expect("poisoned");
            !b.0.is_empty() || b.1.is_some() || st.status_server.task.lock().expect("poisoned").is_none()
        };
        if done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    info(&app)
}

/// A new token: old links stop working.
#[tauri::command]
#[specta::specta]
pub async fn status_page_new_token(app: AppHandle) -> CmdResult<StatusPageInfo> {
    {
        let state = app.state::<AppState>();
        state
            .keychain
            .set(&token_ref(), &Secret::new(status_page::new_token()))
            .map_err(|e| CommandError::msg(e.to_string()))?;
    }
    let enabled = AppSettings::load(&app.state::<AppState>().store())?.status_page.enabled;
    if enabled {
        let s = AppSettings::load(&app.state::<AppState>().store())?.status_page;
        return status_page_set(app, s).await;
    }
    info(&app)
}

async fn pick_save(app: &AppHandle) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let name = format!("nightgrid-status-{}.html", chrono::Local::now().format("%Y-%m-%d-%H%M"));
    app.dialog()
        .file()
        .set_title("Save dashboard snapshot")
        .set_file_name(name)
        .add_filter("HTML page", &["html"])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

/// Writes a static HTML snapshot of the dashboard. `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn dashboard_export(app: AppHandle, with_addresses: bool) -> CmdResult<Option<String>> {
    let Some(path) = pick_save(&app).await else { return Ok(None) };
    let snap = snapshot(&app, with_addresses);
    let html = render_html(&snap, &RenderOpts { refresh_secs: None, utc_offset_min: utc_offset() });
    tokio::fs::write(&path, html)
        .await
        .map_err(|e| CommandError::msg(format!("Couldn't write {}: {e}", path.display())))?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_range() {
        assert!(is_tailscale(Ipv4Addr::new(100, 101, 2, 3)));
        assert!(!is_tailscale(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(192, 168, 1, 2)));
    }

    #[test]
    fn all_interfaces_needs_explicit_choice() {
        let s = StatusPageSettings { enabled: true, bind: "all".into(), port: 8787 };
        assert_eq!(bind_addrs(&s).unwrap(), vec![IpAddr::V4(Ipv4Addr::UNSPECIFIED)]);
    }
}
