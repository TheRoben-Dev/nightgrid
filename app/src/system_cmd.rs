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
use nightgrid_core::power::{self, PowerAction};
use nightgrid_core::power_schedule::{self, InstalledSchedule, ScheduleSpec, ScheduledAction};
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
use nightgrid_core::system::vms::{self, VmAction, VmKind, VmReport};
use nightgrid_core::system::SystemError;
use nightgrid_core::timeline::{EventKind, Level};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager as _, State};
use tokio::sync::watch;

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::term_cmd::{connect_host, connect_posix};

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
#[derive(Deserialize, Type, Clone)]
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
    let sudo = if conn.is_windows() {
        // Windows: administrators act directly over SSH (Root), others can't.
        let o = nightgrid_core::win::run(&conn, nightgrid_core::win::IS_ADMIN).await.map_err(SystemError::other)?;
        if o.stdout.trim() == "True" {
            SudoState::Root
        } else {
            SudoState::Missing
        }
    } else {
        sudo::probe(&conn).await?
    };
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
    let conn = connect_posix(&app, host_id, "system.units").await?;
    if conn.is_windows() {
        return Ok(nightgrid_core::win::services::list(&conn).await?);
    }
    Ok(systemd::list(&conn).await?)
}

/// Security checks and listening ports (read-only, no sudo).
#[tauri::command]
#[specta::specta]
pub async fn system_security(app: AppHandle, host_id: HostId) -> SysResult<SecurityReport> {
    let conn = connect_posix(&app, host_id, "system.security").await?;
    if conn.is_windows() {
        return Ok(nightgrid_core::win::security::audit(&conn).await?);
    }
    Ok(security::audit(&conn).await?)
}

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Records a scan: finds (or saves) the network it was on (D-154), merges
/// it into the device inventory (new/missing/trusted fields), and keeps it as
/// that network's last scan. An error here doesn't lose the scan.
pub(crate) fn with_inventory(
    app: &AppHandle,
    origin: &str,
    mut d: nightgrid_core::netmap::Discovery,
) -> nightgrid_core::netmap::Discovery {
    let now = now_secs();
    let state = app.state::<AppState>();
    let mut store = state.store();
    if let Err(e) = store.net_network_identify(origin, &mut d, now) {
        tracing::warn!("saved networks: {e}");
        d.notes.push(format!("The saved networks couldn't be updated: {e}"));
    }
    if let Err(e) = store.net_inventory_merge(origin, &mut d, now) {
        tracing::warn!("device inventory: {e}");
        d.notes.push(format!("The device inventory couldn't be updated: {e}"));
    }
    if let Err(e) = store.net_network_keep(&d, now) {
        tracing::warn!("saved network scan: {e}");
    }
    d
}

/// What `host_id` sees of its network (M10i): interfaces, gateway,
/// neighbours, names (mDNS, reverse DNS, DHCP leases, NetBIOS, UPnP),
/// Tailscale peers and what faces the internet. `sweep` also pings the
/// host's own LAN subnet (/24 or smaller); `ranges` are routed subnets the
/// host can reach. Only on a click.
#[tauri::command]
#[specta::specta]
pub async fn netmap_discover(
    app: AppHandle,
    host_id: HostId,
    sweep: bool,
    ranges: Vec<String>,
) -> SysResult<nightgrid_core::netmap::Discovery> {
    let conn = connect_posix(&app, host_id, "network.scan").await?;
    tracing::info!(host = %host_id, sweep, ranges = ranges.len(), "network discovery");
    let d = nightgrid_core::netmap::discover(&conn, sweep, &ranges).await.map_err(SystemError::other)?;
    Ok(with_inventory(&app, &host_id.to_string(), d))
}

/// Network discovery run on this computer (macOS and Linux), for the map's
/// "This computer" option. Same script as on a host, run with the local `sh`.
#[tauri::command]
#[specta::specta]
pub async fn netmap_discover_local(
    app: AppHandle,
    sweep: bool,
    ranges: Vec<String>,
) -> SysResult<nightgrid_core::netmap::Discovery> {
    tracing::info!(sweep, ranges = ranges.len(), "network discovery from this computer");
    let d = nightgrid_core::netmap::discover_local(sweep, &ranges).await.map_err(SystemError::other)?;
    Ok(with_inventory(&app, "local", d))
}

