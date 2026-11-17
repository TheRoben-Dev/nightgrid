//! Host metrics: one polling task per saved host, sampling over the host's
//! pooled SSH connection (see `nightgrid_core::metrics`). Hosts on screen
//! are sampled every `metrics_interval_secs`, the rest every
//! `metrics_background_secs`; offline hosts back off; hosts that need a
//! password or a host-key decision wait until the user connects them.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::metrics::history::{MinuteAverager, Point};
use nightgrid_core::metrics::{parse, Collector, MountUsage, Sample, COLLECT};
use nightgrid_core::settings::AppSettings;
use nightgrid_core::ssh::client::ConnectError;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Notify;

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::term_cmd::connect_host;

/// Why a host has no fresh sample.
#[derive(Debug, Clone, Serialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PollState {
    /// Samples are arriving.
    Ok,
    /// Connecting / first sample on its way.
    Connecting,
    /// Can't reach it; retrying with backoff.
    Offline,
    /// Needs a password, passphrase or host-key decision: open a terminal
    /// (or Files/Docker) on it once and metrics start.
    NeedsLogin,
    /// Connected, but the collect script failed (not Linux, no /proc, ...).
    Error,
    /// Metrics are turned off in settings.
    Off,
    /// Doesn't answer, but it was suspended (from Nightgrid, or the host
    /// announced sleep). No offline alert. The message says why.
    Suspended,
    /// Doesn't answer while it reboots (gives up after 15 minutes).
    Rebooting,
    /// Doesn't answer after a shutdown.
    ShuttingDown,
    /// Doesn't answer inside its expected sleep window.
    Asleep,
    /// A client device (laptop, handheld, desktop) that doesn't answer: it
    /// comes and goes, so no offline alert and not counted as down.
    Away,
}

impl PollState {
    fn planned(s: nightgrid_core::power::PowerState) -> Self {
        use nightgrid_core::power::PowerState as P;
        match s {
            P::Suspended => Self::Suspended,
            P::Rebooting => Self::Rebooting,
            P::ShuttingDown => Self::ShuttingDown,
            P::Asleep => Self::Asleep,
        }
    }
}

pub(crate) fn local_minutes() -> u16 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    (now.hour() * 60 + now.minute()) as u16
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Follows logind's sleep/shutdown signals on the host's pooled connection,
/// so a host that goes to sleep shows "Suspended", not "Offline".
fn watch_power(app: AppHandle, host_id: HostId) -> tauri::async_runtime::JoinHandle<()> {
    use nightgrid_core::power::{self, Watch};
    tauri::async_runtime::spawn(async move {
        let Ok(conn) = connect_host(&app, host_id).await else { return };
        power::watch(&conn, |w| match w {
            Watch::Sleep => {
                tracing::info!(host = %host_id, "host announced sleep");
                power::note(
                    host_id,
                    power::PowerState::Suspended,
                    "the host announced it was going to sleep",
                    now_secs(),
                );
            }
            Watch::Shutdown => {
                tracing::info!(host = %host_id, "host announced shutdown");
                power::note(
                    host_id,
                    power::PowerState::ShuttingDown,
                    "the host announced it was shutting down or rebooting",
                    now_secs(),
                );
            }
            Watch::Resumed => power::clear(host_id),
        })
        .await;
    })
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MetricsEvent {
    Sample { host_id: HostId, sample: Box<Sample> },
    State { host_id: HostId, state: PollState, message: Option<String>, retry_secs: Option<u32> },
}

struct Poller {
    wake: Arc<Notify>,
    task: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Default)]
pub struct MetricsHub {
    pollers: Mutex<HashMap<HostId, Poller>>,
    visible: Mutex<HashSet<HostId>>,
    channel: Mutex<Option<Channel<MetricsEvent>>>,
    /// Last state per host, re-sent to a new subscriber.
    states: Mutex<HashMap<HostId, MetricsEvent>>,
    pruner: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// Latest sample per host, for disk alerts and the status page.
    latest: Mutex<HashMap<HostId, Box<Sample>>>,
}

impl MetricsHub {
    fn send(&self, ev: MetricsEvent) {
        match &ev {
            MetricsEvent::State { host_id, .. } => {
                self.states.lock().expect("poisoned").insert(*host_id, ev.clone());
            }
            MetricsEvent::Sample { host_id, sample } => {
                self.latest.lock().expect("poisoned").insert(*host_id, sample.clone());
            }
        }
        if let Some(ch) = self.channel.lock().expect("poisoned").as_ref() {
            let _ = ch.send(ev);
        }
    }

    /// Poll state (and its message) per host that has a poller.
    pub fn poll_states(&self) -> HashMap<HostId, (PollState, Option<String>)> {
        self.states
            .lock()
            .expect("poisoned")
            .iter()
            .filter_map(|(id, ev)| match ev {
                MetricsEvent::State { state, message, .. } => Some((*id, (state.clone(), message.clone()))),
                MetricsEvent::Sample { .. } => None,
            })
            .collect()
    }

