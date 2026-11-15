//! Alerts: one background loop that reads what the app already watches
//! (metrics poll states and mounts, service checks), polls Docker on
//! connected hosts every minute and SMART every six hours, runs it through
//! the core [`Tracker`], and delivers changes: the UI snapshot, the tray,
//! native notifications and the optional webhook.
//!
//! The loop never opens new SSH connections for Docker or SMART: it only
//! uses hosts already in the pool. SMART only runs where sudo needs no
//! prompt (root, NOPASSWD, or a password the user chose to remember).

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use nightgrid_core::alerts::{self, Alert, Change, ContainerWatch, Scope, Severity, Tracker};
use nightgrid_core::backup_jobs;
use nightgrid_core::docker::LogQuery;
use nightgrid_core::hosts::HostId;
use nightgrid_core::logwatch::{self, Detector};
use nightgrid_core::secrets::{Secret, SecretRef, SecretStore};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::system::smart;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

use crate::commands::{CmdResult, CommandError};
use crate::metrics_cmd::PollState;
use crate::state::AppState;

const TICK: Duration = Duration::from_secs(15);
const DOCKER_EVERY: i64 = 60;
const SMART_EVERY: i64 = 6 * 3600;
const BACKUP_EVERY: i64 = 30 * 60;
const LOGS_EVERY: i64 = 10 * 60;
/// The first log read on a host learns from this far back.
const LOGS_LEARN: i64 = 3600;
/// Containers whose logs are read per host and round.
const LOGS_MAX_CONTAINERS: usize = 60;
/// After Docker fails on a host (not installed, no permission), wait this long.
const DOCKER_BACKOFF: i64 = 600;
/// More fired alerts than this in one round become one summary notification.
const NOTIFY_BATCH: usize = 3;
const WEBHOOK_MAX_PER_ROUND: usize = 10;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AlertsSnapshot {
    pub active: Vec<Alert>,
    pub history: Vec<Alert>,
    /// Quiet hours are on right now (no notifications or webhooks).
    pub quiet: bool,
    pub enabled: bool,
}

#[derive(Default)]
pub struct AlertsHub {
    tracker: Mutex<Tracker>,
    containers: Mutex<ContainerWatch>,
    channel: Mutex<Option<Channel<AlertsSnapshot>>>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    wake: Notify,
    docker_skip: Mutex<HashMap<HostId, i64>>,
    smart_last: Mutex<HashMap<HostId, i64>>,
    backup_last: Mutex<i64>,
    logs_last: Mutex<HashMap<HostId, i64>>,
    logs: Mutex<Detector>,
}

impl AlertsHub {
    /// Re-evaluates soon (settings changed).
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    pub fn active_count(&self) -> (usize, usize) {
        let t = self.tracker.lock().expect("poisoned");
        let act = t.active();
        (act.len(), act.iter().filter(|a| a.severity == Severity::Critical).count())
    }

    fn snapshot(&self, settings: &AppSettings) -> AlertsSnapshot {
        let t = self.tracker.lock().expect("poisoned");
        AlertsSnapshot {
            active: t.active(),
            history: t.history(),
            quiet: is_quiet(settings),
            enabled: settings.alerts.enabled,
        }
    }

    fn push(&self, settings: &AppSettings) {
        let snap = self.snapshot(settings);
        if let Some(ch) = self.channel.lock().expect("poisoned").as_ref() {
            let _ = ch.send(snap);
        }
    }
}

fn settings(app: &AppHandle) -> AppSettings {
    AppSettings::load(&app.state::<AppState>().store()).unwrap_or_default()
}

fn local_minutes() -> u16 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    (now.hour() * 60 + now.minute()) as u16
}

fn is_quiet(s: &AppSettings) -> bool {
    s.alerts.quiet_hours && alerts::in_quiet_hours(s.alerts.quiet_start, s.alerts.quiet_end, local_minutes())
}

fn webhook_ref() -> SecretRef {
    SecretRef::alerts_webhook()
}

fn webhook_url(app: &AppHandle) -> Option<String> {
    match app.state::<AppState>().keychain.get(&webhook_ref()) {
        Ok(s) => s.map(|s| s.expose().to_owned()),
        Err(e) => {
            tracing::warn!("keychain read failed: {e}");
            None
        }
    }
}

