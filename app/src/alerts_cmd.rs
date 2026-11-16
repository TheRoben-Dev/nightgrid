//! Alerts: one background loop that reads what the app already watches
//! (metrics poll states and mounts, service checks), polls Docker on
//! connected hosts every minute and SMART every six hours, runs it through
//! the core [`Tracker`], and delivers changes: the UI snapshot, the tray,
//! native notifications and the optional webhook.
//!
//! The loop never opens new SSH connections for Docker or SMART: it only
//! uses hosts already in the pool. SMART only runs where sudo needs no
//! prompt (root, NOPASSWD, or a password the user chose to remember).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use nightgrid_core::alerts::{self, Alert, Change, ContainerWatch, Scope, Severity, Tracker};
use nightgrid_core::backup_jobs;
use nightgrid_core::depgraph::{self, Folding, Graph};
use nightgrid_core::docker::LogQuery;
use nightgrid_core::hosts::HostId;
use nightgrid_core::logwatch::{self, Detector, NoteWhy, RawLine, SourceCtx};
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
const POOL_EVERY: i64 = 30 * 60;
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
/// Longest a notification waits for an upstream alert that may still fire.
const HOLD_MAX: i64 = 300;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AlertsSnapshot {
    pub active: Vec<Alert>,
    pub history: Vec<Alert>,
    /// Quiet hours are on right now (no notifications or webhooks).
    pub quiet: bool,
    pub enabled: bool,
    /// A UPS shutdown waiting to start (M10s), shown with a Cancel button.
    pub ups_countdown: Option<crate::ups_cmd::UpsCountdown>,
    /// New log errors that were only noted (once, or right after a restart).
    pub log_notes: Vec<LogNote>,
    /// Log events left out by a mute or as 404 noise, per source, since the app started.
    pub log_quiet: Vec<LogQuiet>,
}

/// A new log error that didn't alert: seen once, or right after a restart.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LogNote {
    pub host_id: HostId,
    /// `container/<name>` or `journal/<ident>`.
    pub source: String,
    pub fingerprint: String,
    /// The event's first lines (None if they looked like they held a secret).
    pub sample: Option<String>,
    pub count: u32,
    pub why: NoteWhy,
    #[specta(type = u32)]
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LogQuiet {
    pub host_id: HostId,
    pub source: String,
    pub muted: u32,
    pub not_found: u32,
}

/// Notes kept, and for how long.
const NOTES_MAX: usize = 60;
const NOTES_FOR: i64 = 24 * 3600;

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
    pool_last: Mutex<HashMap<HostId, i64>>,
    logs_last: Mutex<HashMap<HostId, i64>>,
    logs: Mutex<Detector>,
    /// Hosts whose stored log patterns are loaded into the detector.
    logs_seeded: Mutex<HashSet<HostId>>,
    log_notes: Mutex<VecDeque<LogNote>>,
    log_quiet: Mutex<HashMap<(HostId, String), (u32, u32)>>,
    /// Which active alerts are explained by an outage upstream (M10s).
    folding: Mutex<Folding>,
    /// Fired alerts whose notification waits for a cause in its grace period.
    held: Mutex<HashMap<u32, (Alert, i64)>>,
    /// Alerts that were folded and never notified (no "resolved" either).
    silent: Mutex<HashSet<u32>>,
    /// Copy of the UPS countdown for the snapshot.
    ups_countdown: Mutex<Option<crate::ups_cmd::UpsCountdown>>,
}

impl AlertsHub {
    /// Re-evaluates soon (settings changed).
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Active alerts right now.
    pub fn active(&self) -> Vec<Alert> {
        self.tracker.lock().expect("poisoned").active()
    }

    /// Resolved alerts, newest first.
    pub fn history(&self) -> Vec<Alert> {
        self.tracker.lock().expect("poisoned").history()
    }

    /// Active alerts and critical ones, not counting those folded under a cause.
    pub fn active_count(&self) -> (usize, usize) {
        let t = self.tracker.lock().expect("poisoned");
        let folding = self.folding.lock().expect("poisoned");
        let act: Vec<Alert> = t.active().into_iter().filter(|a| !folding.caused_by.contains_key(&a.id)).collect();
        (act.len(), act.iter().filter(|a| a.severity == Severity::Critical).count())
    }

