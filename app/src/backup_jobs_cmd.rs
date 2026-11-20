//! Backup status board (M10g): jobs are stored locally; status and detection
//! run one read-only script per host (no sudo, no repository passwords).

use nightgrid_core::backup_jobs::{self, BackupJob, BackupJobId, BackupJobInput, BackupStatus, BackupSuggestion};
use nightgrid_core::backup_restore;
use nightgrid_core::hosts::HostId;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::system::sudo;
use nightgrid_core::system::SystemError;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::system_cmd::{SudoInput, SysResult};
use crate::term_cmd::connect_posix;

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
    let conn = connect_posix(&app, host_id, "fleet.backups").await?;
    Ok(backup_jobs::check(&conn, &jobs, now_unix() as f64).await.map_err(SystemError::other)?)
}

/// Backup jobs found in the host's systemd timers and cron tables.
#[tauri::command]
#[specta::specta]
pub async fn backup_jobs_detect(app: AppHandle, host_id: HostId) -> SysResult<Vec<BackupSuggestion>> {
    let conn = connect_posix(&app, host_id, "fleet.backups").await?;
    Ok(backup_jobs::detect(&conn).await.map_err(SystemError::other)?)
}

// Network map device labels live here too: small local config, no host access.

use nightgrid_core::device_labels::DeviceLabel;

#[tauri::command]
#[specta::specta]
pub fn device_labels_list(state: State<'_, AppState>) -> CmdResult<Vec<DeviceLabel>> {
    Ok(state.store().list_device_labels()?)
}

/// Saves a label; returns null when every field was empty and it was removed.
#[tauri::command]
#[specta::specta]
pub fn device_label_save(state: State<'_, AppState>, label: DeviceLabel) -> CmdResult<Option<DeviceLabel>> {
    Ok(state.store().save_device_label(&label)?)
}

#[tauri::command]
#[specta::specta]
pub fn device_label_delete(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    Ok(state.store().delete_device_label(&key)?)
}

/// The last test restore of every job that has one.
#[tauri::command]
#[specta::specta]
pub fn backup_tests_list(state: State<'_, AppState>) -> CmdResult<Vec<backup_restore::LastTest>> {
    Ok(state.store().backup_tests()?)
}

/// Restores one file from the job's latest backup into a temporary folder on
/// its host, checks it and deletes the folder. Runs as root with the backup
/// unit's own environment, so the repository password never leaves the host.
#[tauri::command]
#[specta::specta]
pub async fn backup_test_restore(
    app: AppHandle,
    job_id: BackupJobId,
    sudo: Option<SudoInput>,
) -> SysResult<backup_restore::RestoreTest> {
    let job = app.state::<AppState>().store().get_backup_job(job_id).map_err(SystemError::other)?;
    if let Some(why) = backup_restore::unsupported(&job) {
        return Err(SystemError::other(why).into());
    }
    let cmd = backup_restore::test_command(&job).ok_or_else(|| SystemError::other("can't be tested"))?;
    let conn = connect_posix(&app, job.host_id, "fleet.backups").await?;
    if conn.is_windows() {
        return Err(SystemError::other(
            "test restores run restic, borg or rsync through a POSIX shell, which Windows hosts don't have: restore a file by hand with the tool's own command to check it",
        )
        .into());
    }
    let auth = crate::system_cmd::resolve_sudo(&app, job.host_id, &conn, sudo).await?;
    tracing::info!(host = %job.host_id, job = job.id, tool = job.tool.as_str(), "test restore");
    let out = tokio::time::timeout(std::time::Duration::from_secs(20 * 60), sudo::run(&conn, &auth, &cmd))
        .await
        .map_err(|_| SystemError::other("the test restore took longer than 20 minutes"))??;
    let t = backup_restore::parse_test(job.id, &out.stdout, now_unix());
    let state = app.state::<AppState>();
    if let Err(e) = state.store().backup_test_save(&t) {
        tracing::warn!("couldn't save the test result: {e}");
    }
    Ok(t)
}