/// Starts the loop once (from app setup).
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let hub = state.alerts.clone();
    let mut task = hub.task.lock().expect("poisoned");
    if task.is_some() {
        return;
    }
    let app = app.clone();
    *task = Some(tauri::async_runtime::spawn(async move {
        // Let the UI subscribe and the first metrics/check results arrive.
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut last_docker = 0i64;
        let mut last_quiet = None;
        loop {
            let cfg = settings(&app);
            let now = now_unix();
            let changes = evaluate(&app, &cfg, now, &mut last_docker).await;
            let quiet = is_quiet(&cfg);
            let hub = app.state::<AppState>().alerts.clone();
            if !changes.is_empty() || last_quiet != Some(quiet) {
                deliver(&app, &cfg, &changes, quiet);
                hub.push(&cfg);
                last_quiet = Some(quiet);
            }
            crate::tray::refresh(&app);
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = hub.wake.notified() => {}
            }
        }
    }));
}

async fn evaluate(app: &AppHandle, cfg: &AppSettings, now: i64, last_docker: &mut i64) -> Vec<Change> {
    let state = app.state::<AppState>();
    let hub = state.alerts.clone();
    let rules = &cfg.alerts;
    let (hosts, services) = {
        let store = state.store();
        let hosts: HashMap<HostId, String> =
            store.list_hosts().unwrap_or_default().into_iter().map(|h| (h.id, h.name)).collect();
        (hosts, store.list_services().unwrap_or_default())
    };
    let mut changes = Vec::new();
    {
        let mut t = hub.tracker.lock().expect("poisoned");
        for gone in t.hosts().into_iter().filter(|h| !hosts.contains_key(h)) {
            t.forget_host(gone);
            hub.containers.lock().expect("poisoned").forget_host(gone);
            hub.logs.lock().expect("poisoned").forget(&format!("{gone}|"));
            hub.logs_last.lock().expect("poisoned").remove(&gone);
        }
        if !rules.enabled {
            t.drop_active();
            return changes;
        }
    }
    let name = |id: HostId| hosts.get(&id).cloned().unwrap_or_else(|| format!("host {id}"));

    // Host offline and disk usage, from the metrics pollers.
    let polls = if cfg.metrics_enabled { state.metrics.poll_states() } else { HashMap::new() };
    let mut offline = Vec::new();
    let mut disks = Vec::new();
    let mut sampled = HashSet::new();
    for (id, (poll, message)) in &polls {
        if !hosts.contains_key(id) {
            continue;
        }
        match poll {
            PollState::Offline if rules.host_offline => {
                offline.push(alerts::host_offline(*id, &name(*id), message.as_deref()))
            }
            PollState::Ok => {
                sampled.insert(*id);
                if rules.disk_full {
                    if let Some(m) = state.metrics.mounts(*id) {
                        disks.extend(alerts::disk_full(*id, &name(*id), &m, rules.disk_percent));
                    }
                }
            }
            _ => {}
        }
    }
    // Disk alerts for a host that's not reporting right now are left as they
    // are (stale, but the offline alert explains it), unless the rule or
    // metrics were turned off.
    let disk_scope = if rules.disk_full && cfg.metrics_enabled { Some(&sampled) } else { None };
    {
        let mut t = hub.tracker.lock().expect("poisoned");
        changes.extend(t.report(&Scope { source: "host", hosts: None }, offline, now));
        changes.extend(t.report(&Scope { source: "disk", hosts: disk_scope }, disks, now));
    }

    // Services and certificates, from the checker's latest results.
    let latest = if cfg.services_checks { state.services.latest() } else { HashMap::new() };
    let mut svc = Vec::new();
    for s in services.iter().filter(|s| s.check_enabled) {
        let Some(st) = latest.get(&s.id) else { continue };
        let conds = alerts::service(s.id, &s.name, s.host_id, &st.result, rules.cert_days, now);
        svc.extend(conds.into_iter().filter(|c| match c.kind {
            alerts::AlertKind::ServiceDown => rules.service_down,
            alerts::AlertKind::CertExpiring => rules.cert_expiring,
            _ => true,
        }));
    }
    changes.extend(hub.tracker.lock().expect("poisoned").report(&Scope { source: "svc", hosts: None }, svc, now));

    let connected: Vec<HostId> = state.pool.connected().into_iter().filter(|h| hosts.contains_key(h)).collect();

    // Containers on connected hosts.
    if !rules.container_down {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "docker", hosts: None },
            vec![],
            now,
        ));
    } else if now - *last_docker >= DOCKER_EVERY {
        *last_docker = now;
        let mut covered = HashSet::new();
        let mut conds = Vec::new();
        for id in &connected {
            if hub.docker_skip.lock().expect("poisoned").get(id).is_some_and(|until| *until > now) {
                continue;
            }
            let list = async {
                let d = crate::docker_cmd::docker_for(app, *id).await.map_err(|e| e.message)?;
                d.containers().await.map_err(|e| e.to_string())
            };
            match tokio::time::timeout(Duration::from_secs(15), list).await {
                Ok(Ok(list)) => {
                    covered.insert(*id);
                    conds.extend(hub.containers.lock().expect("poisoned").conditions(*id, &name(*id), &list));
                }
                Ok(Err(e)) => {
                    tracing::debug!(host = id, "alerts: docker unavailable: {e}");
                    hub.docker_skip.lock().expect("poisoned").insert(*id, now + DOCKER_BACKOFF);
                }
                Err(_) => {
                    hub.docker_skip.lock().expect("poisoned").insert(*id, now + DOCKER_BACKOFF);
                }
            }
        }
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "docker", hosts: Some(&covered) },
            conds,
            now,
        ));
    }

    // SMART on connected hosts, at most every six hours each.
    if !rules.smart_warning {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "smart", hosts: None },
            vec![],
            now,
        ));
    } else {
        let due: Vec<HostId> = {
            let last = hub.smart_last.lock().expect("poisoned");
            connected.iter().copied().filter(|id| !matches!(last.get(id), Some(t) if now - t < SMART_EVERY)).collect()
        };
        let mut covered = HashSet::new();
        let mut conds = Vec::new();
        for id in due {
            hub.smart_last.lock().expect("poisoned").insert(id, now);
            let scan = async {
                let conn = crate::term_cmd::connect_host(app, id).await.ok()?;
                // No input: never prompts. Hosts that need a typed password are skipped.
                let auth = crate::system_cmd::resolve_sudo(app, id, &conn, None).await.ok()?;
                smart::scan(&conn, &auth).await.ok()
            };
            if let Ok(Some(report)) = tokio::time::timeout(Duration::from_secs(90), scan).await {
                covered.insert(id);
                conds.extend(alerts::smart(id, &name(id), &report.disks));
            }
        }
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "smart", hosts: Some(&covered) },
            conds,
            now,
        ));
    }

    // Backup jobs on connected hosts, every 30 minutes.
    if !rules.backup_stale {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "backup", hosts: None },
            vec![],
            now,
        ));
    } else if now - *hub.backup_last.lock().expect("poisoned") >= BACKUP_EVERY {
        *hub.backup_last.lock().expect("poisoned") = now;
        let jobs = state.store().list_backup_jobs().unwrap_or_default();
        let mut covered = HashSet::new();
        let mut conds = Vec::new();
        for id in &connected {
            let mine: Vec<_> = jobs.iter().filter(|j| j.host_id == *id).cloned().collect();
            if mine.is_empty() {
                covered.insert(*id);
                continue;
            }
            let run = async {
                let conn = crate::term_cmd::connect_host(app, *id).await.ok()?;
                backup_jobs::check(&conn, &mine, now as f64).await.ok()
            };
            if let Ok(Some(statuses)) = tokio::time::timeout(Duration::from_secs(60), run).await {
                covered.insert(*id);
                for (j, st) in mine.iter().zip(&statuses) {
                    conds.extend(alerts::backup(j, &name(*id), st));
                }
            }
        }
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "backup", hosts: Some(&covered) },
            conds,
            now,
        ));
    }

    // Log anomalies on connected hosts, every 10 minutes each (opt-in).
    if !rules.log_anomaly {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "logs", hosts: None },
            vec![],
            now,
        ));
    } else {
        let mut covered = HashSet::new();
        let mut conds = Vec::new();
        for id in &connected {
            let since = hub.logs_last.lock().expect("poisoned").get(id).copied();
            if since.is_some_and(|t| now - t < LOGS_EVERY) {
                continue;
            }
            let since = since.unwrap_or(now - LOGS_LEARN);
            let read = read_error_lines(app, *id, since, now);
            let Ok(Some(by_source)) = tokio::time::timeout(Duration::from_secs(90), read).await else { continue };
            hub.logs_last.lock().expect("poisoned").insert(*id, now);
            covered.insert(*id);
            let mut det = hub.logs.lock().expect("poisoned");
            let prefix = format!("{id}|");
            let mut sources: Vec<String> = by_source.keys().cloned().collect();
            for known in det.sources_with_prefix(&prefix) {
                if !sources.contains(&known) {
                    sources.push(known);
                }
            }
            for key in sources {
                let lines = by_source.get(&key).cloned().unwrap_or_default();
                let finding = det.observe(&key, &lines);
                let source = key.strip_prefix(&prefix).unwrap_or(&key);
                conds.extend(alerts::log_anomaly(*id, &name(*id), source, &finding));
            }
        }
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "logs", hosts: Some(&covered) },
            conds,
            now,
        ));
    }
    changes
}

