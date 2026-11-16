//! UPS watch (M10s): reads each watched UPS with `upsc` from the alerts loop
//! (connected hosts only, every 30 s), turns readings into alerts, and runs
//! the shutdown countdown. The countdown lives here, not in the UI, so it
//! works with the window closed; the UI shows it from the alerts snapshot
//! and can cancel it.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use nightgrid_core::alerts::Condition;
use nightgrid_core::hosts::HostId;
use nightgrid_core::power::PowerAction;
use nightgrid_core::ups::{self, UpsId, UpsInput, UpsReading, UpsWatch};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

const READ_EVERY: i64 = 30;

/// A shutdown that will start at `deadline` unless cancelled.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpsCountdown {
    pub watch_id: UpsId,
    pub label: String,
    pub reason: String,
    /// Unix seconds.
    pub deadline: f64,
    /// Host names in the order they will be shut down.
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpsStatus {
    pub watch: UpsWatch,
    pub reading: Option<UpsReading>,
    /// Unix seconds of the last reading.
    pub read_at: Option<f64>,
    pub on_battery_since: Option<f64>,
}

#[derive(Default)]
pub struct UpsHub {
    readings: Mutex<HashMap<UpsId, (UpsReading, i64)>>,
    on_battery_since: Mutex<HashMap<UpsId, i64>>,
    /// Shutdown started (or cancelled by the user) during the current outage.
    done: Mutex<HashSet<UpsId>>,
    countdown: Mutex<Option<UpsCountdown>>,
    /// Bumped on every start and cancel; a countdown task only acts if it still matches.
    generation: AtomicU64,
}

impl UpsHub {
    pub fn countdown(&self) -> Option<UpsCountdown> {
        self.countdown.lock().expect("poisoned").clone()
    }
}

fn now_f() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("notification failed: {e}");
    }
}

async fn read(app: &AppHandle, w: &UpsWatch) -> Option<UpsReading> {
    let conn = crate::term_cmd::connect_host(app, w.host_id).await.ok()?;
    let (_, out, _) = conn.exec(&ups::command(&w.target)).await.ok()?;
    Some(ups::parse(&String::from_utf8_lossy(&out)))
}

/// Reads the UPS watches whose host is connected (at most every 30 s each).
/// Returns their alert conditions and the hosts they cover.
pub async fn evaluate(app: &AppHandle, connected: &[HostId], now: i64) -> (Vec<Condition>, HashSet<HostId>) {
    let state = app.state::<AppState>();
    let hub = state.ups.clone();
    let watches = state.store().ups_list().unwrap_or_default();
    let mut conds = Vec::new();
    let mut covered = HashSet::new();
    for w in watches.iter().filter(|w| w.enabled) {
        let last = hub.readings.lock().expect("poisoned").get(&w.id).map(|(_, t)| *t);
        let fresh = last.is_some_and(|t| now - t < READ_EVERY);
        if !fresh && connected.contains(&w.host_id) {
            if let Ok(Some(r)) = tokio::time::timeout(Duration::from_secs(15), read(app, w)).await {
                hub.readings.lock().expect("poisoned").insert(w.id, (r, now));
            }
        }
        let Some((r, at)) = hub.readings.lock().expect("poisoned").get(&w.id).cloned() else { continue };
        if now - at > 5 * READ_EVERY {
            continue; // stale: the host went away; leave its alerts as they are
        }
        covered.insert(w.host_id);
        conds.extend(ups::conditions(w, &r));
        decide(app, w, &r, now);
    }
    // Forget watches that were removed.
    let ids: HashSet<UpsId> = watches.iter().map(|w| w.id).collect();
    hub.readings.lock().expect("poisoned").retain(|id, _| ids.contains(id));
    hub.on_battery_since.lock().expect("poisoned").retain(|id, _| ids.contains(id));
    let gone = hub.countdown().is_some_and(|c| !ids.contains(&c.watch_id));
    if gone {
        cancel(app, "the UPS watch was removed");
    }
    (conds, covered)
}

fn decide(app: &AppHandle, w: &UpsWatch, r: &UpsReading, now: i64) {
    let hub = app.state::<AppState>().ups.clone();
    if !r.on_battery {
        hub.on_battery_since.lock().expect("poisoned").remove(&w.id);
        hub.done.lock().expect("poisoned").remove(&w.id);
        if hub.countdown().is_some_and(|c| c.watch_id == w.id) {
            cancel(app, "power is back");
            notify(app, &format!("NIGHTGRID · {}", w.label), "Power is back: the shutdown was cancelled.");
        }
        return;
    }
    let since = *hub.on_battery_since.lock().expect("poisoned").entry(w.id).or_insert(now);
    if !w.shutdown
        || w.group_id.is_none()
        || hub.done.lock().expect("poisoned").contains(&w.id)
        || hub.countdown().is_some()
    {
        return;
    }
    if let Some(reason) = ups::trigger(w, r, Some(since), now) {
        start(app, w, reason);
    }
}