/// Every device the network map has seen, newest first.
#[tauri::command]
#[specta::specta]
pub fn netmap_inventory(state: State<'_, AppState>) -> CmdResult<Vec<nightgrid_core::netinv::NetDevice>> {
    Ok(state.store().net_devices()?)
}

/// Marks a device as trusted (known and expected) or not.
#[tauri::command]
#[specta::specta]
pub fn netmap_device_trust(state: State<'_, AppState>, key: String, trusted: bool) -> CmdResult<()> {
    Ok(state.store().net_device_trust(&key, trusted)?)
}

/// Forgets a device; it counts as new the next time a scan sees it.
#[tauri::command]
#[specta::specta]
pub fn netmap_device_forget(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    Ok(state.store().net_device_forget(&key)?)
}

/// Saved networks (D-154), most recently seen first.
#[tauri::command]
#[specta::specta]
pub fn netmap_networks(state: State<'_, AppState>) -> CmdResult<Vec<nightgrid_core::netnets::SavedNetwork>> {
    Ok(state.store().net_networks()?)
}

/// The last scan of a saved network, to show it (or overlay it) without a rescan.
#[tauri::command]
#[specta::specta]
pub fn netmap_network_scan(
    state: State<'_, AppState>,
    id: i32,
) -> CmdResult<Option<nightgrid_core::netmap::Discovery>> {
    Ok(state.store().net_network_snapshot(id.into())?)
}

#[tauri::command]
#[specta::specta]
pub fn netmap_network_rename(
    state: State<'_, AppState>,
    id: i32,
    name: String,
) -> CmdResult<nightgrid_core::netnets::SavedNetwork> {
    Ok(state.store().net_network_rename(id.into(), &name)?)
}

/// Forgets a saved network; its devices stay in the inventory.
#[tauri::command]
#[specta::specta]
pub fn netmap_network_forget(state: State<'_, AppState>, id: i32) -> CmdResult<()> {
    Ok(state.store().net_network_forget(id.into())?)
}

/// Folds one saved network into another (the router was replaced).
#[tauri::command]
#[specta::specta]
pub fn netmap_network_merge(
    state: State<'_, AppState>,
    from: i32,
    into: i32,
) -> CmdResult<nightgrid_core::netnets::SavedNetwork> {
    Ok(state.store().net_network_merge(from.into(), into.into())?)
}

/// Runs a network tool (ping, path, DNS, ports, HTTP/TLS, neighbours,
/// connections) on a host, or on this computer when `host_id` is null (D-142).
#[tauri::command]
#[specta::specta]
pub async fn net_tool(
    app: AppHandle,
    host_id: Option<HostId>,
    req: nightgrid_core::nettools::ToolRequest,
) -> SysResult<nightgrid_core::nettools::ToolResult> {
    tracing::info!(tool = ?req.tool, host = ?host_id, "network tool");
    let r = match host_id {
        Some(id) => {
            let conn = connect_posix(&app, id, "nettools").await?;
            nightgrid_core::nettools::run(&conn, &req).await
        }
        None => nightgrid_core::nettools::run_local(&req).await,
    };
    Ok(r.map_err(SystemError::other)?)
}

/// Web security check of `ips` (D-145): expired or self-signed
/// certificates, admin pages of gear with default passwords, logins over
/// plain HTTP. Runs on a host, or this computer when `host_id` is null.
/// Reads one page or certificate per port; never tries a login.
#[tauri::command]
#[specta::specta]
pub async fn netmap_security(
    app: AppHandle,
    host_id: Option<HostId>,
    ips: Vec<String>,
) -> SysResult<Vec<nightgrid_core::netsec::NetFinding>> {
    tracing::info!(host = ?host_id, devices = ips.len(), "network security check");
    let r = match host_id {
        Some(id) => {
            let conn = connect_posix(&app, id, "network.scan").await?;
            nightgrid_core::netsec::web_check(&conn, &ips).await
        }
        None => nightgrid_core::netsec::web_check_local(&ips).await,
    };
    Ok(r.map_err(SystemError::other)?)
}

