//! Uptime history (M10t): each alerts round reports every host (from its
//! metrics poller) and every checked service (from its latest check) to the
//! recorder, whether alerts are on or not. Reads serve the 90-day bars and
//! outage lists.

use std::collections::HashMap;
use std::sync::Mutex;

use nightgrid_core::depgraph::Node;
use nightgrid_core::sla::{self, SlaReport};
use nightgrid_core::uptime::{availability, Observation, Outage, Recorder, Seen, UptimeSeries};
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::metrics_cmd::PollState;
use crate::state::AppState;

#[derive(Default)]
pub struct UptimeHub {
    recorder: Mutex<Recorder>,
    pruned: Mutex<i64>,
}

fn host_seen(p: &PollState) -> Option<Seen> {
    match p {
        PollState::Ok => Some(Seen::Up),
        PollState::Offline => Some(Seen::Down),
        PollState::Suspended | PollState::Rebooting | PollState::ShuttingDown | PollState::Asleep => Some(Seen::Asleep),
        // A client device that's away isn't an outage either.
        PollState::Away => Some(Seen::Asleep),
        // Connecting, waiting for a login, a broken collect script or
        // metrics off: not known either way.
        _ => None,
    }
}

/// Records one round. `polls` comes from the metrics pollers (empty when metrics are off).
pub fn record(
    app: &AppHandle,
    polls: &HashMap<nightgrid_core::hosts::HostId, (PollState, Option<String>)>,
    services_on: bool,
    interval_secs: u32,
    now: i64,
) {
    let state = app.state::<AppState>();
    let mut obs = Vec::new();
    let (hosts, services) = {
        let store = state.store();
        (store.list_hosts().unwrap_or_default(), store.list_services().unwrap_or_default())
    };
    for h in &hosts {
        if let Some((p, msg)) = polls.get(&h.id) {
            if let Some(seen) = host_seen(p) {
                obs.push(Observation { node: Node::Host(h.id), seen, detail: msg.clone().unwrap_or_default() });
            }
        }
    }
    if services_on {
        let latest = state.services.latest();
        // A result older than three intervals is stale (checker paused or stuck).
        let fresh = i64::from(interval_secs.max(60)) * 3;
        for s in services.iter().filter(|s| s.check_enabled) {
            let Some(st) = latest.get(&s.id) else { continue };
            if now - st.ts > fresh {
                continue;
            }
            let seen = if st.result.up { Seen::Up } else { Seen::Down };
            let detail = st
                .result
                .error
                .clone()
                .unwrap_or_else(|| st.result.status.map(|c| format!("HTTP {c}")).unwrap_or_default());
            obs.push(Observation { node: Node::Service(s.id), seen, detail });
        }
    }
    let hub = state.uptime.clone();
    let mut store = state.store();
    if let Err(e) = hub.recorder.lock().expect("poisoned").tick(&mut store, now, &obs) {
        tracing::warn!("uptime: {e}");
    }
    let mut pruned = hub.pruned.lock().expect("poisoned");
    if now - *pruned > 6 * 3600 {
        *pruned = now;
        if let Err(e) = store.prune_uptime(now) {
            tracing::warn!("uptime prune: {e}");
        }
    }
}

fn window(days: u32, utc_offset_min: i32) -> (i64, i64, i64) {
    let now = nightgrid_core::services::check::now_unix();
    let off = i64::from(utc_offset_min.clamp(-14 * 60, 14 * 60)) * 60;
    let today = (now + off).div_euclid(86_400) * 86_400 - off;
    (today - i64::from(days.clamp(1, 400) - 1) * 86_400, now + 1, off)
}

/// Availability per day for the last `days` days, oldest first, `None` for
/// days with nothing seen (for the status page's bars).
pub fn daily(app: &AppHandle, days: u32, utc_offset_min: i32) -> HashMap<Node, Vec<Option<f32>>> {
    let (from, to, off) = window(days, utc_offset_min);
    let series = app.state::<AppState>().store().uptime_days(from, to, off).unwrap_or_default();
    series
        .into_iter()
        .map(|s| {
            let mut v = vec![None; days.clamp(1, 400) as usize];
            for d in s.days {
                let i = (d.day - from).div_euclid(86_400);
                if let Some(slot) = usize::try_from(i).ok().and_then(|i| v.get_mut(i)) {
                    *slot = availability(d.up.into(), d.down.into()).map(|p| p as f32);
                }
            }
            (s.node, v)
        })
        .collect()
}