/// Error lines since `since` per source, keyed `<host>|journal/<ident>` and
/// `<host>|container/<name>`. `None` if the host couldn't be read at all.
async fn read_error_lines(app: &AppHandle, id: HostId, since: i64, now: i64) -> Option<HashMap<String, Vec<String>>> {
    let conn = crate::term_cmd::connect_host(app, id).await.ok()?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    if let Ok((_, text, _)) = conn.exec(&logwatch::journal_command(since)).await {
        for (ident, msg) in logwatch::parse_journal(&String::from_utf8_lossy(&text)) {
            out.entry(format!("{id}|journal/{ident}")).or_default().push(msg);
        }
    }
    let hub = app.state::<AppState>().alerts.clone();
    if hub.docker_skip.lock().expect("poisoned").get(&id).is_some_and(|until| *until > now) {
        return Some(out);
    }
    if let Ok(d) = crate::docker_cmd::docker_for(app, id).await {
        if let Ok(list) = d.containers().await {
            for c in list.iter().filter(|c| c.state == "running").take(LOGS_MAX_CONTAINERS) {
                let mut lines = Vec::new();
                let q = LogQuery { follow: false, tail: Some(500), since: Some(since.max(0) as u32) };
                let read = d.logs(&c.id, q, |batch| {
                    lines.extend(batch.into_iter().map(|l| l.text).filter(|t| logwatch::is_error_line(t)));
                    true
                });
                let _ = tokio::time::timeout(Duration::from_secs(10), read).await;
                out.insert(format!("{id}|container/{}", c.name), lines);
            }
        }
    }
    Some(out)
}

