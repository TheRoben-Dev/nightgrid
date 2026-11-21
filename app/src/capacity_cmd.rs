//! Capacity planner (1.1, D-172). See `core/src/capacity.rs`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use nightgrid_core::capacity::{self, HostCapacity, HostSpec};
use nightgrid_core::hosts::HostId;
use nightgrid_core::metrics::history::HOURLY;
use nightgrid_core::notify::{NewNotice, NoticeLevel, NoticeSource};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::CmdResult;
use crate::state::AppState;

const DAY: i64 = 86_400;
static SPEC_WRITES: Mutex<Option<HashMap<HostId, i64>>> = Mutex::new(None);
static LAST_PRESSURE: AtomicI64 = AtomicI64::new(0);

fn read(state: &AppState, days: u32, now: i64) -> CmdResult<Vec<HostCapacity>> {
    let store = state.store();
    let specs = store.host_specs()?;
    let from = now - i64::from(days.clamp(1, 400)) * DAY;
    let mut out = Vec::new();
    let mut fleet = store.fleet_disk_forecasts(now)?;
    for h in store.list_hosts()? {
        let points = store.metric_history_at(h.id, HOURLY, from, now)?;
        let disks = fleet.remove(&h.id).unwrap_or_default();
        let spec = specs.iter().find(|s| s.host_id == h.id).cloned();
        out.push(capacity::summarize(h.id, spec, &points, &disks));
    }
    Ok(out)
}

/// Each host's size, p95 CPU and memory over the last `days`, and disks.
#[tauri::command]
#[specta::specta]
pub fn capacity_list(state: State<'_, AppState>, days: u32) -> CmdResult<Vec<HostCapacity>> {
    read(&state, days, now_unix())
}

/// From the chores tick: keeps each connected host's size (hourly) and, once
/// a day, sends a notice for hosts short of memory or CPU over the last 7
/// days (at most one a week per host).
pub(crate) fn due(app: &AppHandle, cfg: &AppSettings, connected: &[HostId], now: i64) {
    if !cfg.feature("fleet.capacity") {
        return;
    }
    let state = app.state::<AppState>();
    {
        let mut g = SPEC_WRITES.lock().expect("poisoned");
        let written = g.get_or_insert_with(HashMap::new);
        for &id in connected {
            if written.get(&id).is_some_and(|t| now - t < 3600) {
                continue;
            }
            let Some(s) = state.metrics.latest(id) else { continue };
            let cores = s.cores.len() as u32;
            if cores == 0 || s.mem_total == 0 {
                continue;
            }
            if state.store().host_spec_save(&HostSpec { host_id: id, cores, mem_total: s.mem_total, at: now }).is_ok() {
                written.insert(id, now);
            }
        }
    }
    if now - LAST_PRESSURE.load(Ordering::Relaxed) < DAY {
        return;
    }
    LAST_PRESSURE.store(now, Ordering::Relaxed);
    let Ok(list) = read(&state, 7, now) else { return };
    for c in list {
        let short = capacity::pressure(&c);
        if short.is_empty() || c.spec.is_none() {
            continue;
        }
        let last = state.store().capacity_notified_at(c.host_id).ok().flatten();
        if last.is_some_and(|t| now - t < 7 * DAY) {
            continue;
        }
        let name =
            state.store().get_host(c.host_id).ok().flatten().map_or_else(|| format!("host {}", c.host_id), |h| h.name);
        crate::notify_cmd::send(
            app,
            NewNotice::new(
                NoticeSource::Capacity,
                NoticeLevel::Warn,
                format!("{name} is running short"),
                format!("Over the last 7 days: {}. Fleet → CAPACITY shows what still fits.", short.join(", ")),
            )
            .host(c.host_id)
            .view("fleet/capacity"),
        );
        let _ = state.store().capacity_notified(c.host_id, now);
    }
}
