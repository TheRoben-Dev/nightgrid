//! The MCP server (M12g, D-166). Off by default. When on, it listens on
//! 127.0.0.1 only, speaks MCP's Streamable HTTP transport (one JSON
//! response per POST) at `/mcp`, and needs the bearer token kept in the
//! app's data folder (`mcp.json`, readable only by this user), which the
//! `--mcp` stdio bridge also reads. Tools read what the app already knows;
//! tools that change something are listed only when writes are on, and
//! each call waits for the person to allow it in the app.

use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use nightgrid_core::docker::Action;
use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::{ChangeArea, ChangeQuery, NewEntry};
use nightgrid_core::mcp::{self, arg_int, arg_str, Dispatch};
use nightgrid_core::metrics::history::Point;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::{AppSettings, McpSettings};
use nightgrid_core::status_page::{new_token, token_matches};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Semaphore};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Event asking the UI to approve a change.
pub const EVENT_ASK: &str = "mcp://ask";
/// How long a change waits for approval.
const ASK_SECS: u64 = 120;
const CALLS_MAX: usize = 50;
const MAX_CONN: usize = 8;
const FILE: &str = "mcp.json";

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpCall {
    pub at: f64,
    pub client: String,
    pub tool: String,
    /// `ok`, `error` or `denied`.
    pub outcome: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpAsk {
    pub id: u32,
    pub client: String,
    pub tool: String,
    /// What would happen, in words.
    pub summary: String,
}

#[derive(Default)]
struct Server {
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    bound: Mutex<(Option<SocketAddr>, Option<String>)>,
    calls: Mutex<VecDeque<McpCall>>,
    asks: Mutex<HashMap<u32, (McpAsk, oneshot::Sender<bool>)>>,
    next_ask: Mutex<u32>,
    client: Mutex<String>,
}

fn server() -> &'static Server {
    static S: OnceLock<Server> = OnceLock::new();
    S.get_or_init(Server::default)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenFile {
    port: u16,
    token: String,
}

pub(crate) fn token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

fn read_token(data_dir: &Path) -> Option<TokenFile> {
    serde_json::from_str(&std::fs::read_to_string(token_path(data_dir)).ok()?).ok()
}

fn write_token(data_dir: &Path, f: &TokenFile) -> Result<(), String> {
    let path = token_path(data_dir);
    let text = serde_json::to_string(f).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        // An older file may have kept wider permissions.
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(())
}

/// The token for `port`, made on first use.
fn ensure_token(data_dir: &Path, port: u16) -> Result<String, String> {
    match read_token(data_dir) {
        Some(mut f) if !f.token.is_empty() => {
            if f.port != port {
                f.port = port;
                write_token(data_dir, &f)?;
            }
            Ok(f.token)
        }
        _ => {
            let f = TokenFile { port, token: new_token() };
            write_token(data_dir, &f)?;
            Ok(f.token)
        }
    }
}

fn settings(app: &AppHandle) -> AppSettings {
    AppSettings::load(&app.state::<AppState>().store()).unwrap_or_default()
}

/// Starts, restarts or stops the server to match the settings.
pub fn apply(app: &AppHandle) {
    let srv = server();
    if let Some(t) = srv.task.lock().expect("poisoned").take() {
        t.abort();
    }
    *srv.bound.lock().expect("poisoned") = (None, None);
    let cfg = settings(app).mcp;
    if !cfg.enabled {
        return;
    }
    let data_dir = app.state::<AppState>().data_dir.clone();
    let token = match ensure_token(&data_dir, cfg.port) {
        Ok(t) => t,
        Err(e) => {
            srv.bound.lock().expect("poisoned").1 = Some(format!("couldn't write {FILE}: {e}"));
            return;
        }
    };
    let app = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let l = match TcpListener::bind((Ipv4Addr::LOCALHOST, cfg.port)).await {
            Ok(l) => l,
            Err(e) => {
                server().bound.lock().expect("poisoned").1 =
                    Some(format!("Can't listen on 127.0.0.1:{}: {e}", cfg.port));
                return;
            }
        };
        let addr = l.local_addr().ok();
        tracing::info!(?addr, "MCP server listening");
        *server().bound.lock().expect("poisoned") = (addr, None);
        let token = Arc::new(token);
        let slots = Arc::new(Semaphore::new(MAX_CONN));
        loop {
            let Ok((sock, _)) = l.accept().await else { continue };
            let Ok(permit) = slots.clone().try_acquire_owned() else { continue };
            let (app, token) = (app.clone(), token.clone());
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) =
                    tokio::time::timeout(Duration::from_secs(ASK_SECS + 30), serve(&app, sock, &token)).await
                {
                    tracing::debug!("mcp request timed out: {e}");
                }
            });
        }
    });
    *srv.task.lock().expect("poisoned") = Some(task);
}