fn deliver(app: &AppHandle, cfg: &AppSettings, changes: &[Change], quiet: bool) {
    if quiet || changes.is_empty() {
        return;
    }
    let fired: Vec<&Alert> = changes
        .iter()
        .filter_map(|c| match c {
            Change::Fired(a) => Some(a),
            _ => None,
        })
        .collect();
    if cfg.alerts.native && !fired.is_empty() {
        if fired.len() > NOTIFY_BATCH {
            let crit = fired.iter().filter(|a| a.severity == Severity::Critical).count();
            let body = fired.iter().take(4).map(|a| alerts::headline(a)).collect::<Vec<_>>().join("\n");
            notify(app, &format!("NIGHTGRID · {} new alerts ({crit} critical)", fired.len()), &body);
        } else {
            for a in &fired {
                notify(app, &format!("NIGHTGRID · {}", a.kind.label()), &format!("{}: {}", a.title, a.detail));
            }
        }
    }
    let kind = cfg.alerts.webhook.clone();
    if kind == "none" {
        return;
    }
    let to_send: Vec<Alert> = changes
        .iter()
        .filter_map(|c| match c {
            Change::Fired(a) => Some(a.clone()),
            Change::Resolved(a) if cfg.alerts.webhook_resolved => Some(a.clone()),
            _ => None,
        })
        .take(WEBHOOK_MAX_PER_ROUND)
        .collect();
    if to_send.is_empty() {
        return;
    }
    let Some(url) = webhook_url(app) else { return };
    tauri::async_runtime::spawn(async move {
        for a in to_send {
            if let Some(req) = alerts::webhook_request(&kind, &url, &a) {
                match nightgrid_core::http::send(req).await {
                    Ok(r) if r.ok() => {}
                    Ok(r) => tracing::warn!(status = r.status, "alert webhook refused"),
                    Err(e) => tracing::warn!("alert webhook failed: {e}"),
                }
            }
        }
    });
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("notification failed: {e}");
    }
}

/// Registers the UI channel and sends the current snapshot.
#[tauri::command]
#[specta::specta]
pub fn alerts_subscribe(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<AlertsSnapshot>,
) -> CmdResult<()> {
    let cfg = settings(&app);
    let _ = on_event.send(state.alerts.snapshot(&cfg));
    *state.alerts.channel.lock().expect("poisoned") = Some(on_event);
    Ok(())
}