    pub fn mounts(&self, id: HostId) -> Option<Vec<MountUsage>> {
        self.latest.lock().expect("poisoned").get(&id).map(|s| s.mounts.clone())
    }

    /// The latest sample of a host.
    pub fn latest(&self, id: HostId) -> Option<Box<Sample>> {
        self.latest.lock().expect("poisoned").get(&id).cloned()
    }

    fn is_visible(&self, id: HostId) -> bool {
        self.visible.lock().expect("poisoned").contains(&id)
    }

    fn wake_all(&self) {
        for p in self.pollers.lock().expect("poisoned").values() {
            p.wake.notify_one();
        }
    }
}

fn settings(app: &AppHandle) -> AppSettings {
    AppSettings::load(&app.state::<AppState>().store()).unwrap_or_default()
}

fn needs_login(e: &ConnectError) -> bool {
    matches!(
        e,
        ConnectError::HostKeyUnknown { .. }
            | ConnectError::HostKeyChanged { .. }
            | ConnectError::PassphraseRequired { .. }
            | ConnectError::PasswordRequired { .. }
            | ConnectError::AuthFailed { .. }
    )
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

async fn poll_host(app: AppHandle, host_id: HostId, wake: Arc<Notify>) {
    let hub = || app.state::<AppState>().metrics.clone();
    let mut collector = Collector::new();
    let mut avg = MinuteAverager::default();
    let mut backoff = 0u32;
    let mut state: Option<PollState> = None;
    let set_state = |state: &mut Option<PollState>, s: PollState, message: Option<String>, retry: Option<u32>| {
        if state.as_ref() != Some(&s) || message.is_some() {
            hub().send(MetricsEvent::State { host_id, state: s.clone(), message, retry_secs: retry });
            *state = Some(s);
        }
    };
    set_state(&mut state, PollState::Connecting, None, None);
    let mut watcher: Option<tauri::async_runtime::JoinHandle<()>> = None;
    // Disk usage for the forecast: at most every ten minutes (one row per hour is kept).
    let mut usage_at = 0i64;
    loop {
        let cfg = settings(&app);
        let pause = if hub().is_visible(host_id) { cfg.metrics_interval_secs } else { cfg.metrics_background_secs };
        let waiting_for_login = state == Some(PollState::NeedsLogin);
        let wait_secs: u32 = if waiting_for_login {
            // Only resume once the user has connected the host some other way.
            if app.state::<AppState>().pool.is_connected(host_id) {
                0
            } else {
                5
            }
        } else if backoff > 0 {
            backoff
        } else {
            0
        };
        if wait_secs > 0 {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(u64::from(wait_secs))) => {}
                _ = wake.notified() => {}
            }
            if waiting_for_login && !app.state::<AppState>().pool.is_connected(host_id) {
                continue;
            }
        }
        let started = std::time::Instant::now();
        let result = async {
            let conn = connect_host(&app, host_id).await?;
            let (code, out, err) = tokio::time::timeout(Duration::from_secs(15), conn.exec(COLLECT))
                .await
                .map_err(|_| ConnectError::Network { host: String::new(), message: "sample timed out".into() })??;
            Ok::<_, ConnectError>((code, out, err))
        }
        .await;
        match result {
            Ok((_, out, err)) => {
                let text = String::from_utf8_lossy(&out);
                let raw = parse(&text);
                if raw.cpu.is_none() && raw.mem.total == 0 {
                    let msg = String::from_utf8_lossy(&err).lines().next().unwrap_or("no /proc data").to_owned();
                    set_state(&mut state, PollState::Error, Some(format!("can't read /proc: {msg}")), None);
                    backoff = 300;
                    continue;
                }
                backoff = 0;
                nightgrid_core::power::clear(host_id);
                if watcher.as_ref().map_or(true, |w| w.inner().is_finished()) {
                    watcher = Some(watch_power(app.clone(), host_id));
                }
                let sample = collector.sample(raw, now_ms());
                set_state(&mut state, PollState::Ok, None, None);
                if let Some(done) = avg.push(&Point::from_sample(&sample)) {
                    if let Err(e) = app.state::<AppState>().store().insert_metric_point(host_id, &done) {
                        tracing::warn!(host = %host_id, "metric history: {e}");
                    }
                }
                let now = now_secs();
                if now - usage_at >= 600 && !sample.mounts.is_empty() {
                    usage_at = now;
                    if let Err(e) = app.state::<AppState>().store().disk_usage_record(host_id, &sample.mounts, now) {
                        tracing::warn!(host = %host_id, "disk usage history: {e}");
                    }
                }
                hub().send(MetricsEvent::Sample { host_id, sample: Box::new(sample) });
            }
            Err(e) if needs_login(&e) => {
                set_state(&mut state, PollState::NeedsLogin, Some(e.to_string()), None);
                continue;
            }
            Err(e) => {
                backoff = (backoff * 2).clamp(5, 300);
                collector = Collector::new();
                if let Some(w) = watcher.take() {
                    w.abort();
                }
                let host = app.state::<AppState>().store().get_host(host_id).ok().flatten();
                let window = host
                    .as_ref()
                    .and_then(|h| h.sleep_window.as_deref())
                    .and_then(|w| nightgrid_core::power::SleepWindow::parse(w).ok());
                let away = host.as_ref().is_some_and(|h| h.away_when_unreachable());
                match nightgrid_core::power::current(host_id, window.as_ref(), local_minutes(), now_secs()) {
                    Some(n) => set_state(&mut state, PollState::planned(n.state), Some(n.reason), Some(backoff)),
                    None if away => set_state(
                        &mut state,
                        PollState::Away,
                        Some(format!("client device, not answering ({e})")),
                        Some(backoff),
                    ),
                    None => set_state(&mut state, PollState::Offline, Some(e.to_string()), Some(backoff)),
                }
                continue;
            }
        }
        let elapsed = started.elapsed();
        let target = Duration::from_secs(u64::from(pause.max(1)));
        if let Some(rest) = target.checked_sub(elapsed) {
            tokio::select! {
                _ = tokio::time::sleep(rest) => {}
                _ = wake.notified() => {}
            }
        }
    }
}