async fn serve(app: &AppHandle, mut sock: TcpStream, token: &str) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > mcp::HEAD_MAX {
            return sock.write_all(&mcp::http_response("431 Request Header Fields Too Large", "text/plain", b"")).await;
        }
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = mcp::parse_head(&String::from_utf8_lossy(&buf[..head_end]));
    let reply = |status: &str, body: &str| mcp::http_response(status, "text/plain; charset=utf-8", body.as_bytes());
    let out = if head.path != mcp::PATH {
        reply("404 Not Found", "Not found.\n")
    } else if !mcp::origin_allowed(head.origin.as_deref()) {
        reply("403 Forbidden", "Only this computer may call.\n")
    } else if !mcp::bearer(head.authorization.as_deref()).is_some_and(|t| token_matches(t, token)) {
        reply("401 Unauthorized", "Needs the token from Nightgrid → Settings → Connections → MCP server.\n")
    } else if head.method != "POST" {
        reply("405 Method Not Allowed", "POST JSON-RPC messages here.\n")
    } else if head.content_length > mcp::BODY_MAX {
        reply("413 Payload Too Large", "Too large.\n")
    } else {
        let mut body = buf[head_end..].to_vec();
        while body.len() < head.content_length {
            let n = sock.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(head.content_length);
        match handle(app, &body).await {
            Some(v) => mcp::http_response("200 OK", "application/json", v.to_string().as_bytes()),
            None => mcp::http_response("202 Accepted", "text/plain", b""),
        }
    };
    sock.write_all(&out).await?;
    sock.shutdown().await
}

/// One JSON-RPC message in, its response out (`None` for notifications).
async fn handle(app: &AppHandle, body: &[u8]) -> Option<Value> {
    let msg: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return Some(mcp::error(&Value::Null, mcp::PARSE_ERROR, &format!("not JSON: {e}"))),
    };
    if msg.get("method").and_then(Value::as_str) == Some("initialize") {
        let name = msg.pointer("/params/clientInfo/name").and_then(Value::as_str).unwrap_or("an MCP client");
        *server().client.lock().expect("poisoned") = name.chars().take(60).collect();
    }
    let writes = settings(app).mcp.allow_writes;
    match mcp::dispatch(&msg, writes, env!("CARGO_PKG_VERSION")) {
        Dispatch::Reply(v) => Some(v),
        Dispatch::Nothing => None,
        Dispatch::Call { id, name, args } => {
            let client = server().client.lock().expect("poisoned").clone();
            let res = run_tool(app, &name, &args, &client).await;
            let (outcome, detail, result) = match res {
                Ok(v) => ("ok", arg_summary(&args), mcp::tool_result(&v)),
                Err(Refused::Denied(why)) => ("denied", why.clone(), mcp::tool_error(&why)),
                Err(Refused::Failed(why)) => ("error", why.clone(), mcp::tool_error(&why)),
            };
            log_call(&client, &name, outcome, &detail);
            Some(mcp::ok(&id, result))
        }
    }
}