/// Re-evaluates now (after alert settings change).
#[tauri::command]
#[specta::specta]
pub fn alerts_refresh(state: State<'_, AppState>) -> CmdResult<()> {
    state.alerts.wake.notify_one();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn alerts_clear_history(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.alerts.tracker.lock().expect("poisoned").clear_history();
    state.alerts.push(&settings(&app));
    Ok(())
}

/// Recent error lines behind a log alert (key `logs:<host>:<source>:…`),
/// for "what broke". Secret-looking lines were never kept.
#[tauri::command]
#[specta::specta]
pub fn alerts_log_samples(state: State<'_, AppState>, key: String) -> CmdResult<Vec<String>> {
    let mut parts = key.splitn(3, ':');
    let (Some("logs"), Some(host), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
        return Ok(Vec::new());
    };
    let source = rest.rsplit_once(':').map_or(rest, |(s, _)| s);
    Ok(state.alerts.logs.lock().expect("poisoned").samples(&format!("{host}|{source}")))
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WebhookInfo {
    pub configured: bool,
    /// Scheme and host only (the path usually holds the token).
    pub host: Option<String>,
}

fn webhook_info(url: Option<&str>) -> WebhookInfo {
    let host = url.and_then(|u| {
        let (scheme, rest) = u.split_once("://")?;
        let host = rest.split(['/', '?', '#']).next()?;
        let host = host.rsplit('@').next()?;
        Some(format!("{scheme}://{host}"))
    });
    WebhookInfo { configured: url.is_some(), host }
}

#[tauri::command]
#[specta::specta]
pub fn alerts_webhook_get(app: AppHandle) -> CmdResult<WebhookInfo> {
    Ok(webhook_info(webhook_url(&app).as_deref()))
}

/// Saves the webhook URL in the keychain (`None` or empty removes it).
#[tauri::command]
#[specta::specta]
pub fn alerts_webhook_set(state: State<'_, AppState>, url: Option<String>) -> CmdResult<WebhookInfo> {
    let url = url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
    match &url {
        None => state.keychain.delete(&webhook_ref()).map_err(|e| CommandError::msg(e.to_string()))?,
        Some(u) => {
            let lower = u.to_ascii_lowercase();
            if !(lower.starts_with("https://") || lower.starts_with("http://")) || u.chars().any(char::is_whitespace) {
                return Err(CommandError::msg("webhook URL must start with https:// or http://"));
            }
            state.keychain.set(&webhook_ref(), &Secret::new(u.clone())).map_err(|e| CommandError::msg(e.to_string()))?
        }
    }
    Ok(webhook_info(url.as_deref()))
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub native: Option<String>,
    pub webhook: Option<String>,
}

/// Sends a test notification (and webhook message, if one is set up).
/// Ignores quiet hours. Returns a line per channel describing what happened.
#[tauri::command]
#[specta::specta]
pub async fn alerts_test(app: AppHandle) -> CmdResult<TestResult> {
    let cfg = settings(&app);
    let native = cfg.alerts.native.then(|| {
        match app.notification().builder().title("NIGHTGRID · TEST").body("Alerts reach this desktop.").show() {
            Ok(()) => "sent".to_owned(),
            Err(e) => format!("failed: {e}"),
        }
    });
    let webhook = if cfg.alerts.webhook == "none" {
        None
    } else if let Some(url) = webhook_url(&app) {
        let a = Alert {
            id: 0,
            key: "test".into(),
            kind: alerts::AlertKind::ServiceDown,
            severity: Severity::Warn,
            title: "test".into(),
            detail: "Test message from Nightgrid: the webhook works.".into(),
            host_id: None,
            since: now_unix(),
            resolved: None,
        };
        match alerts::webhook_request(&cfg.alerts.webhook, &url, &a) {
            Some(req) => Some(match nightgrid_core::http::send(req).await {
                Ok(r) if r.ok() => format!("delivered (HTTP {})", r.status),
                Ok(r) => format!("refused: HTTP {}", r.status),
                Err(e) => format!("failed: {e}"),
            }),
            None => None,
        }
    } else {
        Some("no URL saved".into())
    };
    Ok(TestResult { native, webhook })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_host_only() {
        let i = webhook_info(Some("https://discord.com/api/webhooks/123/secret-token"));
        assert_eq!(i.host.as_deref(), Some("https://discord.com"));
        let i = webhook_info(Some("http://user:pw@ntfy.lan:8080/topic"));
        assert_eq!(i.host.as_deref(), Some("http://ntfy.lan:8080"));
        assert!(!webhook_info(None).configured);
    }
}
