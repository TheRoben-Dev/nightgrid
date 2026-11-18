//! Nightgrid Node pairing (M12f, D-165): finds the node on each host
//! through the pooled SSH connection, keeps one protocol connection per
//! host, fills the app's history from the node's after every reconnect,
//! imports the alerts it recorded while the app was away, takes live
//! samples and events from it, and installs, updates or removes it.
//!
//! Everything here is skipped when Fleet → Nightgrid Node is off, when the
//! host's "use the node" switch is off, or when the node isn't there: the
//! callers then do exactly what they did before.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use nightgrid_core::alerts::Alert;
use nightgrid_core::files::transfer::Side;
use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::ChangeArea;
use nightgrid_core::metrics::Sample;
use nightgrid_core::node::client::{Frame, NodeClient, NodeError};
use nightgrid_core::node::install::{self, Absence, InstallReport, Release, Route};
use nightgrid_core::node::sync::{self, SyncReport};
use nightgrid_core::node::{self, NodeAlert, NodeInfo, SOCKET};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::system::sudo;
use nightgrid_core::system::SystemError;
use serde::Serialize;
use serde_json::json;
use specta::Type;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::commands::{CmdResult, CommandError};
use crate::journal_cmd::{record, Change};
use crate::state::AppState;
use crate::system_cmd::{resolve_sudo, SudoInput, SysResult, SystemCmdError};
use crate::term_cmd::connect_host;

/// Look again this long after finding no usable node on a connection.
const RECHECK_SECS: i64 = 30 * 60;
/// Sync again this often while connected (picks up the node's own history
/// for the time the app's poller was paused or slow).
const RESYNC_SECS: i64 = 15 * 60;
/// A pushed sample older than this isn't used.
const FRESH_MS: f64 = 8_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NodeState {
    /// Not looked yet (the host hasn't been connected).
    Unknown,
    /// Turned off for this host, or Fleet → Nightgrid Node is off.
    Off,
    NotInstalled,
    /// Installed but its socket isn't there (the service is stopped or failed).
    NotRunning,
    /// Running, but the login user isn't in its group.
    NoAccess,
    /// No protocol version in common: the node needs an update.
    Incompatible,
    Ready,
    Error,
}

#[derive(Default)]
struct Link {
    state: Option<NodeState>,
    message: Option<String>,
    client: Option<Arc<NodeClient>>,
    /// The SSH connection the client rides on (pointer identity).
    conn: usize,
    checked: i64,
    user: String,
    arch: String,
    frames: Option<tauri::async_runtime::JoinHandle<()>>,
    events_live: bool,
    metrics_live: bool,
    latest: Option<Box<Sample>>,
    last_ts: f64,
    synced_at: Option<i64>,
    syncing: bool,
    report: Option<SyncReport>,
    rules_note: Option<String>,
}

impl Link {
    fn drop_client(&mut self) {
        if let Some(f) = self.frames.take() {
            f.abort();
        }
        self.client = None;
        self.events_live = false;
        self.metrics_live = false;
        self.latest = None;
    }
}

#[derive(Default)]
struct Hub {
    links: Mutex<HashMap<HostId, Link>>,
    detecting: Mutex<HashSet<HostId>>,
    release: Mutex<Option<(i64, Release)>>,
}

fn hub() -> &'static Hub {
    static HUB: OnceLock<Hub> = OnceLock::new();
    HUB.get_or_init(Hub::default)
}

fn with_link<T>(host: HostId, f: impl FnOnce(&mut Link) -> T) -> T {
    f(hub().links.lock().expect("poisoned").entry(host).or_default())
}

fn settings(app: &AppHandle) -> AppSettings {
    AppSettings::load(&app.state::<AppState>().store()).unwrap_or_default()
}

/// The node may be used on this host.
fn allowed(app: &AppHandle, host: HostId) -> bool {
    let state = app.state::<AppState>();
    let store = state.store();
    AppSettings::load(&store).unwrap_or_default().feature("fleet.node") && store.node_use(host).unwrap_or(true)
}

pub(crate) fn host_name(app: &AppHandle, host: HostId) -> String {
    app.state::<AppState>()
        .store()
        .get_host(host)
        .ok()
        .flatten()
        .map(|h| h.name)
        .unwrap_or_else(|| format!("host {host}"))
}