/// The maker of a MAC address (offline list).
#[tauri::command]
#[specta::specta]
pub fn net_mac_vendor(mac: String) -> Option<String> {
    nightgrid_core::netmap::mac_vendor(&mac)
}

/// Sends a Wake-on-LAN packet for `mac` from a host (null: this computer).
#[tauri::command]
#[specta::specta]
pub async fn net_wake(app: AppHandle, host_id: Option<HostId>, mac: String) -> SysResult<String> {
    let m = power::parse_mac(&mac).map_err(SystemError::other)?;
    tracing::info!(host = ?host_id, "wake-on-lan from the tools");
    match host_id {
        Some(id) => {
            let conn = connect_host(&app, id).await?;
            relay_wake(&conn, &m).await.map_err(SystemError::other)?;
        }
        None => power::wake_local(&m, None).await.map_err(SystemError::other)?,
    }
    Ok(power::format_mac(&m))
}

/// Sends a magic packet from a relay host (sh, or PowerShell on Windows).
async fn relay_wake(conn: &Connection, mac: &[u8; 6]) -> Result<(), String> {
    let t = std::time::Duration::from_secs(10);
    if conn.is_windows() {
        let o = tokio::time::timeout(t, nightgrid_core::win::run(conn, &power::wake_relay_windows(mac)))
            .await
            .map_err(|_| "the host took too long to answer".to_owned())??;
        return if o.code == 0 { Ok(()) } else { Err(o.error_text()) };
    }
    let (code, _, err) = tokio::time::timeout(t, conn.exec(&power::wake_relay_command(mac)))
        .await
        .map_err(|_| "the host took too long to answer".to_owned())?
        .map_err(|e| e.to_string())?;
    if code != 0 {
        return Err(String::from_utf8_lossy(&err).trim().to_owned());
    }
    Ok(())
}

/// The network map as the user arranged it: positions, hand-set parents
/// and the expected picture (D-141).
#[tauri::command]
#[specta::specta]
pub fn netmap_layout_get(state: State<'_, AppState>) -> CmdResult<Vec<nightgrid_core::netlayout::LayoutEntry>> {
    Ok(state.store().netmap_layout()?)
}

/// Saves map positions and parents; `replace` makes the saved layout exactly
/// these entries (reset, or save as expected).
#[tauri::command]
#[specta::specta]
pub fn netmap_layout_save(
    state: State<'_, AppState>,
    entries: Vec<nightgrid_core::netlayout::LayoutEntry>,
    replace: bool,
) -> CmdResult<()> {
    Ok(state.store().netmap_layout_save(&entries, replace)?)
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
    let conn = connect_posix(&app, host_id, "drift.history").await?;
    Ok(history::search(&conn, &query).await?)
}

/// Systemd timers and cron jobs (read-only).
#[tauri::command]
#[specta::specta]
pub async fn system_schedules(app: AppHandle, host_id: HostId) -> SysResult<Schedules> {
    let conn = connect_posix(&app, host_id, "system.schedules").await?;
    if conn.is_windows() {
        return Ok(nightgrid_core::win::tasks::scan(&conn).await?);
    }
    Ok(schedule::scan(&conn).await?)
}

/// Runs, enables or disables a Windows scheduled task (Windows hosts only;
/// disabling was confirmed in the UI).
#[tauri::command]
#[specta::specta]
pub async fn system_task_action(
    app: AppHandle,
    host_id: HostId,
    task: String,
    action: nightgrid_core::win::tasks::TaskAction,
) -> SysResult<()> {
    let conn = connect_posix(&app, host_id, "system.schedules").await?;
    if !conn.is_windows() {
        return Err(SystemError::other("Scheduled tasks are a Windows feature; this host uses timers and cron.").into());
    }
    tracing::info!(host = %host_id, task = %task, action = ?action, "task action");
    let r = nightgrid_core::win::tasks::act(&conn, &task, action).await;
    let verb = match action {
        nightgrid_core::win::tasks::TaskAction::Run => "Ran",
        nightgrid_core::win::tasks::TaskAction::Enable => "Enabled",
        nightgrid_core::win::tasks::TaskAction::Disable => "Disabled",
    };
    let (level, detail) = match &r {
        Ok(()) => (Level::Info, "from Nightgrid".to_owned()),
        Err(e) => (Level::Warn, format!("from Nightgrid, failed: {e}")),
    };
    crate::timeline_cmd::record(&app, host_id, EventKind::Unit, level, &format!("{verb} task {task}"), &detail);
    Ok(r?)
}

