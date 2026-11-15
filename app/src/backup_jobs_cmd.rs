//! Backup status board (M10g): jobs are stored locally; status and detection
//! run one read-only script per host (no sudo, no repository passwords).

use nightgrid_core::backup_jobs::{self, BackupJob, BackupJobId, BackupJobInput, BackupStatus, BackupSuggestion};
use nightgrid_core::hosts::HostId;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::system::SystemError;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::system_cmd::SysResult;
use crate::term_cmd::connect_host;

#[tauri::command]
#[specta::specta]
pub fn backup_jobs_list(state: State<'_, AppState>) -> CmdResult<Vec<BackupJob>> {
    Ok(state.store().list_backup_jobs()?)
}

/// Creates (`id` = null) or updates a job.
#[tauri::command]
#[specta::specta]
pub fn backup_job_save(
    state: State<'_, AppState>,
    id: Option<BackupJobId>,
    input: BackupJobInput,
) -> CmdResult<BackupJob> {
    let mut store = state.store();
    let job = match id {
        Some(id) => store.update_backup_job(id, &input)?,
        None => store.create_backup_job(&input)?,
    };
    state.alerts.wake();
    Ok(job)
}

#[tauri::command]
#[specta::specta]
pub fn backup_job_delete(state: State<'_, AppState>, id: BackupJobId) -> CmdResult<()> {
    Ok(state.store().delete_backup_job(id)?)
}

/// Status of every job on one host.
#[tauri::command]
#[specta::specta]
pub async fn backup_jobs_check(app: AppHandle, host_id: HostId) -> SysResult<Vec<BackupStatus>> {
    let jobs: Vec<BackupJob> = {
        let state = app.state::<AppState>();
        let all = state.store().list_backup_jobs().map_err(SystemError::other)?;
        all.into_iter().filter(|j| j.host_id == host_id).collect()
    };
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    let conn = connect_host(&app, host_id).await?;
    Ok(backup_jobs::check(&conn, &jobs, now_unix() as f64).await.map_err(SystemError::other)?)
}

/// Backup jobs found in the host's systemd timers and cron tables.
#[tauri::command]
#[specta::specta]
pub async fn backup_jobs_detect(app: AppHandle, host_id: HostId) -> SysResult<Vec<BackupSuggestion>> {
    let conn = connect_host(&app, host_id).await?;
    Ok(backup_jobs::detect(&conn).await.map_err(SystemError::other)?)
}