fn client_name() -> String {
    format!("nightgrid/{}", env!("CARGO_PKG_VERSION"))
}

/// The node connection for `host`, finding and connecting it once per
/// pooled SSH connection. `None` when there is no usable node (callers
/// then use SSH as before). Never prompts.
pub(crate) async fn ensure(app: &AppHandle, host: HostId) -> Option<Arc<NodeClient>> {
    if !allowed(app, host) {
        with_link(host, |l| {
            l.drop_client();
            l.state = Some(NodeState::Off);
        });
        return None;
    }
    let state = app.state::<AppState>();
    if !state.pool.is_connected(host) {
        return with_link(host, |l| l.client.clone().filter(|c| !c.is_closed()));
    }
    let conn = connect_host(app, host).await.ok()?;
    let ptr = Arc::as_ptr(&conn) as usize;
    let now = now_unix();
    let (cached, stale) = with_link(host, |l| {
        if l.conn == ptr {
            if let Some(c) = l.client.clone().filter(|c| !c.is_closed()) {
                let stale = !l.syncing && l.synced_at.map_or(true, |t| now - t >= RESYNC_SECS);
                return (Some(Some(c)), stale);
            }
            if l.state.is_some_and(|s| s != NodeState::Ready && s != NodeState::Off) && now - l.checked < RECHECK_SECS {
                return (Some(None), false);
            }
        }
        (None, false)
    });
    if let Some(c) = cached {
        if stale {
            if let Some(c) = &c {
                spawn_sync(app.clone(), host, c.clone(), false);
            }
        }
        return c;
    }
    if !hub().detecting.lock().expect("poisoned").insert(host) {
        return None;
    }
    let found = detect(app, host, &conn, ptr).await;
    hub().detecting.lock().expect("poisoned").remove(&host);
    found
}

async fn detect(
    app: &AppHandle,
    host: HostId,
    conn: &Arc<nightgrid_core::ssh::client::Connection>,
    ptr: usize,
) -> Option<Arc<NodeClient>> {
    let now = now_unix();
    let set = |s: NodeState, msg: Option<String>| {
        with_link(host, |l| {
            l.drop_client();
            l.state = Some(s);
            l.message = msg;
            l.conn = ptr;
            l.checked = now;
        })
    };
    let probe = match tokio::time::timeout(Duration::from_secs(15), conn.exec(&install::probe_script())).await {
        Ok(Ok((_, out, _))) => install::parse_probe(&String::from_utf8_lossy(&out)),
        Ok(Err(e)) => {
            set(NodeState::Error, Some(e.to_string()));
            return None;
        }
        Err(_) => {
            set(NodeState::Error, Some("looking for the node timed out".into()));
            return None;
        }
    };
    with_link(host, |l| {
        l.user = probe.user.clone();
        l.arch = probe.arch.clone();
    });
    let route = match install::route(&probe) {
        Ok(r) => r,
        Err(Absence::NotInstalled) => {
            set(NodeState::NotInstalled, None);
            return None;
        }
        Err(Absence::NotRunning) => {
            let unit = if probe.unit.is_empty() { "not running".to_owned() } else { probe.unit.clone() };
            set(NodeState::NotRunning, Some(format!("installed, but its service is {unit}")));
            return None;
        }
        Err(Absence::NoAccess) => {
            set(
                NodeState::NoAccess,
                Some(format!("{} isn't in the {} group; Update adds it", probe.user, node::GROUP)),
            );
            return None;
        }
    };
    let channel = match route {
        Route::Socket => match conn.open_streamlocal(SOCKET).await {
            Ok(ch) => Ok(ch),
            // sshd may forbid stream-local forwarding: use the bridge.
            Err(_) => conn.open_exec(&install::bridge_command(route)).await,
        },
        Route::Sg => conn.open_exec(&install::bridge_command(route)).await,
    };
    let channel = match channel {
        Ok(ch) => ch,
        Err(e) => {
            set(NodeState::Error, Some(format!("couldn't open a channel to the node: {e}")));
            return None;
        }
    };
    match NodeClient::connect(channel.into_stream(), &client_name()).await {
        Ok((client, frames)) => {
            let client = Arc::new(client);
            let task = tauri::async_runtime::spawn(read_frames(app.clone(), host, frames));
            with_link(host, |l| {
                l.drop_client();
                l.state = Some(NodeState::Ready);
                l.message = None;
                l.conn = ptr;
                l.checked = now;
                l.client = Some(client.clone());
                l.frames = Some(task);
            });
            spawn_sync(app.clone(), host, client.clone(), true);
            Some(client)
        }
        Err(e) if e.code() == Some("unsupported_proto") => {
            set(NodeState::Incompatible, Some(e.to_string()));
            None
        }
        Err(e) => {
            set(NodeState::Error, Some(e.to_string()));
            None
        }
    }
}

