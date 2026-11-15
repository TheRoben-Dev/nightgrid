//! System commands (M6): systemd units, the journal, package updates, SMART
//! and reboot, over the host's pooled SSH connection.
//!
//! Root goes through `nightgrid_core::system::sudo` (security rule 4): the
//! password comes from a per-action prompt, or from the keychain only if the
//! user ticked "remember" for that host (off by default). It is checked with
//! a harmless `sudo true` before the real command runs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nightgrid_core::hosts::HostId;
use nightgrid_core::secrets::{Secret, SecretKind, SecretRef, SecretStore};
use nightgrid_core::ssh::client::{ConnectError, Connection};
use nightgrid_core::system::history::{self, HistoryResult};
use nightgrid_core::system::journal::{self, JournalEntry, JournalQuery};
use nightgrid_core::system::packages::{self, Manager, UpdatesReport, UpgradeOp};
use nightgrid_core::system::schedule::{self, Schedules};
use nightgrid_core::system::security::{self, SecurityReport};
use nightgrid_core::system::smart::{self, DisksReport};
use nightgrid_core::system::sudo::{self, Stream, SudoAuth, SudoState};
use nightgrid_core::system::systemd::{self, Unit, UnitAction};
use nightgrid_core::system::SystemError;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager as _, State};
use tokio::sync::watch;

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::term_cmd::connect_host;

/// A message plus the structured cause, so the UI can show an SSH prompt,
/// the sudo prompt or an install hint.
#[derive(Debug, Serialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct SystemCmdError {
    pub message: String,
    pub connect: Option<Box<ConnectError>>,
    pub system: Option<Box<SystemError>>,
}

impl From<SystemError> for SystemCmdError {
    fn from(e: SystemError) -> Self {
        Self { message: e.to_string(), connect: None, system: Some(Box::new(e)) }
    }
}

impl From<ConnectError> for SystemCmdError {
    fn from(e: ConnectError) -> Self {
        Self { message: e.to_string(), connect: Some(Box::new(e)), system: None }
    }
}

pub(crate) type SysResult<T> = Result<T, SystemCmdError>;

/// The sudo password typed into the prompt. Never logged.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SudoInput {
    pub password: String,
    /// Save it in the OS keychain for this host.
    pub remember: bool,
}

impl std::fmt::Debug for SudoInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SudoInput").field("password", &"<redacted>").field("remember", &self.remember).finish()
    }
}

/// Running journal follows and upgrade runs: stop switch by id.
#[derive(Default)]
pub struct SystemRuns(Mutex<HashMap<u32, watch::Sender<bool>>>);

impl SystemRuns {
    pub(crate) fn add(&self, id: u32) -> watch::Receiver<bool> {
        let (tx, rx) = watch::channel(false);
        self.0.lock().expect("poisoned").insert(id, tx);
        rx
    }
    pub(crate) fn remove(&self, id: u32) {
        self.0.lock().expect("poisoned").remove(&id);
    }
    pub(crate) fn stop(&self, id: u32) {
        if let Some(tx) = self.0.lock().expect("poisoned").get(&id) {
            let _ = tx.send(true);
        }
    }
}

fn sudo_ref(host_id: HostId) -> SecretRef {
    SecretRef::host(SecretKind::SudoPassword, host_id)
}