    fn snapshot(&self, settings: &AppSettings) -> AlertsSnapshot {
        let t = self.tracker.lock().expect("poisoned");
        let folding = self.folding.lock().expect("poisoned");
        let mut active = t.active();
        for a in &mut active {
            a.caused_by = folding.caused_by.get(&a.id).copied();
        }
        AlertsSnapshot {
            active,
            history: t.history(),
            quiet: is_quiet(settings),
            enabled: settings.alerts.enabled,
            ups_countdown: self.ups_countdown.lock().expect("poisoned").clone(),
            log_notes: self.log_notes.lock().expect("poisoned").iter().cloned().collect(),
            log_quiet: self
                .log_quiet
                .lock()
                .expect("poisoned")
                .iter()
                .map(|((h, src), (m, nf))| LogQuiet { host_id: *h, source: src.clone(), muted: *m, not_found: *nf })
                .collect(),
        }
    }

    fn push(&self, settings: &AppSettings) {
        let snap = self.snapshot(settings);
        if let Some(ch) = self.channel.lock().expect("poisoned").as_ref() {
            let _ = ch.send(snap);
        }
    }
}

impl AlertsHub {
    /// Held notifications that can go out now: the upstream alert didn't
    /// fire (or took too long). Those whose cause fired are dropped silently.
    fn release_held(&self, now: i64) -> Vec<Alert> {
        let active: HashSet<u32> = self.tracker.lock().expect("poisoned").active().iter().map(|a| a.id).collect();
        let folding = self.folding.lock().expect("poisoned").clone();
        let mut held = self.held.lock().expect("poisoned");
        let mut out = Vec::new();
        held.retain(|id, (a, since)| {
            if !active.contains(id) {
                return false;
            }
            if folding.caused_by.contains_key(id) {
                self.silent.lock().expect("poisoned").insert(*id);
                return false;
            }
            if !folding.waiting.contains(id) || now - *since >= HOLD_MAX {
                out.push(a.clone());
                return false;
            }
            true
        });
        out
    }
}

/// Recomputes which alerts are folded under a cause. True if that changed.
fn refold(app: &AppHandle, cfg: &AppSettings) -> bool {
    let state = app.state::<AppState>();
    let hub = state.alerts.clone();
    let next = if cfg.alerts.enabled && cfg.alerts.fold_dependent {
        let (deps, services) = {
            let store = state.store();
            (store.deps_list().unwrap_or_default(), store.list_services().unwrap_or_default())
        };
        let on_host: HashMap<_, _> = services.iter().filter_map(|s| s.host_id.map(|h| (s.id, h))).collect();
        let graph = Graph::new(&deps, &on_host);
        let t = hub.tracker.lock().expect("poisoned");
        depgraph::fold(&t.active(), &t.pending(), &graph)
    } else {
        Folding::default()
    };
    let mut cur = hub.folding.lock().expect("poisoned");
    let changed = *cur != next;
    *cur = next;
    changed
}