#[tauri::command]
#[specta::specta]
pub async fn system_unit_status(app: AppHandle, host_id: HostId, unit: String) -> SysResult<String> {
    let conn = connect_posix(&app, host_id, "system.units").await?;
    if conn.is_windows() {
        return Ok(nightgrid_core::win::services::status(&conn, &unit).await?);
    }
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
    let conn = connect_posix(&app, host_id, "system.units").await?;
    let windows = conn.is_windows();
    if !windows && !systemd::valid_unit(&unit) {
        return Err(SystemError::other(format!("invalid unit name: {unit:?}")).into());
    }
    tracing::info!(host = %host_id, unit = %unit, action = action.verb(), "unit action");
    let r = if windows {
        nightgrid_core::win::services::act(&conn, &unit, action).await
    } else {
        let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
        systemd::act(&conn, &auth, &unit, action).await
    };
    let (level, detail) = match &r {
        Ok(()) => (Level::Info, "from Nightgrid".to_owned()),
        Err(e) => (Level::Warn, format!("from Nightgrid, failed: {e}")),
    };
    crate::timeline_cmd::record(
        &app,
        host_id,
        EventKind::Unit,
        level,
        &format!("{} {unit}", cap(action.verb())),
        &detail,
    );
    Ok(r?)
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
    let conn = connect_posix(&app, host_id, "system.journal").await?;
    if conn.is_windows() {
        return windows_events(app, state, conn, query, elevated, on_event);
    }
    let cmd = journal::command(&query)?;
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

/// The Event Log in place of the journal on Windows hosts. `elevated` adds
/// the Security log (Windows only shows it to administrators).
fn windows_events(
    app: AppHandle,
    state: State<'_, AppState>,
    conn: Arc<Connection>,
    query: JournalQuery,
    elevated: bool,
    on_event: Channel<JournalEvent>,
) -> SysResult<SystemRunId> {
    use nightgrid_core::win::events;
    let script = events::script(&query, elevated)?;
    let id = state.next_term_id();
    let cancel = state.system_runs.add(id);
    tauri::async_runtime::spawn(async move {
        // Rows are sent in batches, like the journal's.
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
        let mut on_line = |s: Stream, line: String| -> bool {
            let ev = match s {
                Stream::Out => match events::parse_line(&line) {
                    Some(e) => JournalEvent::Entries { entries: vec![e] },
                    None => return true,
                },
                Stream::Err if events::is_empty_notice(&line) => return true,
                Stream::Err => JournalEvent::Notice { text: line },
            };
            tx.send(ev).is_ok()
        };
        let r = nightgrid_core::win::stream(&conn, &script, Some(cancel), &mut on_line).await;
        drop(tx);
        let _ = forward.await;
        let error = match r {
            Ok(Some(c)) if c != 0 => Some(format!("reading the Event Log exited with {c}")),
            Ok(_) => None,
            Err(e) => Some(e),
        };
        let _ = on_event.send(JournalEvent::End { error });
        app.state::<AppState>().system_runs.remove(id);
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
    let conn = connect_posix(&app, host_id, "system.updates").await?;
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
    let conn = connect_posix(&app, host_id, "system.updates").await?;
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
        let (level, detail) = match &r {
            Ok(Some(0)) => (Level::Info, "finished".to_owned()),
            Ok(Some(c)) => (Level::Warn, format!("exit code {c}")),
            Ok(None) => (Level::Warn, "stopped before it finished".to_owned()),
            Err(e) => (Level::Warn, e.to_string()),
        };
        if op == UpgradeOp::Upgrade {
            let title = format!("{} upgrade from Nightgrid", manager.label());
            crate::timeline_cmd::record(&app2, host_id, EventKind::Packages, level, &title, &detail);
        }
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
    system_power(app, host_id, PowerAction::Reboot, sudo).await
}

/// Reboots, shuts down or suspends the host (as root). Confirmed in the UI by
/// typing the host name. The host then shows "Rebooting", "Shutting down"
/// or "Suspended" instead of "Offline", with no offline alert.
#[tauri::command]
#[specta::specta]
pub async fn system_power(
    app: AppHandle,
    host_id: HostId,
    action: PowerAction,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let conn = connect_host(&app, host_id).await?;
    let windows = conn.is_windows();
    // Windows: the login user acts directly (OpenSSH gives administrators
    // their full rights); there's no sudo to resolve.
    let auth = if windows { SudoAuth::User } else { resolve_sudo(&app, host_id, &conn, sudo).await? };
    tracing::info!(host = %host_id, ?action, windows, "power action");
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // Noted first: the host may stop answering before the command returns.
    power::note(host_id, action.state(), format!("{} from Nightgrid", past(action)), now);
    crate::timeline_cmd::record(
        &app,
        host_id,
        EventKind::Power,
        Level::Info,
        &format!("{} from Nightgrid", cap(past(action))),
        "",
    );
    // The connection usually drops before an answer comes back.
    if windows {
        let r = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            nightgrid_core::win::run(&conn, action.windows_script()),
        )
        .await;
        if let Ok(Ok(o)) = &r {
            if o.code != 0 {
                power::clear(host_id);
                return Err(SystemError::other(format!("{} failed: {}", action.verb(), o.error_text())).into());
            }
        }
        let state = app.state::<AppState>();
        state.pool.disconnect(host_id).await;
        return Ok(());
    }
    let r = tokio::time::timeout(std::time::Duration::from_secs(5), sudo::run(&conn, &auth, action.command())).await;
    if let Ok(Ok(o)) = &r {
        if o.code != 0 && !o.stderr.is_empty() {
            power::clear(host_id);
            return Err(SystemError::other(format!("{} failed: {}", action.verb(), o.stderr.trim())).into());
        }
    }
    let state = app.state::<AppState>();
    state.pool.disconnect(host_id).await;
    Ok(())
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn past(a: PowerAction) -> &'static str {
    match a {
        PowerAction::Reboot => "rebooted",
        PowerAction::Shutdown => "shut down",
        PowerAction::Suspend => "suspended",
    }
}

/// How a wake-up was sent.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WakeSent {
    /// The relay host's name, or `None` when sent from this computer.
    pub via: Option<String>,
    pub mac: String,
}

/// Sends a Wake-on-LAN magic packet to the host's MAC address: from its relay
/// host over SSH when one is set (works away from home), else as a broadcast
/// from this computer.
#[tauri::command]
#[specta::specta]
pub async fn host_wake(app: AppHandle, host_id: HostId) -> SysResult<WakeSent> {
    let (host, relay) = {
        let state = app.state::<AppState>();
        let store = state.store();
        let h =
            store.get_host(host_id).map_err(SystemError::other)?.ok_or_else(|| SystemError::other("host not found"))?;
        let relay = match h.wake_relay_id {
            Some(r) => store.get_host(r).map_err(SystemError::other)?,
            None => None,
        };
        (h, relay)
    };
    let mac = host.mac_address.as_deref().ok_or_else(|| SystemError::other("set the host's MAC address first"))?;
    let mac = power::parse_mac(mac).map_err(SystemError::other)?;
    tracing::info!(host = %host_id, relay = relay.as_ref().map(|r| r.id), "wake-on-lan");
    match relay {
        Some(r) => {
            let conn = connect_host(&app, r.id).await?;
            relay_wake(&conn, &mac).await.map_err(|e| SystemError::other(format!("{}: {e}", r.name)))?;
            crate::timeline_cmd::record(
                &app,
                host_id,
                EventKind::Power,
                Level::Info,
                "Wake-up sent",
                &format!("via {}", r.name),
            );
            Ok(WakeSent { via: Some(r.name), mac: power::format_mac(&mac) })
        }
        None => {
            power::wake_local(&mac, None).await.map_err(SystemError::other)?;
            crate::timeline_cmd::record(
                &app,
                host_id,
                EventKind::Power,
                Level::Info,
                "Wake-up sent",
                "from this computer",
            );
            Ok(WakeSent { via: None, mac: power::format_mac(&mac) })
        }
    }
}

/// A Nightgrid power schedule and the host it is installed on (the host
/// itself, or its wake relay for wake-ups).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostSchedule {
    pub on_host_id: HostId,
    pub on_host_name: String,
    #[serde(flatten)]
    pub schedule: InstalledSchedule,
}

