//! Config drift (M10l): `/etc`, compose files and packages against a baseline
//! saved on the host, or against another host. Read-only apart from the
//! baseline folder in the login's home; file contents are fetched only to
//! show one file's diff and are never stored here (rule 1).

use nightgrid_core::confdiff::{self, DriftReport, FileDiff, SideInfo, Snapshot};
use nightgrid_core::hosts::HostId;
use nightgrid_core::system::SystemError;
use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::system_cmd::SysResult;
use crate::term_cmd::connect_host;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

fn host_name(app: &AppHandle, id: HostId) -> String {
    let state = app.state::<AppState>();
    let name = state.store().get_host(id).ok().flatten().map(|h| h.name);
    name.unwrap_or_else(|| format!("host {id}"))
}

/// The host's saved stack folders: their compose files are compared too.
fn stack_dirs(app: &AppHandle, id: HostId) -> Vec<String> {
    let state = app.state::<AppState>();
    let dirs = state.store().list_stack_dirs(id).unwrap_or_default();
    dirs.into_iter().map(|d| d.path).collect()
}

async fn run(app: &AppHandle, id: HostId, cmd: &str) -> SysResult<Vec<u8>> {
    let conn = connect_host(app, id).await?;
    let (code, out, err) = tokio::time::timeout(TIMEOUT, conn.exec(cmd))
        .await
        .map_err(|_| SystemError::other(format!("{} took too long to answer", host_name(app, id))))?
        .map_err(SystemError::other)?;
    if code != 0 && out.is_empty() {
        let e = String::from_utf8_lossy(&err);
        return Err(SystemError::other(format!("{}: {}", host_name(app, id), e.trim())).into());
    }
    Ok(out)
}

async fn live(app: &AppHandle, id: HostId) -> SysResult<Snapshot> {
    let out = run(app, id, &confdiff::snapshot_command(&stack_dirs(app, id))).await?;
    confdiff::parse_snapshot(&String::from_utf8_lossy(&out)).ok_or_else(|| SystemError::other("empty snapshot").into())
}

async fn baseline(app: &AppHandle, id: HostId) -> SysResult<Option<Snapshot>> {
    let out = run(app, id, &confdiff::read_baseline_command()).await?;
    Ok(confdiff::parse_snapshot(&String::from_utf8_lossy(&out)))
}

/// The host's saved baseline, if any.
#[tauri::command]
#[specta::specta]
pub async fn confdiff_baseline(app: AppHandle, host_id: HostId) -> SysResult<Option<SideInfo>> {
    Ok(baseline(&app, host_id).await?.map(|s| SideInfo::of("baseline", &s)))
}

/// Saves (or replaces) the baseline on the host: a manifest plus an archive
/// of the readable files, in `~/.nightgrid/baselines/default` (mode 700).
#[tauri::command]
#[specta::specta]
pub async fn confdiff_save_baseline(app: AppHandle, host_id: HostId) -> SysResult<SideInfo> {
    tracing::info!(host = %host_id, "saving a config baseline on the host");
    let out = run(&app, host_id, &confdiff::save_baseline_command(&stack_dirs(&app, host_id))).await?;
    if !String::from_utf8_lossy(&out).contains("saved") {
        return Err(SystemError::other("the baseline couldn't be written (is the home folder writable?)").into());
    }
    let b = baseline(&app, host_id).await?.ok_or_else(|| SystemError::other("the baseline wasn't saved"))?;
    Ok(SideInfo::of("baseline", &b))
}

/// Compares the host with its baseline (`other` = `None`) or with another
/// host. `hide_host_specific` leaves out files meant to differ between hosts.
#[tauri::command]
#[specta::specta]
pub async fn confdiff_compare(
    app: AppHandle,
    host_id: HostId,
    other: Option<HostId>,
    hide_host_specific: bool,
) -> SysResult<DriftReport> {
    let name = host_name(&app, host_id);
    match other {
        None => {
            let base = baseline(&app, host_id)
                .await?
                .ok_or_else(|| SystemError::other("No baseline on this host yet: save one first."))?;
            let now = live(&app, host_id).await?;
            Ok(confdiff::report("baseline", &base, &name, &now, false))
        }
        Some(o) => {
            let (a, b) = tokio::join!(live(&app, host_id), live(&app, o));
            Ok(confdiff::report(&name, &a?, &host_name(&app, o), &b?, hide_host_specific))
        }
    }
}

/// Line diff of one file: baseline → live, or this host → the other host.
#[tauri::command]
#[specta::specta]
pub async fn confdiff_file(
    app: AppHandle,
    host_id: HostId,
    other: Option<HostId>,
    path: String,
) -> SysResult<FileDiff> {
    if !confdiff::fetchable(&path) {
        return Err(SystemError::other("that file is compared by hash only").into());
    }
    let read_live = confdiff::read_file_command(&path).expect("checked above");
    let left_cmd = match other {
        None => confdiff::read_baseline_file_command(&path).expect("checked above"),
        Some(_) => read_live.clone(),
    };
    let left = run(&app, host_id, &left_cmd).await?;
    let right = run(&app, other.unwrap_or(host_id), &read_live).await?;
    Ok(confdiff::line_diff(&path, &left, &right, 3))
}