/// Sends a fresh snapshot now (the UPS countdown started or stopped).
pub fn push_now(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.alerts.ups_countdown.lock().expect("poisoned") = state.ups.countdown();
    state.alerts.push(&settings(app));
    crate::tray::refresh(app);
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
            // Features turned off in Settings → Features do no background work.
            let cfg = settings(&app).effective();
            let now = now_unix();
            let changes = evaluate(&app, &cfg, now, &mut last_docker).await;
            crate::timeline_cmd::record_alerts(&app, &changes);
            let quiet = is_quiet(&cfg);
            let hub = app.state::<AppState>().alerts.clone();
            let refolded = refold(&app, &cfg);
            let released = hub.release_held(now);
            if !changes.is_empty() || !released.is_empty() || refolded || last_quiet != Some(quiet) {
                deliver(&app, &cfg, &changes, released, quiet, now);
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
    // Uptime history (M10t) is recorded whether alerts are on or not.
    let polls = if cfg.metrics_enabled { state.metrics.poll_states() } else { HashMap::new() };
    if cfg.feature("fleet.uptime") {
        crate::uptime_cmd::record(app, &polls, cfg.services_checks, u32::from(cfg.services_interval_secs), now);
    }
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

    // Expiry watch (M10l): dates are in the local database; certificate
    // files are re-read from connected hosts twice a day.
    let watched = state.store().watch_list().unwrap_or_default();
    let expiry_on = cfg.feature("services.expiry");
    let expiring =
        if rules.cert_expiring && expiry_on { nightgrid_core::expiry::conditions(&watched, now) } else { vec![] };
    changes.extend(hub.tracker.lock().expect("poisoned").report(
        &Scope { source: "expiry", hosts: None },
        expiring,
        now,
    ));
    if expiry_on {
        crate::expiry_cmd::refresh_due(app, &watched, &connected, now);
    }

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

    // Disk fill-up forecasts, from usage recorded while monitoring.
    {
        let mut conds = Vec::new();
        if rules.disk_forecast {
            for id in hosts.keys() {
                let list = state.store().disk_forecasts(*id, now).unwrap_or_default();
                conds.extend(nightgrid_core::diskforecast::conditions(
                    *id,
                    &name(*id),
                    &list,
                    rules.forecast_days,
                    now,
                ));
            }
        }
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "fill", hosts: None },
            conds,
            now,
        ));
    }

    // Storage pools on connected hosts, every 30 minutes each (no root).
    if !rules.pool_degraded {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "pool", hosts: None },
            vec![],
            now,
        ));
    } else {
        let due: Vec<HostId> = {
            let last = hub.pool_last.lock().expect("poisoned");
            connected.iter().copied().filter(|id| !matches!(last.get(id), Some(t) if now - t < POOL_EVERY)).collect()
        };
        if !due.is_empty() {
            let mut covered = HashSet::new();
            let mut conds = Vec::new();
            for id in due {
                hub.pool_last.lock().expect("poisoned").insert(id, now);
                let scan = async {
                    let conn = crate::term_cmd::connect_host(app, id).await.ok()?;
                    nightgrid_core::system::pools::scan(&conn).await.ok()
                };
                if let Ok(Some(report)) = tokio::time::timeout(Duration::from_secs(30), scan).await {
                    covered.insert(id);
                    conds.extend(nightgrid_core::system::pools::conditions(id, &name(id), &report));
                }
            }
            changes.extend(hub.tracker.lock().expect("poisoned").report(
                &Scope { source: "pool", hosts: Some(&covered) },
                conds,
                now,
            ));
        }
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

    // UPS watches (M10s): readings, alerts and the shutdown countdown.
    if cfg.feature("fleet.ups") {
        let (conds, covered) = crate::ups_cmd::evaluate(app, &connected, now).await;
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "ups", hosts: Some(&covered) },
            conds,
            now,
        ));
    } else {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "ups", hosts: None },
            vec![],
            now,
        ));
    }

    // Saved log searches with alerts on (M10u), every 5 minutes.
    if !cfg.feature("fleet.logs") {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "logsearch", hosts: None },
            vec![],
            now,
        ));
    } else if let Some((conds, covered)) = crate::logsearch_cmd::evaluate(app, &connected, now).await {
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "logsearch", hosts: covered.as_ref() },
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
        let lrules = logwatch::Rules::new(
            usize::from(rules.log_repeat_count),
            i64::from(rules.log_repeat_minutes) * 60,
            i64::from(rules.log_grace_minutes) * 60,
            rules.log_not_found_noise,
            &rules.log_mutes,
        );
        for id in &connected {
            let since = hub.logs_last.lock().expect("poisoned").get(id).copied();
            if since.is_some_and(|t| now - t < LOGS_EVERY) {
                continue;
            }
            let since = since.unwrap_or(now - LOGS_LEARN);
            let read = read_log_lines(app, *id, since, now);
            let Ok(Some(by_source)) = tokio::time::timeout(Duration::from_secs(90), read).await else { continue };
            hub.logs_last.lock().expect("poisoned").insert(*id, now);
            covered.insert(*id);
            let prefix = format!("{id}|");
            if hub.logs_seeded.lock().expect("poisoned").insert(*id) {
                let stored = app.state::<AppState>().store().log_patterns(*id).unwrap_or_default();
                let mut by: HashMap<String, Vec<(String, i64)>> = HashMap::new();
                for (src, fp, at) in stored {
                    by.entry(src).or_default().push((fp, at));
                }
                let mut det = hub.logs.lock().expect("poisoned");
                for (src, known) in by {
                    det.seed(&format!("{prefix}{src}"), known);
                }
            }
            let mut det = hub.logs.lock().expect("poisoned");
            let mut sources: Vec<String> = by_source.keys().cloned().collect();
            for known in det.sources_with_prefix(&prefix) {
                if !sources.contains(&known) {
                    sources.push(known);
                }
            }
            let mut notes = Vec::new();
            for key in sources {
                let (lines, ctx) = by_source.get(&key).cloned().unwrap_or_default();
                let finding = det.observe(&key, &lines, &ctx, &lrules, now);
                let source = key.strip_prefix(&prefix).unwrap_or(&key).to_owned();
                if finding.muted + finding.not_found > 0 {
                    let mut q = hub.log_quiet.lock().expect("poisoned");
                    let e = q.entry((*id, source.clone())).or_default();
                    e.0 += finding.muted as u32;
                    e.1 += finding.not_found as u32;
                }
                for (n, why) in &finding.notes {
                    notes.push(LogNote {
                        host_id: *id,
                        source: source.clone(),
                        fingerprint: n.fingerprint.clone(),
                        sample: n.sample.clone(),
                        count: n.count as u32,
                        why: *why,
                        at: now,
                    });
                }
                conds.extend(alerts::log_anomaly(*id, &name(*id), &source, &finding));
            }
            let learned: Vec<(String, String, i64)> = det
                .take_learned()
                .into_iter()
                .filter_map(|(k, fp, at)| k.strip_prefix(&prefix).map(|s| (s.to_owned(), fp, at)))
                .collect();
            drop(det);
            if !learned.is_empty() {
                if let Err(e) = app.state::<AppState>().store().log_patterns_add(*id, &learned) {
                    tracing::warn!("storing log patterns: {e}");
                }
            }
            let mut kept = hub.log_notes.lock().expect("poisoned");
            for n in notes {
                // One note per pattern: a newer sighting replaces the older one.
                kept.retain(|o| !(o.host_id == n.host_id && o.source == n.source && o.fingerprint == n.fingerprint));
                kept.push_front(n);
            }
            kept.retain(|n| now - n.at <= NOTES_FOR);
            kept.truncate(NOTES_MAX);
        }
        changes.extend(hub.tracker.lock().expect("poisoned").report(
            &Scope { source: "logs", hosts: Some(&covered) },
            conds,
            now,
        ));
    }
    changes
}