/// `host=server01 hours=24` for the recent-calls list.
fn arg_summary(args: &Value) -> String {
    let Some(o) = args.as_object() else { return String::new() };
    o.iter()
        .map(|(k, v)| match v {
            Value::String(s) => format!("{k}={s}"),
            v => format!("{k}={v}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn log_call(client: &str, tool: &str, outcome: &str, detail: &str) {
    let mut c = server().calls.lock().expect("poisoned");
    c.push_front(McpCall {
        at: now_unix() as f64,
        client: client.to_owned(),
        tool: tool.to_owned(),
        outcome: outcome.to_owned(),
        detail: detail.chars().take(200).collect(),
    });
    c.truncate(CALLS_MAX);
}

enum Refused {
    /// The person said no, or didn't answer.
    Denied(String),
    Failed(String),
}

impl<E: std::fmt::Display> From<E> for Refused {
    fn from(e: E) -> Self {
        Refused::Failed(e.to_string())
    }
}

fn fail(msg: impl Into<String>) -> Refused {
    Refused::Failed(msg.into())
}

/// A host by name (any case) or numeric id.
fn find_host(app: &AppHandle, args: &Value) -> Result<(HostId, String), Refused> {
    let want = arg_str(args, "host").ok_or_else(|| fail("which host? (pass host)"))?;
    let hosts = app.state::<AppState>().store().list_hosts()?;
    hosts
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(want) || h.id.to_string() == want)
        .map(|h| (h.id, h.name.clone()))
        .ok_or_else(|| {
            let names: Vec<&str> = hosts.iter().map(|h| h.name.as_str()).take(30).collect();
            fail(format!("no host named {want:?}; hosts: {}", names.join(", ")))
        })
}

/// Averages history points into at most `max` buckets.
fn thin(points: &[Point], max: usize) -> Vec<Value> {
    if points.is_empty() {
        return vec![];
    }
    let per = points.len().div_ceil(max);
    points
        .chunks(per)
        .map(|c| {
            let avg32 = |f: fn(&Point) -> Option<f32>| {
                let v: Vec<f32> = c.iter().filter_map(f).collect();
                (!v.is_empty()).then(|| (v.iter().sum::<f32>() / v.len() as f32 * 10.0).round() / 10.0)
            };
            let avg64 = |f: fn(&Point) -> Option<f64>| {
                let v: Vec<f64> = c.iter().filter_map(f).collect();
                (!v.is_empty()).then(|| (v.iter().sum::<f64>() / v.len() as f64).round())
            };
            let ts = c[0].ts as i64;
            json!({
                "at": chrono::DateTime::from_timestamp(ts, 0).map(|d| d.to_rfc3339()).unwrap_or_default(),
                "cpu": avg32(|p| p.cpu), "mem": avg32(|p| p.mem), "swap": avg32(|p| p.swap),
                "load1": avg32(|p| p.load1), "disk": avg32(|p| p.disk), "temp": avg32(|p| p.temp),
                "netRxBps": avg64(|p| p.net_rx), "netTxBps": avg64(|p| p.net_tx),
                "diskReadBps": avg64(|p| p.disk_read), "diskWriteBps": avg64(|p| p.disk_write),
            })
        })
        .collect()
}

fn iso(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.to_rfc3339()).unwrap_or_default()
}

async fn run_tool(app: &AppHandle, name: &str, args: &Value, client: &str) -> Result<Value, Refused> {
    let state = app.state::<AppState>();
    let now = now_unix();
    Ok(match name {
        "list_hosts" => {
            let snap = crate::status_cmd::snapshot(app, true);
            let hosts = state.store().list_hosts()?;
            let rows: Vec<Value> = snap
                .hosts
                .iter()
                .map(|s| {
                    let h = hosts.iter().find(|h| h.name == s.name);
                    json!({
                        "name": s.name, "address": s.address, "state": s.state,
                        "cpuPct": s.cpu, "memPct": s.mem,
                        "fullestDisk": s.disk.as_ref().map(|(p, m)| json!({ "mount": m, "usedPct": p })),
                        "tempC": s.temp, "uptimeSecs": s.uptime_secs,
                        "tags": h.map(|h| h.tags.clone()).unwrap_or_default(),
                        "role": h.map(|h| h.role.clone()).unwrap_or_default(),
                    })
                })
                .collect();
            json!({ "hosts": rows })
        }
        "host_status" => {
            let (id, host) = find_host(app, args)?;
            let poll = state.metrics.poll_states().get(&id).cloned();
            let sample = state.metrics.latest(id);
            json!({
                "host": host,
                "state": poll.as_ref().map(|(s, _)| serde_json::to_value(s).unwrap_or_default()),
                "message": poll.and_then(|(_, m)| m),
                "sample": sample.map(|s| {
                    let mut v = serde_json::to_value(&*s).unwrap_or_default();
                    v["at"] = json!(iso((s.ts / 1000.0) as i64));
                    v
                }),
            })
        }
        "metric_history" => {
            let (id, host) = find_host(app, args)?;
            let hours = arg_int(args, "hours", 24, 1, 2160);
            let points = state.store().metric_history(id, now - hours * 3600, now)?;
            json!({ "host": host, "hours": hours, "points": thin(&points, 120) })
        }
        "list_alerts" => {
            let active = state.alerts.active();
            let mut v = json!({ "active": active });
            if args.get("history").and_then(Value::as_bool).unwrap_or(false) {
                v["resolved"] = json!(state.alerts.history().into_iter().take(50).collect::<Vec<_>>());
            }
            v
        }
        "host_timeline" => {
            let (id, host) = find_host(app, args)?;
            let hours = arg_int(args, "hours", 24, 1, 2160);
            let events = state.store().events_for(id, now - hours * 3600)?;
            let rows: Vec<Value> = events
                .iter()
                .take(300)
                .map(|e| json!({ "at": iso(e.at), "kind": e.kind, "level": e.level, "title": e.title, "detail": e.detail }))
                .collect();
            json!({ "host": host, "hours": hours, "events": rows })
        }
        "service_checks" => {
            let snap = crate::status_cmd::snapshot(app, false);
            json!({ "services": snap.services })
        }
        "journal" => {
            let host = if arg_str(args, "host").is_some() { Some(find_host(app, args)?.0) } else { None };
            let q = ChangeQuery {
                text: arg_str(args, "text").unwrap_or("").to_owned(),
                host_id: host,
                since: Some(now - arg_int(args, "days", 14, 1, 730) * 86_400),
                limit: arg_int(args, "limit", 50, 1, 200) as u32,
                ..Default::default()
            };
            let rows: Vec<Value> = state
                .store()
                .journal_query(&q)?
                .into_iter()
                .map(|e| {
                    let (plus, minus) = e.diff.as_deref().map_or((0, 0), |d| {
                        (d.lines().filter(|l| l.starts_with('+')).count(), d.lines().filter(|l| l.starts_with('-')).count())
                    });
                    json!({ "at": iso(e.at), "host": e.host, "area": e.area, "title": e.title, "detail": e.detail,
                            "ok": e.ok, "byHand": e.manual, "diff": e.diff.as_ref().map(|_| format!("+{plus} −{minus} lines")) })
                })
                .collect();
            json!({ "entries": rows })
        }
        "disk_forecast" => {
            let store = state.store();
            let mut rows = Vec::new();
            let fleet = store.fleet_disk_forecasts(now)?;
            for h in store.list_hosts()? {
                for f in fleet.get(&h.id).into_iter().flatten() {
                    let mut v = serde_json::to_value(f)?;
                    v["host"] = json!(h.name);
                    rows.push(v);
                }
            }
            json!({ "mounts": rows })
        }
        "list_containers" => {
            let (id, host) = find_host(app, args)?;
            let d = crate::docker_cmd::docker_for(app, id).await.map_err(|e| fail(e.to_string()))?;
            let list = d.containers().await.map_err(|e| fail(e.to_string()))?;
            let rows: Vec<Value> = list
                .iter()
                .map(|c| json!({ "name": c.name, "image": c.image, "state": c.state, "status": c.status, "health": c.health, "project": c.project }))
                .collect();
            json!({ "host": host, "containers": rows })
        }
        "container_action" => {
            let (id, host) = find_host(app, args)?;
            let container =
                arg_str(args, "container").ok_or_else(|| fail("which container? (pass container)"))?.to_owned();
            let (action, verb, done) = match arg_str(args, "action") {
                Some("start") => (Action::Start, "Start", "Started"),
                Some("stop") => (Action::Stop, "Stop", "Stopped"),
                Some("restart") => (Action::Restart, "Restart", "Restarted"),
                _ => return Err(fail("action must be start, stop or restart")),
            };
            ask(app, client, "container_action", format!("{verb} container {container} on {host}")).await?;
            let res = crate::docker_cmd::docker_action(app.clone(), id, vec![container.clone()], action)
                .await
                .map_err(|e| fail(e.to_string()))?;
            match res.into_iter().next().and_then(|r| r.error) {
                Some(e) => return Err(fail(e)),
                None => json!({ "done": format!("{done} {container} on {host}") }),
            }
        }
        "add_journal_note" => {
            let title = arg_str(args, "title").ok_or_else(|| fail("a note needs a title"))?;
            let title: String = title.chars().take(200).collect();
            let detail: String = arg_str(args, "detail").unwrap_or("").chars().take(4000).collect();
            let host = if arg_str(args, "host").is_some() { Some(find_host(app, args)?) } else { None };
            let on = host.as_ref().map(|(_, n)| format!(" for {n}")).unwrap_or_default();
            ask(app, client, "add_journal_note", format!("Add a journal note{on}: \"{title}\"")).await?;
            let id = state.store().journal_add(&NewEntry {
                at: now,
                host_id: host.as_ref().map(|(id, _)| *id),
                host: host.map(|(_, n)| n).unwrap_or_default(),
                area: Some(ChangeArea::Note),
                title,
                detail: format!("{detail}\n\n(added by {client} through MCP)").trim().to_owned(),
                diff: None,
                ok: true,
                manual: true,
            })?;
            json!({ "added": id })
        }
        other => return Err(fail(format!("unknown tool {other}"))),
    })
}

/// Waits for the person to allow a change in the app.
async fn ask(app: &AppHandle, client: &str, tool: &str, summary: String) -> Result<(), Refused> {
    let srv = server();
    let id = {
        let mut n = srv.next_ask.lock().expect("poisoned");
        *n = n.wrapping_add(1);
        *n
    };
    let (tx, rx) = oneshot::channel();
    let a = McpAsk { id, client: client.to_owned(), tool: tool.to_owned(), summary: summary.clone() };
    srv.asks.lock().expect("poisoned").insert(id, (a.clone(), tx));
    let _ = app.emit(EVENT_ASK, a);
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if !focused {
        let _ = app
            .notification()
            .builder()
            .title("NIGHTGRID · approve a change?")
            .body(format!("{client}: {summary}"))
            .show();
    }
    let answer = tokio::time::timeout(Duration::from_secs(ASK_SECS), rx).await;
    srv.asks.lock().expect("poisoned").remove(&id);
    match answer {
        Ok(Ok(true)) => Ok(()),
        Ok(Ok(false)) => Err(Refused::Denied("The person declined this in Nightgrid.".into())),
        _ => Err(Refused::Denied(format!("Nobody approved this in Nightgrid within {} minutes.", ASK_SECS / 60))),
    }
}

// ---- commands ----

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub settings: McpSettings,
    pub running: bool,
    pub url: Option<String>,
    pub error: Option<String>,
    /// Shown so it can be pasted into a client's config.
    pub token: Option<String>,
    /// This app's executable, for a stdio client config (`--mcp`).
    pub exe: Option<String>,
    pub calls: Vec<McpCall>,
    /// Changes waiting for an answer.
    pub asks: Vec<McpAsk>,
}