fn start(app: &AppHandle, w: &UpsWatch, reason: String) {
    let state = app.state::<AppState>();
    let hub = state.ups.clone();
    let hosts = state.store().list_hosts().unwrap_or_default();
    let members: Vec<HostId> =
        hosts.iter().filter(|h| h.group_id == w.group_id && w.group_id.is_some()).map(|h| h.id).collect();
    let order = ups::shutdown_order(&members, w.host_id);
    if order.is_empty() {
        return;
    }
    let names: HashMap<HostId, String> = hosts.iter().map(|h| (h.id, h.name.clone())).collect();
    let name = move |id: HostId| names.get(&id).cloned().unwrap_or_else(|| format!("host {id}"));
    let cd = UpsCountdown {
        watch_id: w.id,
        label: w.label.clone(),
        reason: reason.clone(),
        deadline: now_f() + f64::from(w.countdown_secs),
        targets: order.iter().map(|id| name(*id)).collect(),
    };
    let gen = hub.generation.fetch_add(1, Ordering::SeqCst) + 1;
    *hub.countdown.lock().expect("poisoned") = Some(cd.clone());
    hub.done.lock().expect("poisoned").insert(w.id);
    tracing::warn!(ups = %w.label, %reason, "UPS shutdown countdown started");
    // Ignores quiet hours: this one matters.
    notify(
        app,
        &format!("NIGHTGRID · {}: shutdown in {}", w.label, ups::words(w.countdown_secs)),
        &format!("{reason}. Shutting down {} unless you cancel it in Nightgrid.", cd.targets.join(", ")),
    );
    crate::alerts_cmd::push_now(app);
    let app = app.clone();
    let secs = w.countdown_secs;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(u64::from(secs))).await;
        let hub = app.state::<AppState>().ups.clone();
        if hub.generation.load(Ordering::SeqCst) != gen {
            return; // cancelled or replaced
        }
        *hub.countdown.lock().expect("poisoned") = None;
        crate::alerts_cmd::push_now(&app);
        let mut failed = Vec::new();
        for id in order {
            if let Err(e) = crate::system_cmd::system_power(app.clone(), id, PowerAction::Shutdown, None).await {
                tracing::warn!(host = id, "UPS shutdown failed: {:?}", e);
                failed.push(name(id));
            }
        }
        let body = if failed.is_empty() {
            "Every host in the group was told to shut down.".to_owned()
        } else {
            format!("Couldn't shut down {} (sudo needs a password there, or it was unreachable).", failed.join(", "))
        };
        notify(&app, "NIGHTGRID · UPS shutdown", &body);
    });
}

fn cancel(app: &AppHandle, why: &str) {
    let hub = app.state::<AppState>().ups.clone();
    hub.generation.fetch_add(1, Ordering::SeqCst);
    if hub.countdown.lock().expect("poisoned").take().is_some() {
        tracing::info!("UPS shutdown countdown cancelled: {why}");
        crate::alerts_cmd::push_now(app);
    }
}

#[tauri::command]
#[specta::specta]
pub fn ups_list(state: State<'_, AppState>) -> CmdResult<Vec<UpsWatch>> {
    Ok(state.store().ups_list()?)
}

#[tauri::command]
#[specta::specta]
pub fn ups_save(state: State<'_, AppState>, input: UpsInput) -> CmdResult<UpsWatch> {
    let w = state.store().ups_save(&input)?;
    state.ups.readings.lock().expect("poisoned").remove(&w.id);
    state.alerts.wake();
    Ok(w)
}

#[tauri::command]
#[specta::specta]
pub fn ups_delete(state: State<'_, AppState>, id: UpsId) -> CmdResult<()> {
    state.store().ups_delete(id)?;
    state.alerts.wake();
    Ok(())
}

/// Every watch with its latest reading.
#[tauri::command]
#[specta::specta]
pub fn ups_status(state: State<'_, AppState>) -> CmdResult<Vec<UpsStatus>> {
    let readings = state.ups.readings.lock().expect("poisoned").clone();
    let since = state.ups.on_battery_since.lock().expect("poisoned").clone();
    Ok(state
        .store()
        .ups_list()?
        .into_iter()
        .map(|w| {
            let r = readings.get(&w.id);
            UpsStatus {
                reading: r.map(|(r, _)| r.clone()),
                read_at: r.map(|(_, t)| *t as f64),
                on_battery_since: since.get(&w.id).map(|t| *t as f64),
                watch: w,
            }
        })
        .collect())
}

/// Reads a UPS now (connects to its host if needed), to test a new watch.
#[tauri::command]
#[specta::specta]
pub async fn ups_read_now(app: AppHandle, host_id: HostId, target: String) -> CmdResult<UpsReading> {
    if !ups::valid_target(&target) {
        return Err(CommandError::msg("a NUT name like ups or ups@localhost"));
    }
    let conn = crate::term_cmd::connect_host(&app, host_id).await.map_err(|e| CommandError::msg(e.to_string()))?;
    let (_, out, _) = conn.exec(&ups::command(&target)).await.map_err(|e| CommandError::msg(e.to_string()))?;
    Ok(ups::parse(&String::from_utf8_lossy(&out)))
}

/// Stops a running shutdown countdown. It won't start again until power
/// comes back and goes out again.
#[tauri::command]
#[specta::specta]
pub fn ups_cancel(app: AppHandle) -> CmdResult<()> {
    cancel(&app, "cancelled by the user");
    Ok(())
}