/// The host and, when it has a MAC address and a relay, the relay.
fn host_and_relay(
    app: &AppHandle,
    host_id: HostId,
) -> SysResult<(nightgrid_core::hosts::Host, Option<nightgrid_core::hosts::Host>)> {
    let state = app.state::<AppState>();
    let store = state.store();
    let h = store.get_host(host_id).map_err(SystemError::other)?.ok_or_else(|| SystemError::other("host not found"))?;
    let relay = match h.wake_relay_id {
        Some(r) if h.mac_address.is_some() => store.get_host(r).map_err(SystemError::other)?,
        _ => None,
    };
    Ok((h, relay))
}

async fn list_schedules(app: &AppHandle, id: HostId) -> SysResult<Vec<InstalledSchedule>> {
    let conn = connect_posix(app, id, "power.schedules").await?;
    let (_, out, _) = tokio::time::timeout(std::time::Duration::from_secs(15), conn.exec(power_schedule::LIST_COMMAND))
        .await
        .map_err(|_| SystemError::other("the host took too long to answer"))?
        .map_err(SystemError::other)?;
    Ok(power_schedule::parse_list(&String::from_utf8_lossy(&out)))
}

/// Power schedules for a host: its own timers, plus wake-ups for it on its relay.
#[tauri::command]
#[specta::specta]
pub async fn power_schedules(app: AppHandle, host_id: HostId) -> SysResult<Vec<HostSchedule>> {
    let (host, relay) = host_and_relay(&app, host_id)?;
    let mut out: Vec<HostSchedule> = list_schedules(&app, host_id)
        .await?
        .into_iter()
        .filter(|s| s.spec.action != ScheduledAction::Wake)
        .map(|schedule| HostSchedule { on_host_id: host.id, on_host_name: host.name.clone(), schedule })
        .collect();
    if let (Some(r), Some(mac)) = (relay, host.mac_address.as_deref()) {
        let mac = power::format_mac(&power::parse_mac(mac).map_err(SystemError::other)?);
        // The relay may be off; its schedules are a bonus, not an error.
        match list_schedules(&app, r.id).await {
            Ok(list) => {
                out.extend(
                    list.into_iter()
                        .filter(|s| s.spec.mac.as_deref() == Some(mac.as_str()))
                        .map(|schedule| HostSchedule { on_host_id: r.id, on_host_name: r.name.clone(), schedule }),
                )
            }
            Err(e) => tracing::info!("wake schedules on {}: {e}", r.name),
        }
    }
    Ok(out)
}