fn status(app: &AppHandle) -> McpStatus {
    let state = app.state::<AppState>();
    let cfg = settings(app).mcp;
    let (addr, error) = server().bound.lock().expect("poisoned").clone();
    McpStatus {
        running: addr.is_some(),
        url: addr.map(|a| format!("http://{a}{}", mcp::PATH)),
        error,
        token: if cfg.enabled { read_token(&state.data_dir).map(|f| f.token) } else { None },
        exe: std::env::current_exe().ok().map(|p| p.display().to_string()),
        calls: server().calls.lock().expect("poisoned").iter().cloned().collect(),
        asks: server().asks.lock().expect("poisoned").values().map(|(a, _)| a.clone()).collect(),
        settings: cfg,
    }
}

#[tauri::command]
#[specta::specta]
pub fn mcp_status(app: AppHandle) -> CmdResult<McpStatus> {
    Ok(status(&app))
}

/// Saves the MCP settings and starts or stops the server.
#[tauri::command]
#[specta::specta]
pub async fn mcp_set(app: AppHandle, settings: McpSettings) -> CmdResult<McpStatus> {
    {
        let state = app.state::<AppState>();
        let store = state.store();
        let mut s = AppSettings::load(&store)?;
        s.mcp = settings;
        s.save(&store)?;
    }
    apply(&app);
    // Let the listener bind (or fail) before reporting.
    tokio::time::sleep(Duration::from_millis(150)).await;
    Ok(status(&app))
}

