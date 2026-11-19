//! Firewall manager (M10v): read ufw / firewalld / nftables, and change ufw
//! and firewalld behind a rollback armed on the host. A change sticks only
//! when the user keeps it after a fresh SSH connection was tested.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use nightgrid_core::hosts::HostId;
use nightgrid_core::ssh::client;
use nightgrid_core::system::firewall::{self, Backend, FirewallReport, FwChange};
use nightgrid_core::system::sudo::{self, SudoAuth};
use nightgrid_core::system::SystemError;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::{connect_posix, resolve_host};

/// A change waiting for Keep. The sudo auth is held in memory only until
/// then, so keeping or undoing works even if sudo would prompt again.
struct Pending {
    token: String,
    deadline: i64,
    change: String,
    auth: SudoAuth,
}

#[derive(Default)]
pub struct FirewallHub {
    pending: Mutex<HashMap<HostId, Pending>>,
    /// The last report per host, to check changes against.
    reports: Mutex<HashMap<HostId, FirewallReport>>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    pub token: String,
    /// Unix seconds when the host rolls back by itself.
    #[specta(type = u32)]
    pub deadline: i64,
    pub change: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FirewallView {
    pub report: FirewallReport,
    pub pending: Option<PendingChange>,
    /// The port Nightgrid connects to SSH on.
    pub ssh_port: u16,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeCheck {
    pub summary: String,
    /// Why it could lock Nightgrid out; the host name must then be typed.
    pub risk: Option<String>,
}

fn now() -> i64 {
    nightgrid_core::services::check::now_unix()
}

fn ssh_port(app: &AppHandle, host_id: HostId) -> SysResult<(String, u16)> {
    let h = app
        .state::<AppState>()
        .store()
        .get_host(host_id)
        .map_err(SystemError::other)?
        .ok_or_else(|| SystemError::other("no such host"))?;
    Ok((h.name, h.port))
}

fn pending_of(hub: &FirewallHub, host_id: HostId) -> Option<PendingChange> {
    let mut map = hub.pending.lock().expect("poisoned");
    // Past the deadline the host has rolled back by itself.
    if map.get(&host_id).is_some_and(|p| p.deadline + 5 < now()) {
        map.remove(&host_id);
    }
    map.get(&host_id).map(|p| PendingChange { token: p.token.clone(), deadline: p.deadline, change: p.change.clone() })
}

/// Reads the host's firewall (root).
#[tauri::command]
#[specta::specta]
pub async fn firewall_read(app: AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<FirewallView> {
    let (_, port) = ssh_port(&app, host_id)?;
    let conn = connect_posix(&app, host_id, "system.firewall").await?;
    let report = if conn.is_windows() {
        let o = nightgrid_core::win::run(&conn, &nightgrid_core::win::firewall::read_script())
            .await
            .map_err(SystemError::other)?;
        if o.code != 0 {
            return Err(SystemError::other(o.error_text()).into());
        }
        nightgrid_core::win::firewall::parse(&o.stdout).map_err(SystemError::other)?
    } else {
        let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
        let o = sudo::run(&conn, &auth, firewall::READ_COMMAND).await?;
        firewall::parse_read(&o.stdout)
    };
    let hub = app.state::<AppState>().firewall.clone();
    hub.reports.lock().expect("poisoned").insert(host_id, report.clone());
    Ok(FirewallView { pending: pending_of(&hub, host_id), report, ssh_port: port })
}

fn check_against(
    app: &AppHandle,
    host_id: HostId,
    change: &FwChange,
) -> SysResult<(String, FirewallReport, ChangeCheck)> {
    let (name, port) = ssh_port(app, host_id)?;
    let hub = app.state::<AppState>().firewall.clone();
    let report = hub
        .reports
        .lock()
        .expect("poisoned")
        .get(&host_id)
        .cloned()
        .ok_or_else(|| SystemError::other("read the firewall first"))?;
    let want = change.backend();
    let usable = match report.backend {
        Backend::Ufw | Backend::Firewalld | Backend::Windows => report.backend,
        _ => Backend::None,
    };
    if want != usable {
        return Err(SystemError::other("this host's firewall can't take that change").into());
    }
    if let FwChange::UfwDelete { spec } = change {
        if !report.ufw.as_ref().is_some_and(|u| u.rules.iter().any(|r| &r.spec == spec)) {
            return Err(SystemError::other("that rule isn't in the firewall any more: read it again").into());
        }
    }
    firewall::change_command(change).map_err(SystemError::other)?;
    let risk = match &report.windows {
        Some(w) => nightgrid_core::win::firewall::lockout_risk(w, change, port),
        None => firewall::lockout_risk(&report, change, port),
    };
    let check = ChangeCheck { summary: change.describe(), risk };
    Ok((name, report, check))
}

/// What a change does and whether it could lock Nightgrid out.
#[tauri::command]
#[specta::specta]
pub fn firewall_check(app: AppHandle, host_id: HostId, change: FwChange) -> SysResult<ChangeCheck> {
    Ok(check_against(&app, host_id, &change)?.2)
}

/// Applies one change behind a rollback armed for `secs`. A change that
/// could lock Nightgrid out needs `typed` to be the host name (rule 5).
#[tauri::command]
#[specta::specta]
pub async fn firewall_apply(
    app: AppHandle,
    host_id: HostId,
    change: FwChange,
    secs: u32,
    typed: Option<String>,
    sudo: Option<SudoInput>,
) -> SysResult<PendingChange> {
    let (name, _, check) = check_against(&app, host_id, &change)?;
    if check.risk.is_some() && typed.as_deref().map(str::trim) != Some(name.as_str()) {
        return Err(SystemError::other(format!("type {name} to confirm a change that could lock you out")).into());
    }
    let hub = app.state::<AppState>().firewall.clone();
    if pending_of(&hub, host_id).is_some() {
        return Err(SystemError::other("keep or undo the last change first").into());
    }
    let conn = connect_posix(&app, host_id, "system.firewall").await?;
    let windows = change.backend() == Backend::Windows;
    let auth = if windows { SudoAuth::User } else { resolve_sudo(&app, host_id, &conn, sudo).await? };
    let token = now().to_string();
    let secs = secs.clamp(30, 900);
    tracing::info!(host = %host_id, change = %check.summary, secs, "firewall change");
    let o = if windows {
        let script = nightgrid_core::win::firewall::apply_script(&change, &token, secs).map_err(SystemError::other)?;
        let o = nightgrid_core::win::run(&conn, &script).await.map_err(SystemError::other)?;
        sudo::Output { code: o.code, stderr: o.error_text(), stdout: o.stdout }
    } else {
        sudo::run(&conn, &auth, &firewall::apply_command(&change, &token, secs).map_err(SystemError::other)?).await?
    };
    if !o.stdout.contains("@@armed") {
        return Err(SystemError::other(format!("nothing was changed: {}", o.stderr.trim())).into());
    }
    if !o.stdout.contains("@@applied") {
        let why = o.stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the command failed").to_owned();
        let title = format!("Firewall: {}", check.summary);
        crate::journal_cmd::record(
            &app,
            crate::journal_cmd::Change::new(host_id, nightgrid_core::journal::ChangeArea::Firewall, &title)
                .detail(&format!("failed, old rules put back: {why}"))
                .ok(false),
        );
        return Err(SystemError::other(format!("the change failed and the old rules were put back: {why}")).into());
    }
    let p = Pending { token: token.clone(), deadline: now() + i64::from(secs), change: check.summary.clone(), auth };
    let out = PendingChange { token, deadline: p.deadline, change: p.change.clone() };
    hub.pending.lock().expect("poisoned").insert(host_id, p);
    let title = format!("Firewall: {}", check.summary);
    crate::journal_cmd::record(
        &app,
        crate::journal_cmd::Change::new(host_id, nightgrid_core::journal::ChangeArea::Firewall, &title)
            .detail(&format!("applied with a {secs} s rollback armed")),
    );
    Ok(out)
}

/// Opens a brand-new SSH connection (not the pooled one) and runs `true`:
/// what a new login would get after the change. Returns milliseconds.
#[tauri::command]
#[specta::specta]
pub async fn firewall_test(app: AppHandle, host_id: HostId) -> SysResult<u32> {
    let state = app.state::<AppState>();
    let (hop, jumps) = resolve_host(&state, host_id)?;
    let opts = state.connect_options();
    let jump_targets: Vec<_> = jumps.into_iter().map(|h| h.target).collect();
    let start = Instant::now();
    let conn = tokio::time::timeout(Duration::from_secs(15), client::connect(&hop.target, &jump_targets, &opts))
        .await
        .map_err(|_| SystemError::other("a new connection timed out"))??;
    let (code, _, _) = tokio::time::timeout(Duration::from_secs(10), conn.exec("true"))
        .await
        .map_err(|_| SystemError::other("the new connection stopped answering"))?
        .map_err(SystemError::other)?;
    if code != 0 {
        return Err(SystemError::other("the new connection couldn't run a command").into());
    }
    Ok(start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32)
}

async fn finish(app: &AppHandle, host_id: HostId, token: &str, keep: bool) -> SysResult<()> {
    let hub = app.state::<AppState>().firewall.clone();
    let auth = {
        let map = hub.pending.lock().expect("poisoned");
        let p = map
            .get(&host_id)
            .filter(|p| p.token == token)
            .ok_or_else(|| SystemError::other("no change is waiting on this host (it may have rolled back already)"))?;
        p.auth.clone()
    };
    let conn = connect_posix(app, host_id, "system.firewall").await?;
    let o = if conn.is_windows() {
        use nightgrid_core::win::firewall as wf;
        let script =
            if keep { wf::confirm_script() } else { wf::rollback_script(token).map_err(SystemError::other)? };
        let o = nightgrid_core::win::run(&conn, &script).await.map_err(SystemError::other)?;
        sudo::Output { code: o.code, stderr: o.error_text(), stdout: o.stdout }
    } else {
        let cmd = if keep { firewall::confirm_command(token) } else { firewall::rollback_command(token) }
            .map_err(SystemError::other)?;
        sudo::run(&conn, &auth, &cmd).await?
    };
    let ok = if keep { o.stdout.contains("kept") } else { o.stdout.contains("restored") };
    if !ok {
        return Err(SystemError::other(format!("the host didn't confirm: {}", o.stderr.trim())).into());
    }
    hub.pending.lock().expect("poisoned").remove(&host_id);
    tracing::info!(host = %host_id, keep, "firewall change finished");
    Ok(())
}

/// Keeps the change: cancels the rollback on the host.
#[tauri::command]
#[specta::specta]
pub async fn firewall_keep(app: AppHandle, host_id: HostId, token: String) -> SysResult<()> {
    finish(&app, host_id, &token, true).await?;
    let a = nightgrid_core::journal::ChangeArea::Firewall;
    crate::journal_cmd::record(&app, crate::journal_cmd::Change::new(host_id, a, "Firewall change kept"));
    Ok(())
}

/// Puts the old rules back now.
#[tauri::command]
#[specta::specta]
pub async fn firewall_undo(app: AppHandle, host_id: HostId, token: String) -> SysResult<()> {
    finish(&app, host_id, &token, false).await?;
    let a = nightgrid_core::journal::ChangeArea::Firewall;
    crate::journal_cmd::record(&app, crate::journal_cmd::Change::new(host_id, a, "Firewall change undone"));
    Ok(())
}
