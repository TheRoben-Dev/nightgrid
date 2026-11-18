//! Restore drills (1.1, D-171): run a backup job's test restore or a
//! Proxmox restore-into-sandbox by hand or on a schedule, keep each run and
//! notify. See `core/src/drills.rs`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use nightgrid_core::drills::{self, Drill, DrillKind, DrillRun, DrillView};
use nightgrid_core::notify::{NewNotice, NoticeLevel, NoticeSource};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Drills running now (one at a time per drill).
static RUNNING: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
static LAST_LOOK: AtomicI64 = AtomicI64::new(0);

fn claim(id: u32) -> bool {
    RUNNING.lock().expect("poisoned").get_or_insert_with(HashSet::new).insert(id)
}

fn release(id: u32) {
    if let Some(s) = RUNNING.lock().expect("poisoned").as_mut() {
        s.remove(&id);
    }
}

fn running() -> Vec<u32> {
    RUNNING.lock().expect("poisoned").as_ref().map(|s| s.iter().copied().collect()).unwrap_or_default()
}

async fn run(app: &AppHandle, d: &Drill) -> (bool, Vec<String>, Option<String>, String) {
    let mut steps = Vec::new();
    match d.kind {
        DrillKind::Job => {
            let Some(job) = d.job_id else { return (false, steps, Some("no backup job".into()), String::new()) };
            match crate::backup_jobs_cmd::backup_test_restore(app.clone(), job, None).await {
                Ok(t) => {
                    steps.extend(t.steps.clone());
                    let detail = match (&t.snapshot, &t.file) {
                        (Some(s), Some(f)) => format!(
                            "snapshot {s}, file {f}{}",
                            t.sha256.as_deref().map(|h| format!(" (sha256 {h})")).unwrap_or_default()
                        ),
                        (_, Some(f)) => format!("file {f}"),
                        _ => String::new(),
                    };
                    (t.ok, steps, t.error, detail)
                }
                Err(e) => (false, steps, Some(e.to_string()), String::new()),
            }
        }
        DrillKind::Proxmox => match crate::proxmox_cmd::drill_proxmox(app, d, &mut steps).await {
            Ok(detail) => (true, steps, None, detail),
            Err(e) => (false, steps, Some(e), String::new()),
        },
    }
}

/// Runs a drill, keeps the run and sends a notice.
async fn run_and_keep(app: &AppHandle, d: Drill) -> CmdResult<DrillRun> {
    let id = d.id.unwrap_or_default();
    if !claim(id) {
        return Err(CommandError::msg("this drill is already running"));
    }
    let at = now_unix();
    let t0 = std::time::Instant::now();
    let (ok, steps, error, detail) = run(app, &d).await;
    release(id);
    let seconds = t0.elapsed().as_secs() as u32;
    let state = app.state::<AppState>();
    let rid = state.store().drill_run_save(id, at, seconds, ok, &steps, error.as_deref(), &detail)?;
    let (level, title, body) = if ok {
        (
            NoticeLevel::Ok,
            format!("Restore drill passed: {}", d.name),
            format!("Restored and checked in {seconds} s. {detail}"),
        )
    } else {
        (
            NoticeLevel::Warn,
            format!("Restore drill failed: {}", d.name),
            error.clone().unwrap_or_else(|| "See Fleet → DRILLS for the steps.".into()),
        )
    };
    crate::notify_cmd::send(
        app,
        NewNotice::new(NoticeSource::Drill, level, title, body).host(d.host_id).view("fleet/drills"),
    );
    tracing::info!(drill = id, ok, seconds, "restore drill");
    Ok(DrillRun { id: rid, drill_id: id, at, seconds, ok, steps, error, detail })
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DrillBoard {
    pub drills: Vec<DrillView>,
    /// Drills running right now.
    pub running: Vec<u32>,
}

#[tauri::command]
#[specta::specta]
pub fn drills_list(state: State<'_, AppState>) -> CmdResult<DrillBoard> {
    Ok(DrillBoard { drills: state.store().drill_views(now_unix())?, running: running() })
}

#[tauri::command]
#[specta::specta]
pub fn drill_save(state: State<'_, AppState>, drill: Drill) -> CmdResult<u32> {
    Ok(state.store().drill_save(&drill)?)
}

#[tauri::command]
#[specta::specta]
pub fn drill_delete(state: State<'_, AppState>, id: u32) -> CmdResult<()> {
    Ok(state.store().drill_delete(id)?)
}

/// Last 50 runs, newest first.
#[tauri::command]
#[specta::specta]
pub fn drill_runs(state: State<'_, AppState>, id: u32) -> CmdResult<Vec<DrillRun>> {
    Ok(state.store().drill_runs(id, 50)?)
}

/// Runs a drill now and waits for it (a Proxmox drill can take a while).
#[tauri::command]
#[specta::specta]
pub async fn drill_run(app: AppHandle, id: u32) -> CmdResult<DrillRun> {
    let d = app.state::<AppState>().store().drill(id)?;
    run_and_keep(&app, d).await
}

/// The drill's runs as a Markdown report.
#[tauri::command]
#[specta::specta]
pub fn drill_report(state: State<'_, AppState>, id: u32) -> CmdResult<String> {
    let store = state.store();
    let d = store.drill(id)?;
    let host = store.get_host(d.host_id)?.map_or_else(|| format!("host {}", d.host_id), |h| h.name);
    Ok(drills::report(&d, &host, &store.drill_runs(id, 50)?))
}

/// From the chores tick: every 10 minutes, starts scheduled drills that are
/// due, one at a time, on hosts that are connected (a drill never opens a
/// connection or asks for a password on its own).
pub(crate) fn due(app: &AppHandle, cfg: &AppSettings, connected: &[nightgrid_core::hosts::HostId], now: i64) {
    if !cfg.feature("fleet.drills") || now - LAST_LOOK.load(Ordering::Relaxed) < 600 || !running().is_empty() {
        return;
    }
    LAST_LOOK.store(now, Ordering::Relaxed);
    let Ok(views) = app.state::<AppState>().store().drill_views(now) else { return };
    let Some(v) =
        views.into_iter().find(|v| connected.contains(&v.drill.host_id) && v.next_at.is_some_and(|n| n <= now))
    else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = run_and_keep(&app, v.drill).await;
    });
}