/// Starts or stops pollers so there is one per saved host (all of them off
/// when metrics are disabled).
fn sync_pollers(app: &AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let hub = state.metrics.clone();
    let enabled = settings(app).metrics_enabled;
    let ids: HashSet<HostId> =
        if enabled { state.store().list_hosts()?.into_iter().map(|h| h.id).collect() } else { HashSet::new() };
    let mut pollers = hub.pollers.lock().expect("poisoned");
    pollers.retain(|id, p| {
        let keep = ids.contains(id);
        if !keep {
            p.task.abort();
            hub.states.lock().expect("poisoned").remove(id);
            hub.latest.lock().expect("poisoned").remove(id);
            if let Some(ch) = hub.channel.lock().expect("poisoned").as_ref() {
                let _ = ch.send(MetricsEvent::State {
                    host_id: *id,
                    state: PollState::Off,
                    message: None,
                    retry_secs: None,
                });
            }
        }
        keep
    });
    for id in ids {
        pollers.entry(id).or_insert_with(|| {
            let wake = Arc::new(Notify::new());
            let task = tauri::async_runtime::spawn(poll_host(app.clone(), id, wake.clone()));
            Poller { wake, task }
        });
    }
    Ok(())
}

/// Subscribes the UI to samples and states, starts the pollers and the
/// hourly history pruning. Calling it again replaces the channel.
#[tauri::command]
#[specta::specta]
pub fn metrics_subscribe(app: AppHandle, state: State<'_, AppState>, on_event: Channel<MetricsEvent>) -> CmdResult<()> {
    let hub = state.metrics.clone();
    for ev in hub.states.lock().expect("poisoned").values() {
        let _ = on_event.send(ev.clone());
    }
    *hub.channel.lock().expect("poisoned") = Some(on_event);
    let mut pruner = hub.pruner.lock().expect("poisoned");
    if pruner.is_none() {
        let app2 = app.clone();
        *pruner = Some(tauri::async_runtime::spawn(async move {
            loop {
                let days = settings(&app2).metrics_retention_days;
                let now = (now_ms() / 1000.0) as i64;
                if let Err(e) = app2.state::<AppState>().store().prune_metrics(now, u32::from(days)) {
                    tracing::warn!("metric pruning: {e}");
                }
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        }));
    }
    drop(pruner);
    sync_pollers(&app)
}

/// Re-reads the host list and settings (after hosts were added/removed or
/// metrics were turned on/off) and wakes every poller.
#[tauri::command]
#[specta::specta]
pub fn metrics_sync(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    sync_pollers(&app)?;
    state.metrics.wake_all();
    Ok(())
}

/// Hosts currently on screen (sampled fast). An empty list = window hidden.
#[tauri::command]
#[specta::specta]
pub fn metrics_set_visible(state: State<'_, AppState>, host_ids: Vec<HostId>) -> CmdResult<()> {
    let hub = state.metrics.clone();
    let newly: Vec<HostId> = {
        let mut v = hub.visible.lock().expect("poisoned");
        let next: HashSet<HostId> = host_ids.into_iter().collect();
        let newly = next.difference(&v).copied().collect();
        *v = next;
        newly
    };
    let pollers = hub.pollers.lock().expect("poisoned");
    for id in newly {
        if let Some(p) = pollers.get(&id) {
            p.wake.notify_one();
        }
    }
    Ok(())
}

/// Retry a host now (skips the offline backoff).
#[tauri::command]
#[specta::specta]
pub fn metrics_retry(state: State<'_, AppState>, host_id: HostId) -> CmdResult<()> {
    if let Some(p) = state.metrics.pollers.lock().expect("poisoned").get(&host_id) {
        p.wake.notify_one();
    }
    Ok(())
}

/// Stored history between `from` and `to` (unix seconds).
#[tauri::command]
#[specta::specta]
pub fn metrics_history(state: State<'_, AppState>, host_id: HostId, from: f64, to: f64) -> CmdResult<Vec<Point>> {
    Ok(state.store().metric_history(host_id, from as i64, to as i64)?)
}