/// Installs (or replaces) a schedule as a systemd timer. Wake-ups go on the
/// host's relay, with the host's MAC address. Needs root on that host.
#[tauri::command]
#[specta::specta]
pub async fn power_schedule_set(
    app: AppHandle,
    host_id: HostId,
    spec: ScheduleSpec,
    sudo: Option<SudoInput>,
) -> SysResult<HostSchedule> {
    let (host, relay) = host_and_relay(&app, host_id)?;
    let (target, spec) = if spec.action == ScheduledAction::Wake {
        let r = relay.ok_or_else(|| {
            SystemError::other(
                "a scheduled wake-up runs on another host: set the MAC address and Wake from in the host editor",
            )
        })?;
        (r, ScheduleSpec { mac: host.mac_address.clone(), ..spec })
    } else {
        (host, spec)
    };
    let spec = spec.normalized().map_err(SystemError::other)?;
    let cmd = power_schedule::install_command(&spec).map_err(SystemError::other)?;
    let conn = connect_posix(&app, target.id, "power.schedules").await?;
    let auth = resolve_sudo(&app, target.id, &conn, sudo).await?;
    tracing::info!(host = %target.id, unit = %spec.unit(), "install power schedule");
    let o = sudo::run(&conn, &auth, &cmd).await?;
    if o.code != 0 || !o.stdout.contains("installed") {
        return Err(SystemError::other(format!("couldn't install the schedule: {}", o.stderr.trim())).into());
    }
    let unit = spec.unit();
    let next = list_schedules(&app, target.id)
        .await
        .ok()
        .and_then(|l| l.into_iter().find(|s| s.unit == unit))
        .and_then(|s| s.next);
    Ok(HostSchedule {
        on_host_id: target.id,
        on_host_name: target.name,
        schedule: InstalledSchedule { unit, spec, next },
    })
}