/// Log lines since `since` per source with what's known about the source,
/// keyed `<host>|journal/<ident>` and `<host>|container/<name>`. `None` if
/// the host couldn't be read at all.
async fn read_log_lines(
    app: &AppHandle,
    id: HostId,
    since: i64,
    now: i64,
) -> Option<HashMap<String, (Vec<RawLine>, SourceCtx)>> {
    let conn = crate::term_cmd::connect_host(app, id).await.ok()?;
    let mut out: HashMap<String, (Vec<RawLine>, SourceCtx)> = HashMap::new();
    if let Ok((_, text, _)) = conn.exec(&logwatch::journal_command(since)).await {
        for (ident, msg) in logwatch::parse_journal(&String::from_utf8_lossy(&text)) {
            let e = out
                .entry(format!("{id}|journal/{ident}"))
                .or_insert_with(|| (Vec::new(), SourceCtx { journal: true, ..Default::default() }));
            e.0.push(RawLine::new(None, msg));
        }
    }
    let hub = app.state::<AppState>().alerts.clone();
    if hub.docker_skip.lock().expect("poisoned").get(&id).is_some_and(|until| *until > now) {
        return Some(out);
    }
    if let Ok(d) = crate::docker_cmd::docker_for(app, id).await {
        if let Ok(list) = d.containers().await {
            let started = |status: &str| logwatch::uptime_from_status(status).map(|up| now - up);
            // The newest reverse-proxy start on the host: every site behind it reloads.
            let proxy = list
                .iter()
                .filter(|c| c.state == "running" && logwatch::is_reverse_proxy(&c.image))
                .filter_map(|c| started(&c.status))
                .max();
            for c in list.iter().filter(|c| c.state == "running" || c.state == "restarting").take(LOGS_MAX_CONTAINERS) {
                let mut lines = Vec::new();
                let q = LogQuery { follow: false, tail: Some(500), since: Some(since.max(0) as u32) };
                let read = d.logs(&c.id, q, |batch| {
                    lines.extend(
                        batch
                            .into_iter()
                            .map(|l| RawLine::new(l.ts.as_deref().and_then(logwatch::parse_rfc3339), l.text)),
                    );
                    true
                });
                let _ = tokio::time::timeout(Duration::from_secs(10), read).await;
                let is_proxy = logwatch::is_reverse_proxy(&c.image);
                let ctx = SourceCtx {
                    journal: false,
                    started_at: started(&c.status),
                    proxy_started_at: if is_proxy { None } else { proxy },
                    unhealthy: c.health.as_deref() == Some("unhealthy"),
                    restarting: c.state == "restarting",
                };
                out.insert(format!("{id}|container/{}", c.name), (lines, ctx));
            }
        }
    }
    Some(out)
}

