//! Proxmox VE (M10r): `pvesh` on the host over SSH, as root through the
//! usual sudo path. Nothing is stored; every read is fresh.

use nightgrid_core::hosts::HostId;
use nightgrid_core::proxmox::{self, PveAction, PveOverview, PveTaskDetail};
use nightgrid_core::system::sudo;
use nightgrid_core::system::SystemError;
use nightgrid_core::timeline::{EventKind, Level};
use tauri::AppHandle;

use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_host;

fn other(e: impl std::fmt::Display) -> SystemError {
    SystemError::other(e)
}

async fn root_run(app: &AppHandle, host_id: HostId, sudo_in: Option<SudoInput>, script: &str) -> SysResult<String> {
    let conn = connect_host(app, host_id).await?;
    let auth = resolve_sudo(app, host_id, &conn, sudo_in).await?;
    let o = sudo::run(&conn, &auth, &sudo::sh(script)).await?;
    // pvesh writes its errors to stderr; keep them for the parsers.
    Ok(if o.stderr.trim().is_empty() { o.stdout } else { format!("{}\n{}", o.stdout, o.stderr) })
}

/// Nodes, guests, storage and recent tasks of the cluster this host is in.
#[tauri::command]
#[specta::specta]
pub async fn pve_overview(app: AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<PveOverview> {
    let out = root_run(&app, host_id, sudo, &proxmox::overview_script()).await?;
    Ok(proxmox::parse_overview(&out))
}

/// Starts a power action on a guest; returns the task's UPID.
#[tauri::command]
#[specta::specta]
pub async fn pve_guest_action(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    action: PveAction,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let script = proxmox::action_script(&node, &kind, vmid, action)
        .ok_or_else(|| other(format!("can't {} {kind} {vmid} on {node:?}", action.path())))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    let upid = proxmox::parse_upid(&out).map_err(other)?;
    tracing::info!(host = %host_id, %node, vmid, ?action, "proxmox guest action");
    crate::timeline_cmd::record(
        &app,
        host_id,
        EventKind::Power,
        if action.is_destructive() { Level::Warn } else { Level::Info },
        &format!("Proxmox: {} {vmid}", action.path()),
        &format!("{kind} {vmid} on {node}"),
    );
    Ok(upid)
}

/// A task's state and its log.
#[tauri::command]
#[specta::specta]
pub async fn pve_task(
    app: AppHandle,
    host_id: HostId,
    node: String,
    upid: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveTaskDetail> {
    let script = proxmox::task_script(&node, &upid, 2000).ok_or_else(|| other("not a Proxmox task id"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    proxmox::parse_task(&out).map_err(|e| other(e).into())
}

// ---------------------------------------------------------------- part 2

use nightgrid_core::proxmox::backup::{self as pbk, PveBackups, PveSnapshot, PveStorageRef};

fn invalid(what: &str) -> SystemError {
    other(format!("invalid {what}"))
}

async fn start_task(
    app: &AppHandle,
    host_id: HostId,
    sudo: Option<SudoInput>,
    script: Option<String>,
    what: &str,
) -> SysResult<String> {
    let script = script.ok_or_else(|| invalid(what))?;
    let out = root_run(app, host_id, sudo, &script).await?;
    Ok(proxmox::parse_upid(&out).map_err(other)?)
}

fn note(app: &AppHandle, host_id: HostId, level: Level, title: String, detail: String) {
    tracing::info!(host = %host_id, %title, "proxmox");
    crate::timeline_cmd::record(app, host_id, EventKind::Stack, level, &title, &detail);
}

/// A guest's snapshots, oldest first.
#[tauri::command]
#[specta::specta]
pub async fn pve_snapshots(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    sudo: Option<SudoInput>,
) -> SysResult<Vec<PveSnapshot>> {
    let script = pbk::snapshots_script(&node, &kind, vmid).ok_or_else(|| invalid("guest"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(pbk::parse_snapshots(&out).map_err(other)?)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_snapshot_create(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    name: String,
    description: String,
    vmstate: bool,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pbk::snapshot_create_script(&node, &kind, vmid, &name, &description, vmstate);
    let upid = start_task(&app, host_id, sudo, s, "snapshot name").await?;
    note(&app, host_id, Level::Info, format!("Proxmox: snapshot {name} of {vmid}"), format!("{kind} {vmid} on {node}"));
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_snapshot_rollback(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    name: String,
    start: bool,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pbk::snapshot_rollback_script(&node, &kind, vmid, &name, start);
    let upid = start_task(&app, host_id, sudo, s, "snapshot").await?;
    note(
        &app,
        host_id,
        Level::Warn,
        format!("Proxmox: {vmid} rolled back to {name}"),
        format!("{kind} {vmid} on {node}"),
    );
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_snapshot_delete(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    name: String,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pbk::snapshot_delete_script(&node, &kind, vmid, &name);
    let upid = start_task(&app, host_id, sudo, s, "snapshot").await?;
    note(
        &app,
        host_id,
        Level::Warn,
        format!("Proxmox: snapshot {name} of {vmid} deleted"),
        format!("{kind} {vmid} on {node}"),
    );
    Ok(upid)
}

/// Backup jobs, uncovered guests and the backup files on each storage.
#[tauri::command]
#[specta::specta]
pub async fn pve_backups(
    app: AppHandle,
    host_id: HostId,
    storages: Vec<PveStorageRef>,
    sudo: Option<SudoInput>,
) -> SysResult<PveBackups> {
    let script = pbk::backups_script(&storages).ok_or_else(|| invalid("storage"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(pbk::parse_backups(&out))
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_backup_now(
    app: AppHandle,
    host_id: HostId,
    node: String,
    vmid: u32,
    storage: String,
    mode: String,
    notes: String,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pbk::backup_now_script(&node, vmid, &storage, &mode, &notes);
    let upid = start_task(&app, host_id, sudo, s, "backup settings").await?;
    note(
        &app,
        host_id,
        Level::Info,
        format!("Proxmox: backup of {vmid} started"),
        format!("to {storage}, {mode} mode"),
    );
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_restore(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    volid: String,
    storage: String,
    force: bool,
    start: bool,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pbk::restore_script(&node, &kind, vmid, &volid, &storage, force, start);
    let upid = start_task(&app, host_id, sudo, s, "restore").await?;
    note(
        &app,
        host_id,
        if force { Level::Warn } else { Level::Info },
        format!("Proxmox: restore to {vmid}{}", if force { " (overwrote it)" } else { "" }),
        volid,
    );
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_backup_protect(
    app: AppHandle,
    host_id: HostId,
    node: String,
    volid: String,
    protect: bool,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = pbk::backup_protect_script(&node, &volid, protect).ok_or_else(|| invalid("backup"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    let t = out.trim();
    if t.is_empty() || t == "null" || t == "\"\"" {
        Ok(())
    } else {
        Err(other(proxmox::explain(t)).into())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn pve_backup_delete(
    app: AppHandle,
    host_id: HostId,
    volid: String,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = pbk::backup_delete_script(&volid).ok_or_else(|| invalid("backup"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    if !out.lines().any(|l| l.trim() == "@@ok") {
        return Err(other(proxmox::explain(&out)).into());
    }
    note(&app, host_id, Level::Warn, "Proxmox: backup deleted".into(), volid);
    Ok(())
}