/// How to become root on `host_id`, checking any password before use.
pub(crate) async fn resolve_sudo(
    app: &AppHandle,
    host_id: HostId,
    conn: &Connection,
    input: Option<SudoInput>,
) -> SysResult<SudoAuth> {
    match sudo::probe(conn).await? {
        SudoState::Root => return Ok(SudoAuth::Root),
        SudoState::NoPassword => return Ok(SudoAuth::NoPassword),
        SudoState::Missing => return Err(SystemError::NoSudo.into()),
        SudoState::Password => {}
    }
    let state = app.state::<AppState>();
    if let Some(i) = input {
        let remember = i.remember;
        let secret = Arc::new(Secret::new(i.password));
        let auth = SudoAuth::Password(secret.clone());
        sudo::validate(conn, &auth).await?;
        if remember {
            if let Err(e) = state.keychain.set(&sudo_ref(host_id), &secret) {
                tracing::warn!("couldn't save the sudo password: {e}");
            }
        }
        return Ok(auth);
    }
    match state.keychain.get(&sudo_ref(host_id)) {
        Ok(Some(s)) => {
            let auth = SudoAuth::Password(Arc::new(s));
            match sudo::validate(conn, &auth).await {
                Ok(()) => Ok(auth),
                Err(SystemError::SudoRequired { .. }) => {
                    // Changed on the host: forget it and ask again.
                    let _ = state.keychain.delete(&sudo_ref(host_id));
                    Err(SystemError::SudoRequired { retry: true }.into())
                }
                Err(e) => Err(e.into()),
            }
        }
        Ok(None) => Err(SystemError::SudoRequired { retry: false }.into()),
        Err(e) => {
            tracing::warn!("keychain read failed: {e}");
            Err(SystemError::SudoRequired { retry: false }.into())
        }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub sudo: SudoState,
    /// A sudo password for this host is saved in the keychain.
    pub remembered: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn system_info(app: AppHandle, host_id: HostId) -> SysResult<SystemInfo> {
    let conn = connect_host(&app, host_id).await?;
    let sudo = sudo::probe(&conn).await?;
    let state = app.state::<AppState>();
    let remembered = matches!(state.keychain.get(&sudo_ref(host_id)), Ok(Some(_)));
    Ok(SystemInfo { sudo, remembered })
}

/// Deletes the host's saved sudo password.
#[tauri::command]
#[specta::specta]
pub fn system_sudo_forget(state: State<'_, AppState>, host_id: HostId) -> CmdResult<()> {
    state.keychain.delete(&sudo_ref(host_id))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn system_units(app: AppHandle, host_id: HostId) -> SysResult<Vec<Unit>> {
    let conn = connect_host(&app, host_id).await?;
    Ok(systemd::list(&conn).await?)
}

/// Security checks and listening ports (read-only, no sudo).
#[tauri::command]
#[specta::specta]
pub async fn system_security(app: AppHandle, host_id: HostId) -> SysResult<SecurityReport> {
    let conn = connect_host(&app, host_id).await?;
    Ok(security::audit(&conn).await?)
}

/// Searches the login user's shell history (opt-in setting). Secret-looking
/// lines are dropped in core; nothing is stored.
#[tauri::command]
#[specta::specta]
pub async fn system_history_search(app: AppHandle, host_id: HostId, query: String) -> SysResult<HistoryResult> {
    let enabled = nightgrid_core::settings::AppSettings::load(&app.state::<AppState>().store())
        .map(|s| s.history_search)
        .unwrap_or(false);
    if !enabled {
        return Err(SystemError::other("History search is off. Turn it on in Settings → Terminal.").into());
    }
    let conn = connect_host(&app, host_id).await?;
    Ok(history::search(&conn, &query).await?)
}

/// Systemd timers and cron jobs (read-only).
#[tauri::command]
#[specta::specta]
pub async fn system_schedules(app: AppHandle, host_id: HostId) -> SysResult<Schedules> {
    let conn = connect_host(&app, host_id).await?;
    Ok(schedule::scan(&conn).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn system_unit_status(app: AppHandle, host_id: HostId, unit: String) -> SysResult<String> {
    let conn = connect_host(&app, host_id).await?;
    Ok(systemd::status(&conn, &unit).await?)
}

/// Runs a unit action as root. Stop / restart / disable were confirmed in
/// the UI with the unit and host named (security rule 5).
#[tauri::command]
#[specta::specta]
pub async fn system_unit_action(
    app: AppHandle,
    host_id: HostId,
    unit: String,
    action: UnitAction,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    if !systemd::valid_unit(&unit) {
        return Err(SystemError::other(format!("invalid unit name: {unit:?}")).into());
    }
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    tracing::info!(host = %host_id, unit = %unit, action = action.verb(), "unit action");
    Ok(systemd::act(&conn, &auth, &unit, action).await?)
}

/// What the journal channel carries.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum JournalEvent {
    Entries {
        entries: Vec<JournalEntry>,
    },
    /// Only the user's own entries are visible (read with sudo for all).
    Limited,
    /// A message from journalctl itself (bad filter, ...).
    Notice {
        text: String,
    },
    End {
        error: Option<String>,
    },
}

pub type SystemRunId = u32;

/// Streams the journal; stop with `system_stream_stop`. `elevated` reads it
/// with sudo (needed when the user isn't in `systemd-journal`/`adm`).
#[tauri::command]
#[specta::specta]
pub async fn system_journal_start(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    query: JournalQuery,
    elevated: bool,
    sudo: Option<SudoInput>,
    on_event: Channel<JournalEvent>,
) -> SysResult<SystemRunId> {
    let cmd = journal::command(&query)?;
    let conn = connect_host(&app, host_id).await?;
    let auth = if elevated { resolve_sudo(&app, host_id, &conn, sudo).await? } else { SudoAuth::User };
    let id = state.next_term_id();
    let cancel = state.system_runs.add(id);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        // Lines are parsed as they arrive and sent in batches.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<JournalEvent>();
        let ch = on_event.clone();
        let forward = tauri::async_runtime::spawn(async move {
            while let Some(first) = rx.recv().await {
                let mut entries = Vec::new();
                let mut others = Vec::new();
                let mut next = Some(first);
                while let Some(ev) = next.take() {
                    match ev {
                        JournalEvent::Entries { entries: e } => entries.extend(e),
                        other => others.push(other),
                    }
                    if entries.len() < 1000 {
                        next = rx.try_recv().ok();
                    }
                }
                if !entries.is_empty() && ch.send(JournalEvent::Entries { entries }).is_err() {
                    return;
                }
                for ev in others {
                    let _ = ch.send(ev);
                }
            }
        });
        let mut limited_sent = false;
        let mut on_line = |s: Stream, line: String| -> bool {
            let ev = match s {
                Stream::Out => match journal::parse_entry(&line) {
                    Some(e) => JournalEvent::Entries { entries: vec![e] },
                    None => return true,
                },
                Stream::Err if journal::is_limited_notice(&line) => {
                    if limited_sent {
                        return true;
                    }
                    limited_sent = true;
                    JournalEvent::Limited
                }
                Stream::Err if journal::is_noise(&line) => return true,
                Stream::Err => JournalEvent::Notice { text: line },
            };
            tx.send(ev).is_ok()
        };
        let r = sudo::stream(&conn, &auth, &cmd, Some(cancel), &mut on_line).await;
        drop(on_line);
        drop(tx);
        let _ = forward.await;
        let error = match r {
            Ok(Some(c)) if c != 0 && c != 1 => Some(format!("journalctl exited with {c}")),
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };
        let _ = on_event.send(JournalEvent::End { error });
        app2.state::<AppState>().system_runs.remove(id);
    });
    Ok(id)
}

/// Stops a journal follow (or asks an upgrade to stop: it gets SIGTERM).
#[tauri::command]
#[specta::specta]
pub fn system_stream_stop(state: State<'_, AppState>, run: SystemRunId) -> CmdResult<()> {
    state.system_runs.stop(run);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn system_updates(app: AppHandle, host_id: HostId) -> SysResult<UpdatesReport> {
    let conn = connect_host(&app, host_id).await?;
    Ok(packages::check(&conn).await?)
}

/// What an upgrade run's channel carries.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunEvent {
    Lines { lines: Vec<String> },
    Done { code: Option<i32>, error: Option<String> },
}

/// Refreshes package lists or installs upgrades as root, streaming output.
/// Upgrades were confirmed in the UI with the host and package count named
/// (security rule 5).
#[tauri::command]
#[specta::specta]
pub async fn system_upgrade_run(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    manager: Manager,
    op: UpgradeOp,
    sudo: Option<SudoInput>,
    on_event: Channel<RunEvent>,
) -> SysResult<SystemRunId> {
    let cmd = packages::upgrade_command(manager, op)?;
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    tracing::info!(host = %host_id, manager = manager.label(), op = ?op, "package run");
    let id = state.next_term_id();
    let cancel = state.system_runs.add(id);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let ch = on_event.clone();
        let forward = tauri::async_runtime::spawn(async move {
            while let Some(first) = rx.recv().await {
                let mut lines = vec![first];
                while lines.len() < 500 {
                    match rx.try_recv() {
                        Ok(l) => lines.push(l),
                        Err(_) => break,
                    }
                }
                let _ = ch.send(RunEvent::Lines { lines });
            }
        });
        let mut on_line = |_: Stream, line: String| -> bool {
            let _ = tx.send(line);
            true
        };
        let r = sudo::stream(&conn, &auth, &cmd, Some(cancel), &mut on_line).await;
        drop(tx);
        let _ = forward.await;
        let done = match r {
            Ok(code) => RunEvent::Done { code, error: None },
            Err(e) => RunEvent::Done { code: None, error: Some(e.to_string()) },
        };
        let _ = on_event.send(done);
        app2.state::<AppState>().system_runs.remove(id);
    });
    Ok(id)
}

/// Reboots the host. Confirmed in the UI by typing the host name.
#[tauri::command]
#[specta::specta]
pub async fn system_reboot(app: AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<()> {
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    tracing::info!(host = %host_id, "reboot");
    // The connection usually drops before an answer comes back.
    let r =
        tokio::time::timeout(std::time::Duration::from_secs(5), sudo::run(&conn, &auth, packages::reboot_command()))
            .await;
    if let Ok(Ok(o)) = &r {
        if o.code != 0 && !o.stderr.is_empty() {
            return Err(SystemError::other(format!("reboot failed: {}", o.stderr.trim())).into());
        }
    }
    let state = app.state::<AppState>();
    state.pool.disconnect(host_id).await;
    Ok(())
}

/// SMART health of every disk (needs root).
#[tauri::command]
#[specta::specta]
pub async fn system_disks(app: AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<DisksReport> {
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    Ok(smart::scan(&conn, &auth).await?)
}