fn deliver(app: &AppHandle, cfg: &AppSettings, changes: &[Change], released: Vec<Alert>, quiet: bool, now: i64) {
    let hub = app.state::<AppState>().alerts.clone();
    let folding = hub.folding.lock().expect("poisoned").clone();
    // Alerts explained by an outage upstream stay quiet; those whose cause
    // may still fire wait a little (M10s).
    let mut fired: Vec<Alert> = released;
    let mut folded_under: HashMap<u32, usize> = HashMap::new();
    for c in changes {
        if let Change::Fired(a) = c {
            if let Some(cause) = folding.caused_by.get(&a.id) {
                hub.silent.lock().expect("poisoned").insert(a.id);
                *folded_under.entry(*cause).or_default() += 1;
            } else if folding.waiting.contains(&a.id) {
                hub.held.lock().expect("poisoned").insert(a.id, (a.clone(), now));
            } else {
                fired.push(a.clone());
            }
        }
    }
    let mut resolved = Vec::new();
    for c in changes {
        if let Change::Resolved(a) = c {
            hub.held.lock().expect("poisoned").remove(&a.id);
            if !hub.silent.lock().expect("poisoned").remove(&a.id) {
                resolved.push(a.clone());
            }
        }
    }
    if quiet || (fired.is_empty() && resolved.is_empty()) {
        return;
    }
    let explains = |a: &Alert| match folded_under.get(&a.id) {
        Some(n) => format!(" (and {n} alert{} it explains)", if *n == 1 { "" } else { "s" }),
        None => String::new(),
    };
    if cfg.alerts.native && !fired.is_empty() {
        if fired.len() > NOTIFY_BATCH {
            let crit = fired.iter().filter(|a| a.severity == Severity::Critical).count();
            let body = fired.iter().take(4).map(alerts::headline).collect::<Vec<_>>().join("\n");
            notify(app, &format!("NIGHTGRID · {} new alerts ({crit} critical)", fired.len()), &body);
        } else {
            for a in &fired {
                notify(
                    app,
                    &format!("NIGHTGRID · {}", a.kind.label()),
                    &format!("{}: {}{}", a.title, a.detail, explains(a)),
                );
            }
        }
    }
    let kind = cfg.alerts.webhook.clone();
    if kind == "none" {
        return;
    }
    let to_send: Vec<Alert> = fired
        .into_iter()
        .chain(resolved.into_iter().filter(|_| cfg.alerts.webhook_resolved))
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

/// What depends on what (M10s).
#[tauri::command]
#[specta::specta]
pub fn alerts_deps_list(state: State<'_, AppState>) -> CmdResult<Vec<depgraph::Dependency>> {
    Ok(state.store().deps_list()?)
}

/// Adds "`child` depends on `parent`" (refuses loops).
#[tauri::command]
#[specta::specta]
pub fn alerts_deps_add(
    state: State<'_, AppState>,
    child: depgraph::Node,
    parent: depgraph::Node,
) -> CmdResult<depgraph::Dependency> {
    let d = state.store().deps_add(child, parent)?;
    state.alerts.wake();
    Ok(d)
}

#[tauri::command]
#[specta::specta]
pub fn alerts_deps_delete(state: State<'_, AppState>, id: depgraph::DepId) -> CmdResult<()> {
    state.store().deps_delete(id)?;
    state.alerts.wake();
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
    let source = rest
        .split_once(":new:")
        .map(|(s, _)| s)
        .or_else(|| rest.strip_suffix(":burst"))
        .or_else(|| rest.strip_suffix(":404"))
        .unwrap_or(rest);
    Ok(state.alerts.logs.lock().expect("poisoned").samples(&format!("{host}|{source}")))
}

/// One event from attached log lines, judged by the same rules as the log alerts.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LogEventView {
    /// The first line.
    pub head: String,
    pub lines: u32,
    /// debug, info, warn, error or fatal.
    pub level: String,
    pub not_found: bool,
    /// Why it is likely harmless under the rules, or None.
    pub harmless: Option<String>,
}