/// Handles pushed lines until the connection ends.
async fn read_frames(app: AppHandle, host: HostId, mut frames: mpsc::UnboundedReceiver<Frame>) {
    while let Some(f) = frames.recv().await {
        match f {
            Frame::Event(e) => {
                let mut alerts = HashMap::new();
                {
                    let state = app.state::<AppState>();
                    let store = state.store();
                    if let Err(err) = sync::apply_event(&store, host, &e, &mut alerts) {
                        tracing::warn!(host = %host, "node event: {err}");
                    }
                    let _ = sync::note_seq(&store, host, e.seq);
                }
                if !alerts.is_empty() {
                    crate::alerts_cmd::import_from_node(
                        &app,
                        &host_name(&app, host),
                        alerts.into_values().collect(),
                        false,
                    );
                }
            }
            Frame::Metrics(s) => with_link(host, |l| l.latest = Some(s)),
            Frame::Lagged { stream, .. } if stream == "events" => {
                let client = with_link(host, |l| l.client.clone());
                if let Some(c) = client {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let mut r = SyncReport::default();
                        let state = app.state::<AppState>();
                        if sync::events(&c, &state.store, host, &mut r).await.is_ok() && !r.alerts.is_empty() {
                            crate::alerts_cmd::import_from_node(&app, &host_name(&app, host), r.alerts, false);
                        }
                    });
                }
            }
            Frame::Lagged { .. } | Frame::Other(_) => {}
        }
    }
    with_link(host, |l| {
        l.client = None;
        l.events_live = false;
        l.metrics_live = false;
        l.latest = None;
    });
}

fn spawn_sync(app: AppHandle, host: HostId, client: Arc<NodeClient>, first: bool) {
    if !with_link(host, |l| !std::mem::replace(&mut l.syncing, true)) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let res = run_sync(&app, host, &client, first).await;
        with_link(host, |l| {
            l.syncing = false;
            l.synced_at = Some(now_unix());
            match res {
                Ok(r) => l.report = Some(r),
                Err(e) => l.message = Some(format!("sync: {e}")),
            }
        });
    });
}

/// Pushes the alert rules, fills history, imports alerts and (protocol 2)
/// subscribes to events, syncing once more to close the gap.
async fn run_sync(app: &AppHandle, host: HostId, client: &NodeClient, first: bool) -> Result<SyncReport, NodeError> {
    let w = client.welcome().clone();
    if first && w.cap("alerts") >= 1 {
        push_rules(app, host, client).await;
    }
    let state = app.state::<AppState>();
    let mut r = sync::all(client, &state.store, host, now_unix()).await?;
    if first && w.streams() && w.cap("events") >= 1 {
        client.call_value("subscribe", json!({ "streams": ["events"] })).await?;
        with_link(host, |l| l.events_live = true);
        sync::events(client, &state.store, host, &mut r).await?;
    }
    if !r.alerts.is_empty() {
        crate::alerts_cmd::import_from_node(app, &host_name(app, host), std::mem::take(&mut r.alerts), first);
    }
    Ok(r)
}

/// Sends the app's alert settings; the node keeps the fields it knows.
async fn push_rules(app: &AppHandle, host: HostId, client: &NodeClient) {
    let cfg = settings(app).effective();
    let Ok(v) = serde_json::to_value(&cfg.alerts) else { return };
    let note = match client.call_value("alerts.rules.set", v).await {
        Ok(_) => None,
        Err(e) if e.code() == Some("bad_request") => Some(format!("the node kept its previous alert rules ({e})")),
        Err(e) => Some(format!("couldn't send the alert rules: {e}")),
    };
    with_link(host, |l| l.rules_note = note);
}