/// A new token: clients need the new one.
#[tauri::command]
#[specta::specta]
pub async fn mcp_new_token(app: AppHandle) -> CmdResult<McpStatus> {
    let state = app.state::<AppState>();
    let port = settings(&app).mcp.port;
    write_token(&state.data_dir, &TokenFile { port, token: new_token() }).map_err(CommandError::msg)?;
    crate::secrets_cmd::stamp(&state, "app:mcp");
    apply(&app);
    tokio::time::sleep(Duration::from_millis(150)).await;
    Ok(status(&app))
}

/// The person's answer to a change an MCP client asked for.
#[tauri::command]
#[specta::specta]
pub fn mcp_answer(id: u32, allow: bool) -> CmdResult<()> {
    if let Some((_, tx)) = server().asks.lock().expect("poisoned").remove(&id) {
        let _ = tx.send(allow);
    }
    Ok(())
}

// ---- the stdio bridge (`nightgrid --mcp`) ----

/// The app's data folder without Tauri (same rule Tauri uses).
fn data_dir_guess() -> Option<PathBuf> {
    let id = "dev.nightgrid.Nightgrid";
    dirs::data_dir().map(|d| d.join(id))
}

/// Runs as an MCP stdio server for clients that start a command: each line
/// on stdin is posted to the running app's MCP server and its answer
/// printed. Returns the exit code.
pub fn stdio_bridge() -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("nightgrid --mcp: {e}");
            return 1;
        }
    };
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let id = serde_json::from_str::<Value>(&line).ok().and_then(|v| v.get("id").cloned());
        let answer = match rt.block_on(forward(&line)) {
            Ok(Some(body)) => Some(body),
            Ok(None) => None,
            Err(why) => id.map(|id| mcp::error(&id, -32000, &why).to_string()),
        };
        if let Some(a) = answer {
            if writeln!(out, "{}", a.trim()).is_err() || out.flush().is_err() {
                break;
            }
        }
    }
    0
}

async fn forward(line: &str) -> Result<Option<String>, String> {
    const OFF: &str =
        "Nightgrid isn't running, or its MCP server is off: open Nightgrid → Settings → Connections → MCP server.";
    let f = data_dir_guess().and_then(|d| read_token(&d)).ok_or(OFF)?;
    let req = nightgrid_core::http::Request::post(
        format!("http://127.0.0.1:{}{}", f.port, mcp::PATH),
        line.as_bytes().to_vec(),
    )
    .header("Content-Type", "application/json")
    .header("Accept", "application/json, text/event-stream")
    .header("Authorization", format!("Bearer {}", f.token))
    .timeout(Duration::from_secs(ASK_SECS + 20));
    let r = nightgrid_core::http::send(req).await.map_err(|_| OFF.to_owned())?;
    match r.status {
        200 => Ok(Some(String::from_utf8_lossy(&r.body).into_owned())),
        202 => Ok(None),
        401 => Err("Nightgrid refused the token: turn the MCP server off and on, or make a new token.".into()),
        s => Err(format!("Nightgrid answered HTTP {s}")),
    }
}