/// Per-day totals for the last `days` days (at most 400), days starting at
/// local midnight (`utc_offset_min` east of UTC, from the UI).
#[tauri::command]
#[specta::specta]
pub fn uptime_days(state: State<'_, AppState>, days: u32, utc_offset_min: i32) -> CmdResult<Vec<UptimeSeries>> {
    let (from, to, off) = window(days, utc_offset_min);
    Ok(state.store().uptime_days(from, to, off)?)
}

/// Outages in `[from, to]` (unix seconds), newest first; one host or service, or all.
#[tauri::command]
#[specta::specta]
pub fn uptime_outages(state: State<'_, AppState>, node: Option<Node>, from: u32, to: u32) -> CmdResult<Vec<Outage>> {
    Ok(state.store().outages(node, i64::from(from), i64::from(to))?)
}

/// Local midnight on the 1st of `month` to the 1st of the next, as unix seconds.
fn month_bounds(year: u32, month: u32) -> Option<(i64, i64)> {
    use chrono::{Local, TimeZone};
    let y = i32::try_from(year).ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let start = Local.with_ymd_and_hms(y, month, 1, 0, 0, 0).earliest()?;
    let (ny, nm) = if month == 12 { (y + 1, 1) } else { (y, month + 1) };
    let end = Local.with_ymd_and_hms(ny, nm, 1, 0, 0, 0).earliest()?;
    Some((start.timestamp(), end.timestamp()))
}

/// Hosts by name, then checked services by name.
fn subjects(app: &AppHandle) -> Vec<(Node, String)> {
    let state = app.state::<AppState>();
    let store = state.store();
    let mut hosts: Vec<(Node, String)> =
        store.list_hosts().unwrap_or_default().into_iter().map(|h| (Node::Host(h.id), h.name)).collect();
    hosts.sort_by_key(|(_, n)| n.to_lowercase());
    let mut services: Vec<(Node, String)> = store
        .list_services()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.check_enabled)
        .map(|s| (Node::Service(s.id), s.name))
        .collect();
    services.sort_by_key(|(_, n)| n.to_lowercase());
    hosts.extend(services);
    hosts
}

fn build_report(app: &AppHandle, year: u32, month: u32, target: f64) -> CmdResult<SlaReport> {
    let (from, to) = month_bounds(year, month).ok_or_else(|| CommandError::msg("pick a month"))?;
    let now = nightgrid_core::services::check::now_unix();
    let label = format!("{} {year}", sla::MONTHS[(month - 1) as usize]);
    let target = if target.is_finite() { target.clamp(50.0, 99.999) } else { 99.9 };
    let subjects = subjects(app);
    Ok(app.state::<AppState>().store().sla_report(&subjects, from, to.min(now + 1), &label, target, now)?)
}

/// The uptime report for a calendar month (local time), checked against
/// `target` percent.
#[tauri::command]
#[specta::specta]
pub fn uptime_report(app: AppHandle, year: u32, month: u32, target: f64) -> CmdResult<SlaReport> {
    build_report(&app, year, month, target)
}

/// Saves the month's report as an HTML page picked with a save dialog.
/// `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn uptime_report_export(app: AppHandle, year: u32, month: u32, target: f64) -> CmdResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let report = build_report(&app, year, month, target)?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Save uptime report")
        .set_file_name(format!("nightgrid-uptime-{year}-{month:02}.html"))
        .add_filter("HTML page", &["html"])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(path) = rx.await.ok().flatten().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let offset = chrono::Local::now().offset().local_minus_utc() / 60;
    let html = sla::render_html(&report, offset);
    tokio::fs::write(&path, html)
        .await
        .map_err(|e| CommandError::msg(format!("Couldn't write {}: {e}", path.display())))?;
    Ok(Some(path.display().to_string()))
}