/// Sends changed alert settings to every connected node.
pub(crate) fn rules_changed(app: &AppHandle) {
    let clients: Vec<(HostId, Arc<NodeClient>)> = hub()
        .links
        .lock()
        .expect("poisoned")
        .iter()
        .filter_map(|(h, l)| l.client.clone().filter(|c| c.welcome().cap("alerts") >= 1).map(|c| (*h, c)))
        .collect();
    for (host, c) in clients {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { push_rules(&app, host, &c).await });
    }
}

/// Drops the node connection and finds the node again (after a step that
/// restarted it, such as a config save or `serve add`).
pub(crate) async fn reconnect(app: &AppHandle, host: HostId) -> Option<Arc<NodeClient>> {
    with_link(host, |l| {
        l.drop_client();
        l.state = None;
        l.checked = 0;
    });
    for _ in 0..10 {
        if let Some(c) = ensure(app, host).await {
            return Some(c);
        }
        // The socket comes back a moment after the service restarts.
        tokio::time::sleep(Duration::from_millis(700)).await;
        with_link(host, |l| l.checked = 0);
    }
    None
}

/// What the metrics poller gets from the node this round.
pub(crate) enum NodeSample {
    /// No usable node: sample over SSH as before.
    None,
    /// The node is the source but has nothing newer yet.
    Same,
    Fresh(Box<Sample>),
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

/// A sample from the node instead of running the collect script over SSH:
/// pushed every 2 s while the host is visible (protocol 2), otherwise the
/// node's latest (it samples every 10 s on its own).
pub(crate) async fn sample(app: &AppHandle, host: HostId, visible: bool) -> NodeSample {
    let Some(c) = ensure(app, host).await else {
        return NodeSample::None;
    };
    if c.welcome().cap("metrics") < 1 {
        return NodeSample::None;
    }
    if c.welcome().streams() {
        let live = with_link(host, |l| l.metrics_live);
        if visible != live {
            let method = if visible { "subscribe" } else { "unsubscribe" };
            if c.call_value(method, json!({ "streams": ["metrics"] })).await.is_ok() {
                with_link(host, |l| {
                    l.metrics_live = visible;
                    if !visible {
                        l.latest = None;
                    }
                });
            }
        }
        if visible {
            let got = with_link(host, |l| {
                let s = l.latest.as_ref().filter(|s| now_ms() - s.ts < FRESH_MS && s.ts > l.last_ts)?.clone();
                l.last_ts = s.ts;
                Some(s)
            });
            match got {
                Some(s) => return NodeSample::Fresh(s),
                None if with_link(host, |l| l.metrics_live && l.latest.is_some()) => return NodeSample::Same,
                None => {}
            }
        }
    }
    match c.call::<Option<Sample>>("metrics.latest", json!({})).await {
        Ok(Some(s)) => {
            let newer = with_link(host, |l| {
                let newer = s.ts > l.last_ts;
                l.last_ts = l.last_ts.max(s.ts);
                newer
            });
            if newer {
                NodeSample::Fresh(Box::new(s))
            } else {
                NodeSample::Same
            }
        }
        // Before the node's first sample: use SSH this round.
        Ok(None) => NodeSample::None,
        Err(e) => {
            tracing::debug!(host = %host, "node metrics: {e}");
            with_link(host, Link::drop_client);
            NodeSample::None
        }
    }
}

// ---- commands ----

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeAccess {
    pub name: String,
    /// `ok`, `denied`, `absent` or `off`.
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeCoverage {
    pub res: u32,
    pub points: f64,
    pub oldest: Option<f64>,
    pub newest: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    pub host_id: HostId,
    pub state: NodeState,
    pub message: Option<String>,
    pub use_node: bool,
    pub version: Option<String>,
    /// First 8 characters of the node id.
    pub node_id: Option<String>,
    pub proto: u32,
    pub caps: Vec<String>,
    /// Events arrive as they happen (protocol 2).
    pub events_live: bool,
    /// Samples arrive every 2 s instead of over SSH.
    pub metrics_live: bool,
    pub syncing: bool,
    pub synced_at: Option<f64>,
    pub last_sync: Option<SyncReport>,
    pub rules_note: Option<String>,
    pub user: String,
    pub arch: String,
    /// Filled by `node_check` only.
    pub info: Option<NodeInfoView>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfoView {
    pub started: f64,
    pub interval_secs: u32,
    pub rss_bytes: f64,
    pub db_bytes: f64,
    pub coverage: Vec<NodeCoverage>,
    pub access: Vec<NodeAccess>,
    /// The alerts active on the node right now.
    pub alerts: Vec<Alert>,
}

fn view(app: &AppHandle, host: HostId) -> NodeView {
    let use_node = app.state::<AppState>().store().node_use(host).unwrap_or(true);
    with_link(host, |l| {
        let w = l.client.as_ref().map(|c| c.welcome().clone());
        NodeView {
            host_id: host,
            state: l.state.unwrap_or(NodeState::Unknown),
            message: l.message.clone(),
            use_node,
            version: w.as_ref().map(|w| w.node.version.clone()),
            node_id: w.as_ref().map(|w| w.node.node_id.chars().take(8).collect()),
            proto: w.as_ref().map_or(0, |w| w.proto),
            caps: w.as_ref().map(|w| w.caps.keys().cloned().collect()).unwrap_or_default(),
            events_live: l.events_live,
            metrics_live: l.metrics_live,
            syncing: l.syncing,
            synced_at: l.synced_at.map(|t| t as f64),
            last_sync: l.report.clone(),
            rules_note: l.rules_note.clone(),
            user: l.user.clone(),
            arch: l.arch.clone(),
            info: None,
        }
    })
}

/// What's known about the node on every host, without connecting.
#[tauri::command]
#[specta::specta]
pub fn node_list(app: AppHandle) -> CmdResult<Vec<NodeView>> {
    let hosts = app.state::<AppState>().store().list_hosts()?;
    Ok(hosts.into_iter().map(|h| view(&app, h.id)).collect())
}

/// Looks for the node on `host` now (connecting if needed) and reads its
/// details.
#[tauri::command]
#[specta::specta]
pub async fn node_check(app: AppHandle, host_id: HostId) -> SysResult<NodeView> {
    connect_host(&app, host_id).await?;
    with_link(host_id, |l| {
        if l.client.as_ref().map_or(true, |c| c.is_closed()) {
            l.checked = 0;
        }
    });
    let client = ensure(&app, host_id).await;
    let mut v = view(&app, host_id);
    if let Some(c) = client {
        let info: Option<NodeInfo> =
            if c.welcome().cap("node") >= 1 { c.call("node.info", json!({})).await.ok() } else { None };
        let alerts: Vec<NodeAlert> = if c.welcome().cap("alerts") >= 1 {
            c.call("alerts.active", json!({})).await.unwrap_or_default()
        } else {
            vec![]
        };
        v.info = Some(NodeInfoView {
            started: info.as_ref().map_or(0.0, |i| i.started as f64),
            interval_secs: info.as_ref().map_or(0, |i| i.interval_secs),
            rss_bytes: info.as_ref().map_or(0.0, |i| i.rss_bytes as f64),
            db_bytes: info.as_ref().map_or(0.0, |i| i.db_bytes as f64),
            coverage: info
                .as_ref()
                .map(|i| {
                    i.coverage
                        .iter()
                        .map(|c| NodeCoverage {
                            res: c.res.clamp(0, u32::MAX as i64) as u32,
                            points: c.points as f64,
                            oldest: c.oldest.map(|t| t as f64),
                            newest: c.newest.map(|t| t as f64),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            access: info
                .map(|i| i.access.into_iter().map(|(name, state)| NodeAccess { name, state }).collect())
                .unwrap_or_default(),
            alerts: alerts.iter().filter_map(|a| node::app_alert(a, host_id)).collect(),
        });
    }
    Ok(v)
}

/// Fills history from the node now.
#[tauri::command]
#[specta::specta]
pub async fn node_sync_now(app: AppHandle, host_id: HostId) -> SysResult<SyncReport> {
    let c = ensure(&app, host_id).await.ok_or_else(|| err("Nightgrid Node isn't reachable on this host"))?;
    if with_link(host_id, |l| std::mem::replace(&mut l.syncing, true)) {
        return Err(err("a sync is already running"));
    }
    let res = run_sync(&app, host_id, &c, false).await;
    with_link(host_id, |l| {
        l.syncing = false;
        l.synced_at = Some(now_unix());
        if let Ok(r) = &res {
            l.report = Some(r.clone());
        }
    });
    res.map_err(err)
}

/// Turns using the node on this host on or off (history already synced stays).
#[tauri::command]
#[specta::specta]
pub fn node_set_use(app: AppHandle, host_id: HostId, on: bool) -> CmdResult<NodeView> {
    app.state::<AppState>().store().node_use_set(host_id, on)?;
    with_link(host_id, |l| {
        l.drop_client();
        l.state = if on { None } else { Some(NodeState::Off) };
        l.checked = 0;
    });
    Ok(view(&app, host_id))
}

pub(crate) fn err(e: impl std::fmt::Display) -> SystemCmdError {
    SystemError::other(e).into()
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let req = nightgrid_core::http::Request::get(url)
        .header("User-Agent", "nightgrid")
        .header("Accept", "application/octet-stream, application/json")
        .timeout(Duration::from_secs(120))
        .max_body(install::MAX_BINARY);
    let r = nightgrid_core::http::send_following(req, 5).await?;
    if !r.ok() {
        return Err(format!("GitHub answered {} for {url}", r.status));
    }
    if r.body.len() > install::MAX_BINARY {
        return Err("the download is too large".into());
    }
    Ok(r.body)
}

async fn latest_release() -> Result<Release, String> {
    let now = now_unix();
    if let Some((at, r)) = hub().release.lock().expect("poisoned").clone() {
        if now - at < 3600 {
            return Ok(r);
        }
    }
    let body = fetch(install::LATEST_RELEASE).await?;
    let r = install::parse_release(&String::from_utf8_lossy(&body))?;
    *hub().release.lock().expect("poisoned") = Some((now, r.clone()));
    Ok(r)
}

/// The newest Nightgrid Node release (checked at most once an hour).
#[tauri::command]
#[specta::specta]
pub async fn node_release() -> Result<Release, CommandError> {
    latest_release().await.map_err(CommandError::msg)
}

/// The verified binary `<program>-<tag>-<arch>-linux` from the latest
/// release (`nightgrid-node` or `nightgrid-hub`), cached in the app's data
/// folder.
pub(crate) async fn release_binary(app: &AppHandle, program: &str, arch: &str) -> Result<(String, Vec<u8>), String> {
    let rel = latest_release().await?;
    let name = format!("{program}-{}-{arch}-linux", rel.tag);
    let dir = app.state::<AppState>().data_dir.join("node").join(&rel.tag);
    let cached = dir.join(&name);
    let sums_path = dir.join("SHA256SUMS");
    if let (Ok(bin), Ok(sums)) = (tokio::fs::read(&cached).await, tokio::fs::read_to_string(&sums_path).await) {
        if install::verify(&bin, &sums, &name).is_ok() {
            return Ok((rel.version().to_owned(), bin));
        }
    }
    let asset = rel.asset(&name).ok_or_else(|| format!("release {} has no build for {arch}", rel.tag))?;
    let sums_asset = rel.asset("SHA256SUMS").ok_or_else(|| format!("release {} has no SHA256SUMS", rel.tag))?;
    let sums = String::from_utf8_lossy(&fetch(&sums_asset.url).await?).into_owned();
    let bin = fetch(&asset.url).await?;
    install::verify(&bin, &sums, &name)?;
    if tokio::fs::create_dir_all(&dir).await.is_ok() {
        let _ = tokio::fs::write(&sums_path, &sums).await;
        let _ = tokio::fs::write(&cached, &bin).await;
    }
    Ok((rel.version().to_owned(), bin))
}

pub(crate) async fn pick_binary(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Pick a nightgrid-node binary").pick_file(move |p| {
        let _ = tx.send(p);
    });
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstallResult {
    pub report: InstallReport,
    pub view: NodeView,
}

/// Installs or updates the node on `host_id`: the latest release's binary
/// for the host's architecture, checked against its SHA256SUMS (or a file
/// the person picks when `from_file`), uploaded to a private temp folder
/// and installed by itself as root. `None` when the file pick was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn node_install(
    app: AppHandle,
    host_id: HostId,
    from_file: bool,
    sudo: Option<SudoInput>,
) -> SysResult<Option<NodeInstallResult>> {
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    let (_, out, _) = conn.exec(&install::probe_script()).await?;
    let probe = install::parse_probe(&String::from_utf8_lossy(&out));
    let arch = install::arch(&probe.arch)
        .ok_or_else(|| err(format!("Nightgrid Node has no build for {} (x86_64 and aarch64 only)", probe.arch)))?;
    let (version, bin) = if from_file {
        let Some(path) = pick_binary(&app).await else { return Ok(None) };
        let bin = tokio::fs::read(&path).await.map_err(err)?;
        install::looks_like_binary(&bin).map_err(err)?;
        (String::new(), bin)
    } else {
        release_binary(&app, "nightgrid-node", arch).await.map_err(err)?
    };
    let (code, out, e) = conn.exec("mktemp -d /tmp/nightgrid-node.XXXXXX").await?;
    let dir = String::from_utf8_lossy(&out).trim().to_owned();
    if code != 0 || !dir.starts_with("/tmp/nightgrid-node.") {
        return Err(err(format!("couldn't make a temp folder: {}", String::from_utf8_lossy(&e).trim())));
    }
    let path = format!("{dir}/nightgrid-node");
    let result = async {
        let fs = crate::files_cmd::fs_for(&app, Side::Host { id: host_id }).await.map_err(|e| err(e.message))?;
        let mut w = fs.open_write(&path, None).await.map_err(err)?;
        w.write_all(&bin).await.map_err(err)?;
        w.shutdown().await.map_err(err)?;
        drop(w);
        fs.set_mode(&path, 0o700).await.map_err(err)?;
        let o = sudo::run(&conn, &auth, &install::install_command(&path, &probe.user)).await?;
        Ok::<_, SystemCmdError>(o)
    }
    .await;
    let _ = conn.exec(&format!("rm -rf {}", nightgrid_core::files::path::shell_quote(&dir))).await;
    let o = result?;
    let report = install::parse_report(&o.stdout);
    let verb = if probe.bin { "Updated" } else { "Installed" };
    let shown = if report.version.is_empty() { version } else { report.version.clone() };
    let title = format!("{verb} Nightgrid Node {shown}");
    let ok = o.code == 0;
    record(&app, Change::new(host_id, ChangeArea::System, &title).detail(&report.steps.join("\n")).ok(ok));
    if !ok {
        let why = o.stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the install failed").to_owned();
        return Err(err(why));
    }
    with_link(host_id, |l| {
        l.drop_client();
        l.checked = 0;
        l.state = None;
    });
    // The socket comes up a moment after the service starts.
    tokio::time::sleep(Duration::from_secs(2)).await;
    ensure(&app, host_id).await;
    Ok(Some(NodeInstallResult { report, view: view(&app, host_id) }))
}

/// Removes the node from `host_id` (with its history and account when
/// `purge`), and forgets its sync cursors.
#[tauri::command]
#[specta::specta]
pub async fn node_uninstall(
    app: AppHandle,
    host_id: HostId,
    purge: bool,
    sudo: Option<SudoInput>,
) -> SysResult<NodeInstallResult> {
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    with_link(host_id, Link::drop_client);
    let o = sudo::run(&conn, &auth, &install::uninstall_command(purge)).await?;
    let report = install::parse_report(&o.stdout);
    let title = if purge { "Removed Nightgrid Node and its history" } else { "Removed Nightgrid Node" };
    record(&app, Change::new(host_id, ChangeArea::System, title).detail(&report.steps.join("\n")).ok(o.code == 0));
    if o.code != 0 {
        let why = o.stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the removal failed").to_owned();
        return Err(err(why));
    }
    app.state::<AppState>().store().node_forget(host_id).map_err(err)?;
    with_link(host_id, |l| {
        l.state = Some(NodeState::NotInstalled);
        l.message = None;
        l.report = None;
        l.synced_at = None;
        l.checked = now_unix();
    });
    Ok(NodeInstallResult { report, view: view(&app, host_id) })
}
