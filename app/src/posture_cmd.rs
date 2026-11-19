//! Security posture (1.1, D-168): the System → security audit plus two quest
//! checks on many hosts, kept as a history; a drop notifies. Read only, no
//! sudo. See `core/src/posture.rs`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::notify::{NewNotice, NoticeLevel, NoticeSource};
use nightgrid_core::posture::{self, PostureHost, PostureRun};
use nightgrid_core::quest;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::system::security;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::CmdResult;
use crate::state::AppState;

const TIMEOUT: Duration = Duration::from_secs(45);
/// Hosts read at once.
const PARALLEL: usize = 3;
/// Connected hosts are re-scored when their last run is this old.
const DAILY: i64 = 24 * 3600;

static AUTO_RUNNING: AtomicBool = AtomicBool::new(false);

async fn read(app: &AppHandle, host_id: HostId) -> PostureRun {
    let now = now_unix();
    let conn = match crate::term_cmd::connect_posix(app, host_id, "security.posture").await {
        Ok(c) => c,
        Err(e) => return posture::failed(host_id, now, e.to_string()),
    };
    let work = async {
        if conn.is_windows() {
            // The quest checks are Linux commands; Windows scores its own checks.
            let report = nightgrid_core::win::security::audit(&conn).await.map_err(|e| e.to_string())?;
            return Ok(posture::combine(host_id, now, &report, &[]));
        }
        let report = security::audit(&conn).await.map_err(|e| e.to_string())?;
        let (planned, _) = quest::plan(&posture::EXTRA_CHECKS.map(String::from));
        let extras = match conn.exec(&quest::script(&planned)).await {
            Ok((_, out, _)) => quest::results(&planned, &String::from_utf8_lossy(&out)),
            Err(_) => Vec::new(),
        };
        Ok::<_, String>(posture::combine(host_id, now, &report, &extras))
    };
    match tokio::time::timeout(TIMEOUT, work).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => posture::failed(host_id, now, e),
        Err(_) => posture::failed(host_id, now, "the host took longer than 45 s to answer".into()),
    }
}

/// Scores one host, keeps the run and notifies on a drop.
async fn score_host(app: &AppHandle, host_id: HostId) -> PostureRun {
    let prev = app.state::<AppState>().store().posture_last_good(host_id).ok().flatten();
    let run = read(app, host_id).await;
    let state = app.state::<AppState>();
    if let Err(e) = state.store().posture_save(&run) {
        tracing::warn!("couldn't keep the posture run: {e}");
    }
    if posture::dropped(prev.as_ref(), &run) {
        let name = state.store().get_host(host_id).ok().flatten().map_or_else(|| format!("host {host_id}"), |h| h.name);
        let worse = prev.as_ref().map(|p| posture::newly_failing(p, &run)).unwrap_or_default();
        let (p, n) = (prev.as_ref().and_then(|p| p.score).unwrap_or(0), run.score.unwrap_or(0));
        let level = if n < 50 { NoticeLevel::Critical } else { NoticeLevel::Warn };
        let body =
            if worse.is_empty() { "Open Security → POSTURE for the checks.".to_owned() } else { worse.join("; ") };
        crate::notify_cmd::send(
            app,
            NewNotice::new(NoticeSource::Posture, level, format!("Security score fell on {name}: {p} → {n}"), body)
                .host(host_id)
                .view("security/posture"),
        );
    }
    tracing::info!(host = %host_id, score = ?run.score, "posture check");
    run
}

async fn score_many(app: &AppHandle, ids: Vec<HostId>) -> Vec<PostureRun> {
    let mut out = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(PARALLEL) {
        let runs = futures_util::future::join_all(chunk.iter().map(|id| score_host(app, *id))).await;
        out.extend(runs);
    }
    out
}

/// Every scored host's latest run, the score before it and the trend.
#[tauri::command]
#[specta::specta]
pub fn posture_list(state: State<'_, AppState>) -> CmdResult<Vec<PostureHost>> {
    let hosts: Vec<HostId> = state.store().list_hosts()?.into_iter().map(|h| h.id).collect();
    Ok(state.store().posture_hosts()?.into_iter().filter(|p| hosts.contains(&p.latest.host_id)).collect())
}

/// Recent runs of one host, newest first.
#[tauri::command]
#[specta::specta]
pub fn posture_history(state: State<'_, AppState>, host_id: HostId) -> CmdResult<Vec<PostureRun>> {
    Ok(state.store().posture_runs(host_id, 30)?)
}

/// Scores these hosts now (three at a time).
#[tauri::command]
#[specta::specta]
pub async fn posture_check(app: AppHandle, host_ids: Vec<HostId>) -> CmdResult<Vec<PostureRun>> {
    Ok(score_many(&app, host_ids).await)
}

/// Re-scores connected hosts whose last run is a day old (from the alert
/// loop; one batch at a time).
pub(crate) fn due(app: &AppHandle, cfg: &AppSettings, connected: &[HostId], now: i64) {
    if !cfg.feature("security.posture") || !cfg.security.posture_daily || connected.is_empty() {
        return;
    }
    let last = app.state::<AppState>().store().posture_last_at().unwrap_or_default();
    let ids: Vec<HostId> =
        connected.iter().copied().filter(|h| !last.iter().any(|(id, at)| id == h && now - at < DAILY)).collect();
    if ids.is_empty() || AUTO_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        score_many(&app, ids).await;
        AUTO_RUNNING.store(false, Ordering::SeqCst);
    });
}