/// Removes a Nightgrid schedule from the host it's installed on.
#[tauri::command]
#[specta::specta]
pub async fn power_schedule_remove(
    app: AppHandle,
    on_host_id: HostId,
    unit: String,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    let cmd = power_schedule::remove_command(&unit).map_err(SystemError::other)?;
    let conn = connect_posix(&app, on_host_id, "power.schedules").await?;
    let auth = resolve_sudo(&app, on_host_id, &conn, sudo).await?;
    tracing::info!(host = %on_host_id, %unit, "remove power schedule");
    let o = sudo::run(&conn, &auth, &cmd).await?;
    if o.code != 0 || !o.stdout.contains("removed") {
        return Err(SystemError::other(format!("couldn't remove the schedule: {}", o.stderr.trim())).into());
    }
    Ok(())
}

/// SMART health of every disk (needs root).
#[tauri::command]
#[specta::specta]
pub async fn system_disks(app: AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<DisksReport> {
    let conn = connect_posix(&app, host_id, "system.disks").await?;
    if conn.is_windows() {
        return Ok(nightgrid_core::win::disks::disks(&conn).await?);
    }
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    Ok(smart::scan(&conn, &auth).await?)
}

/// ZFS pools, RAID arrays and btrfs filesystems (no root needed).
#[tauri::command]
#[specta::specta]
pub async fn system_pools(app: AppHandle, host_id: HostId) -> SysResult<nightgrid_core::system::pools::PoolsReport> {
    let conn = connect_posix(&app, host_id, "system.disks").await?;
    if conn.is_windows() {
        return Ok(nightgrid_core::win::disks::pools(&conn).await?);
    }
    Ok(nightgrid_core::system::pools::scan(&conn).await?)
}

/// Usage trend and "full in about X days" per mount, from usage recorded
/// while the host was monitored (local only, no connection).
#[tauri::command]
#[specta::specta]
pub fn disk_forecasts(
    state: State<'_, AppState>,
    host_id: HostId,
) -> CmdResult<Vec<nightgrid_core::diskforecast::MountForecast>> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let list = state.store().disk_forecasts(host_id, now)?;
    Ok(list)
}

/// VMs and containers on a libvirt or Proxmox host. `elevate` reads as root
/// (when this login may not use virsh / qm / pct).
#[tauri::command]
#[specta::specta]
pub async fn system_vms(
    app: AppHandle,
    host_id: HostId,
    elevate: bool,
    sudo: Option<SudoInput>,
) -> SysResult<VmReport> {
    let conn = connect_posix(&app, host_id, "system.vms").await?;
    let auth = if elevate { resolve_sudo(&app, host_id, &conn, sudo).await? } else { SudoAuth::User };
    Ok(vms::scan(&conn, &auth).await?)
}

/// Starts, shuts down, reboots or stops a guest. Everything but start was
/// confirmed in the UI with the guest and host named (security rule 5).
#[tauri::command]
#[specta::specta]
pub async fn system_vm_action(
    app: AppHandle,
    host_id: HostId,
    kind: VmKind,
    id: String,
    action: VmAction,
    elevate: bool,
    sudo: Option<SudoInput>,
) -> SysResult<()> {
    if !vms::valid_id(kind, &id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")).into());
    }
    let conn = connect_posix(&app, host_id, "system.vms").await?;
    let auth = if elevate { resolve_sudo(&app, host_id, &conn, sudo).await? } else { SudoAuth::User };
    tracing::info!(host = %host_id, guest = %id, ?kind, ?action, "vm action");
    Ok(vms::act(&conn, &auth, kind, &id, action).await?)
}
