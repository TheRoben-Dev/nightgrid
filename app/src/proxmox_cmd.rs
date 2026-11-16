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
