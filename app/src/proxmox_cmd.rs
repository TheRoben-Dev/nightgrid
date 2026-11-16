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

// ---------------------------------------------------------------- part 3

use nightgrid_core::proxmox::create::{self as pcr, PveAppliance, PveCloneSpec, PveCreateInfo, PveCtSpec, PveVmSpec};

/// Bridges, ISOs, CT templates and the next free id on `node`.
#[tauri::command]
#[specta::specta]
pub async fn pve_create_info(
    app: AppHandle,
    host_id: HostId,
    node: String,
    iso_storages: Vec<String>,
    tmpl_storages: Vec<String>,
    sudo: Option<SudoInput>,
) -> SysResult<PveCreateInfo> {
    let script = pcr::create_info_script(&node, &iso_storages, &tmpl_storages).ok_or_else(|| invalid("storage"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(pcr::parse_create_info(&out))
}

#[tauri::command]
#[specta::specta]
pub async fn pve_vm_create(
    app: AppHandle,
    host_id: HostId,
    spec: PveVmSpec,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pcr::vm_create_script(&spec).map_err(other)?;
    let upid = start_task(&app, host_id, sudo, Some(s), "VM").await?;
    note(
        &app,
        host_id,
        Level::Info,
        format!("Proxmox: VM {} ({}) created", spec.vmid, spec.name),
        format!("on {}", spec.node),
    );
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_ct_create(
    app: AppHandle,
    host_id: HostId,
    spec: PveCtSpec,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pcr::ct_create_script(&spec).map_err(other)?;
    let upid = start_task(&app, host_id, sudo, Some(s), "container").await?;
    note(
        &app,
        host_id,
        Level::Info,
        format!("Proxmox: CT {} ({}) created", spec.vmid, spec.hostname),
        format!("on {}", spec.node),
    );
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_clone(
    app: AppHandle,
    host_id: HostId,
    spec: PveCloneSpec,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pcr::clone_script(&spec).map_err(other)?;
    let upid = start_task(&app, host_id, sudo, Some(s), "clone").await?;
    note(
        &app,
        host_id,
        Level::Info,
        format!("Proxmox: {} cloned to {} ({})", spec.vmid, spec.newid, spec.name),
        format!("{} clone on {}", if spec.full { "full" } else { "linked" }, spec.node),
    );
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_template(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let upid = start_task(&app, host_id, sudo, pcr::template_script(&node, &kind, vmid), "guest").await?;
    note(&app, host_id, Level::Info, format!("Proxmox: {vmid} made a template"), format!("{kind} on {node}"));
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_destroy(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let upid = start_task(&app, host_id, sudo, pcr::destroy_script(&node, &kind, vmid), "guest").await?;
    note(&app, host_id, Level::Warn, format!("Proxmox: {vmid} destroyed"), format!("{kind} on {node}, disks purged"));
    Ok(upid)
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_iso_download(
    app: AppHandle,
    host_id: HostId,
    node: String,
    storage: String,
    url: String,
    filename: String,
    sha256: Option<String>,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s =
        pcr::iso_download_script(&node, &storage, &url, &filename, sha256.as_deref().filter(|h| !h.trim().is_empty()));
    let upid = start_task(&app, host_id, sudo, s, "URL, file name or checksum").await?;
    note(&app, host_id, Level::Info, format!("Proxmox: downloading {filename}"), url);
    Ok(upid)
}

/// Container templates Proxmox offers for download.
#[tauri::command]
#[specta::specta]
pub async fn pve_appliances(
    app: AppHandle,
    host_id: HostId,
    node: String,
    sudo: Option<SudoInput>,
) -> SysResult<Vec<PveAppliance>> {
    let script = pcr::appliances_script(&node).ok_or_else(|| invalid("node"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(pcr::parse_appliances(&out).map_err(other)?)
}

#[tauri::command]
#[specta::specta]
pub async fn pve_appliance_download(
    app: AppHandle,
    host_id: HostId,
    node: String,
    storage: String,
    template: String,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let s = pcr::appliance_download_script(&node, &storage, &template);
    let upid = start_task(&app, host_id, sudo, s, "template").await?;
    note(&app, host_id, Level::Info, format!("Proxmox: downloading {template}"), format!("to {storage}"));
    Ok(upid)
}

// ---- part 4: hardware and options ----

use nightgrid_core::proxmox::hardware::{self as phw, PveConfig, PveHostDevices, PveSetting};

/// A guest's configuration with its pending changes.
#[tauri::command]
#[specta::specta]
pub async fn pve_config(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    sudo: Option<SudoInput>,
) -> SysResult<PveConfig> {
    let script = phw::config_script(&node, &kind, vmid).ok_or_else(|| invalid("guest"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(phw::parse_config(&out, &kind).map_err(other)?)
}

/// Sets and removes configuration keys, refused if the config changed since `digest`.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_config_set(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    digest: String,
    set: Vec<PveSetting>,
    remove: Vec<String>,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = phw::config_set_script(&node, &kind, vmid, &digest, &set, &remove).map_err(other)?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    if !out.lines().any(|l| l.trim() == "@@ok") {
        return Err(other(proxmox::explain(&out)).into());
    }
    let mut what: Vec<String> = set.iter().map(|s| s.key.clone()).collect();
    what.extend(remove.iter().map(|k| format!("-{k}")));
    let level = if remove.iter().any(|k| phw::is_destructive_delete(k)) { Level::Warn } else { Level::Info };
    note(
        &app,
        host_id,
        level,
        format!("Proxmox: {vmid} hardware changed"),
        format!("{kind} on {node}: {}", what.join(", ")),
    );
    Ok(())
}

/// Grows a disk; returns the task when Proxmox runs it as one.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_disk_resize(
    app: AppHandle,
    host_id: HostId,
    node: String,
    kind: String,
    vmid: u32,
    disk: String,
    grow_gb: u32,
    sudo: Option<SudoInput>,
) -> SysResult<Option<String>> {
    let script = phw::resize_script(&node, &kind, vmid, &disk, grow_gb).ok_or_else(|| invalid("disk"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    let upid = phw::parse_resize(&out).map_err(other)?;
    note(
        &app,
        host_id,
        Level::Info,
        format!("Proxmox: {vmid} {disk} grown"),
        format!("+{grow_gb} GB, {kind} on {node}"),
    );
    Ok(upid)
}

/// PCI and USB devices on a node, for passthrough.
#[tauri::command]
#[specta::specta]
pub async fn pve_host_devices(
    app: AppHandle,
    host_id: HostId,
    node: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveHostDevices> {
    let script = phw::host_devices_script(&node).ok_or_else(|| invalid("node"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(phw::parse_host_devices(&out))
}

// ---- part 5: node network ----

use nightgrid_core::proxmox::network::{self as pnet, PveIfaceSpec, PveNetwork};

/// Seconds the node waits for "keep" before restoring the old network config.
const NET_ROLLBACK_SECS: u32 = 90;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PveNetApply {
    /// Pass to `pve_network_settle` to keep or roll back.
    pub token: String,
    pub upid: Option<String>,
    pub secs: u32,
}

/// A node's interfaces, staged changes (diff) and rollback state.
#[tauri::command]
#[specta::specta]
pub async fn pve_network(
    app: AppHandle,
    host_id: HostId,
    node: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveNetwork> {
    let script = pnet::network_script(&node).ok_or_else(|| invalid("node"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(pnet::parse_network(&out).map_err(other)?)
}

async fn staged(app: &AppHandle, host_id: HostId, sudo: Option<SudoInput>, script: &str) -> SysResult<()> {
    let out = root_run(app, host_id, sudo, script).await?;
    if out.lines().any(|l| l.trim() == "@@ok") {
        Ok(())
    } else {
        Err(other(proxmox::explain(&out)).into())
    }
}

/// Stages a new or changed interface. Nothing changes on the node until apply.
#[tauri::command]
#[specta::specta]
pub async fn pve_iface_save(
    app: AppHandle,
    host_id: HostId,
    node: String,
    spec: PveIfaceSpec,
    create: bool,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = if create { pnet::iface_create_script(&node, &spec) } else { pnet::iface_update_script(&node, &spec) }
        .map_err(other)?;
    staged(&app, host_id, sudo, &script).await
}

#[tauri::command]
#[specta::specta]
pub async fn pve_iface_delete(
    app: AppHandle,
    host_id: HostId,
    node: String,
    iface: String,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = pnet::iface_delete_script(&node, &iface).ok_or_else(|| invalid("interface"))?;
    staged(&app, host_id, sudo, &script).await
}

/// Drops every staged change.
#[tauri::command]
#[specta::specta]
pub async fn pve_network_revert(
    app: AppHandle,
    host_id: HostId,
    node: String,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = pnet::revert_script(&node).ok_or_else(|| invalid("node"))?;
    staged(&app, host_id, sudo, &script).await
}

/// Applies the staged changes with a rollback armed on the node.
#[tauri::command]
#[specta::specta]
pub async fn pve_network_apply(
    app: AppHandle,
    host_id: HostId,
    node: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveNetApply> {
    let token = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    let script = pnet::apply_script(&node, &token, NET_ROLLBACK_SECS).ok_or_else(|| invalid("node"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    let upid = pnet::parse_apply(&out).map_err(other)?;
    note(
        &app,
        host_id,
        Level::Warn,
        format!("Proxmox: network applied on {node}"),
        format!("rollback in {NET_ROLLBACK_SECS} s unless kept"),
    );
    Ok(PveNetApply { token, upid, secs: NET_ROLLBACK_SECS })
}

/// Keeps the applied network change, or restores the old config now.
#[tauri::command]
#[specta::specta]
pub async fn pve_network_settle(
    app: AppHandle,
    host_id: HostId,
    token: String,
    keep: bool,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = if keep { pnet::confirm_script(&token) } else { pnet::rollback_script(&token) }
        .ok_or_else(|| invalid("token"))?;
    staged(&app, host_id, sudo, &script).await?;
    let what = if keep { "kept" } else { "rolled back" };
    note(&app, host_id, Level::Info, format!("Proxmox: network change {what}"), String::new());
    Ok(())
}

// ---- part 6: firewall ----

use nightgrid_core::proxmox::firewall::{self as pfw, PveFirewall, PveFwRuleSpec, PveFwScope};

/// Seconds the node waits for "keep" before restoring its firewall files.
const FW_ROLLBACK_SECS: u32 = 90;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PveFwRun {
    /// A rollback is armed: keep or roll back with this token.
    pub armed: bool,
    pub token: String,
    pub secs: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn pve_firewall(
    app: AppHandle,
    host_id: HostId,
    scope: PveFwScope,
    sudo: Option<SudoInput>,
) -> SysResult<PveFirewall> {
    let script = pfw::firewall_script(&scope).ok_or_else(|| invalid("firewall scope"))?;
    let out = root_run(&app, host_id, sudo, &script).await?;
    Ok(pfw::parse_firewall(&out, &scope.level).map_err(other)?)
}

/// Runs one firewall change, under the session's rollback for host-level scopes.
#[allow(clippy::too_many_arguments)]
async fn fw_run(
    app: &AppHandle,
    host_id: HostId,
    scope: &PveFwScope,
    local: &str,
    token: Option<String>,
    cmd: Result<String, String>,
    what: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveFwRun> {
    let cmd = cmd.map_err(other)?;
    let token = token.filter(|t| pfw::valid_token(t)).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).to_string()
    });
    let script =
        pfw::run_script(scope, local, &token, FW_ROLLBACK_SECS, &cmd).ok_or_else(|| invalid("firewall change"))?;
    let out = root_run(app, host_id, sudo, &script).await?;
    let armed = pfw::parse_run(&out).map_err(other)?;
    let level = if armed { Level::Warn } else { Level::Info };
    note(
        app,
        host_id,
        level,
        format!("Proxmox firewall: {what}"),
        format!("{} level{}", scope.level, if armed { ", rollback armed" } else { "" }),
    );
    Ok(PveFwRun { armed, token, secs: FW_ROLLBACK_SECS })
}

/// Adds a rule (pos None) or replaces the rule at `pos`.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_fw_rule_save(
    app: AppHandle,
    host_id: HostId,
    scope: PveFwScope,
    local: String,
    token: Option<String>,
    pos: Option<u32>,
    digest: String,
    rule: PveFwRuleSpec,
    sudo: Option<SudoInput>,
) -> SysResult<PveFwRun> {
    let cmd = match pos {
        Some(p) => pfw::rule_update_cmd(&scope, p, &digest, &rule),
        None => pfw::rule_create_cmd(&scope, &rule),
    };
    let what = format!("{} rule {} {}", if pos.is_some() { "changed" } else { "added" }, rule.kind, rule.action);
    fw_run(&app, host_id, &scope, &local, token, cmd, what, sudo).await
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_fw_rule_move(
    app: AppHandle,
    host_id: HostId,
    scope: PveFwScope,
    local: String,
    token: Option<String>,
    pos: u32,
    to: u32,
    digest: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveFwRun> {
    let cmd = pfw::rule_move_cmd(&scope, pos, to, &digest);
    fw_run(&app, host_id, &scope, &local, token, cmd, format!("rule {pos} moved to {to}"), sudo).await
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_fw_rule_delete(
    app: AppHandle,
    host_id: HostId,
    scope: PveFwScope,
    local: String,
    token: Option<String>,
    pos: u32,
    digest: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveFwRun> {
    let cmd = pfw::rule_delete_cmd(&scope, pos, &digest);
    fw_run(&app, host_id, &scope, &local, token, cmd, format!("rule {pos} deleted"), sudo).await
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_fw_options(
    app: AppHandle,
    host_id: HostId,
    scope: PveFwScope,
    local: String,
    token: Option<String>,
    enable: bool,
    policy_in: String,
    policy_out: String,
    digest: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveFwRun> {
    let cmd = pfw::options_cmd(&scope, enable, &policy_in, &policy_out, &digest);
    let what = format!("{} (in {policy_in}, out {policy_out})", if enable { "enabled" } else { "disabled" });
    fw_run(&app, host_id, &scope, &local, token, cmd, what, sudo).await
}

/// Security groups and aliases (datacenter level): `op` is group-add,
/// group-delete, alias-add or alias-delete.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn pve_fw_object(
    app: AppHandle,
    host_id: HostId,
    local: String,
    token: Option<String>,
    op: String,
    name: String,
    cidr: String,
    comment: String,
    sudo: Option<SudoInput>,
) -> SysResult<PveFwRun> {
    let cmd = match op.as_str() {
        "group-add" => pfw::group_create_cmd(&name, &comment),
        "group-delete" => pfw::group_delete_cmd(&name),
        "alias-add" => pfw::alias_create_cmd(&name, &cidr, &comment),
        "alias-delete" => pfw::alias_delete_cmd(&name),
        _ => Err("unknown change".into()),
    };
    let scope =
        PveFwScope { level: "cluster".into(), node: String::new(), kind: String::new(), vmid: 0, group: String::new() };
    fw_run(&app, host_id, &scope, &local, token, cmd, format!("{op} {name}"), sudo).await
}

/// Keeps the session's firewall changes, or restores the saved files now.
#[tauri::command]
#[specta::specta]
pub async fn pve_fw_settle(
    app: AppHandle,
    host_id: HostId,
    token: String,
    keep: bool,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let script = if keep { pfw::confirm_script(&token) } else { pfw::rollback_script(&token) }
        .ok_or_else(|| invalid("token"))?;
    staged(&app, host_id, sudo, &script).await?;
    note(
        &app,
        host_id,
        Level::Info,
        format!("Proxmox firewall: changes {}", if keep { "kept" } else { "rolled back" }),
        String::new(),
    );
    Ok(())
}