/// Groups log text into events and says which are likely harmless (WARN and
/// lower, one-off 404s), for DRIFT's triage.
#[tauri::command]
#[specta::specta]
pub fn log_classify(text: String) -> Vec<LogEventView> {
    let lines: Vec<RawLine> = text.lines().take(2000).map(|l| RawLine::new(None, l)).collect();
    logwatch::events(&lines, false)
        .into_iter()
        .take(100)
        .map(|e| {
            let level = match e.level {
                logwatch::Level::Debug => "debug",
                logwatch::Level::Info => "info",
                logwatch::Level::Warn => "warn",
                logwatch::Level::Error => "error",
                logwatch::Level::Fatal => "fatal",
            };
            let harmless = if e.level == logwatch::Level::Fatal {
                None
            } else if e.not_found {
                Some(
                    "an HTTP 404 / NOT_FOUND: a request for something that doesn't exist, not a fault in the app"
                        .to_owned(),
                )
            } else if e.level < logwatch::Level::Error {
                Some(format!("logged at {} level, not as an error", level.to_uppercase()))
            } else {
                None
            };
            LogEventView {
                head: e.head.chars().take(300).collect(),
                lines: e.lines as u32,
                level: level.into(),
                not_found: e.not_found,
                harmless,
            }
        })
        .collect()
}

/// "Mute this pattern": adds the pattern's fingerprint to the mute list for
/// its source, so it never alerts or is noted again (it's still counted).
/// Returns the updated settings.
#[tauri::command]
#[specta::specta]
pub fn alerts_log_mute(
    state: State<'_, AppState>,
    host_id: HostId,
    source: String,
    fingerprint: String,
) -> CmdResult<AppSettings> {
    let pattern = state.alerts.logs.lock().expect("poisoned").pattern_of(&format!("{host_id}|{source}"), &fingerprint);
    let label = pattern
        .filter(|p| !nightgrid_core::redact::line_has_secret(p))
        .map(|p| p.chars().take(160).collect::<String>())
        .unwrap_or_else(|| format!("pattern {fingerprint}"));
    let mute =
        logwatch::LogMute { source: source.clone(), regex: String::new(), fingerprint: fingerprint.clone(), label };
    if let Some(p) = mute.problem() {
        return Err(nightgrid_core::Error::Invalid { field: "fingerprint", reason: p }.into());
    }
    let store = state.store();
    let mut s = AppSettings::load(&store)?;
    if !s.alerts.log_mutes.iter().any(|m| m.source == source && m.fingerprint == fingerprint) {
        s.alerts.log_mutes.push(mute);
        s.save(&store)?;
    }
    drop(store);
    state.alerts.log_notes.lock().expect("poisoned").retain(|n| !(n.source == source && n.fingerprint == fingerprint));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    state
        .alerts
        .tracker
        .lock()
        .expect("poisoned")
        .resolve_key(&format!("logs:{host_id}:{source}:new:{fingerprint}"), now);
    state.alerts.push(&s);
    state.alerts.wake();
    Ok(s)
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
            caused_by: None,
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
