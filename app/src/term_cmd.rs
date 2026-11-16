//! Terminal commands: local PTYs and SSH shells streamed to the UI over a
//! `tauri::ipc::Channel`, plus the host-key and credential prompts that an SSH
//! connect can raise.

use std::collections::HashSet;
use std::sync::Arc;

use base64::Engine;
use nightgrid_core::edit_hook;
use nightgrid_core::hosts::HostId;
use nightgrid_core::secrets::{Secret, SecretKind, SecretRef, SecretStore};
use nightgrid_core::ssh::client::{self, ConnectError};
use nightgrid_core::ssh::resolve::resolve;
use nightgrid_core::term::{self, ConnectFuture, Connector, Sink, TermEvent, TermState};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

pub type TermId = u32;

/// What the terminal channel carries to the UI.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TermMessage {
    /// Output bytes, base64 encoded (terminal output is not always UTF-8).
    Data {
        b64: String,
    },
    Status {
        state: TermState,
        attempt: u32,
        message: Option<String>,
    },
    Exit {
        code: Option<i32>,
    },
}

fn sink_for(channel: Channel<TermMessage>) -> Sink {
    let b64 = base64::engine::general_purpose::STANDARD;
    Arc::new(move |ev| {
        let msg = match ev {
            TermEvent::Data(d) => TermMessage::Data { b64: b64.encode(d) },
            TermEvent::Status { state, attempt, message } => TermMessage::Status { state, attempt, message },
            TermEvent::Exit { code } => TermMessage::Exit { code },
        };
        // The webview may have gone away; nothing useful to do then.
        let _ = channel.send(msg);
    })
}

#[tauri::command]
#[specta::specta]
pub fn term_open_local(
    state: State<'_, AppState>,
    cols: u16,
    rows: u16,
    on_event: Channel<TermMessage>,
) -> CmdResult<TermId> {
    let handle = term::spawn_local(None, None, cols, rows, sink_for(on_event))?;
    let id = state.next_term_id();
    state.terms.lock().expect("poisoned").insert(id, handle);
    Ok(id)
}

/// Result of opening an SSH terminal. `needsUser` carries the prompt the UI
/// must show (host key, password, passphrase) before calling again.
#[derive(Debug, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TermOpen {
    Opened { id: TermId },
    NeedsUser { error: ConnectError },
}

/// Returns the host's pooled connection, connecting if needed. Re-resolves
/// the host each time so edits and newly entered secrets take effect.
// Same error type as `client::connect` and the terminal `Connector`; boxing it
// here alone would just move the conversion.
#[allow(clippy::result_large_err)]
pub(crate) async fn connect_host(app: &AppHandle, host_id: HostId) -> Result<Arc<client::Connection>, ConnectError> {
    let state = app.state::<AppState>();
    let (hop, jumps) = resolve_host(&state, host_id)?;
    let mut opts = state.connect_options();
    // Banners (a Tailscale SSH approval link) go to the UI while this waits.
    let notices = app.clone();
    opts.on_notice = Some(client::NoticeSink(Arc::new(move |n| {
        tracing::info!(host_id, target = %n.target, url = ?n.url, "login notice from the server");
        let st = notices.state::<AppState>();
        st.auth_notices.lock().expect("poisoned").insert(host_id, (n, std::time::Instant::now()));
    })));
    let jump_targets: Vec<_> = jumps.into_iter().map(|h| h.target).collect();
    let res = state.pool.get(host_id, || client::connect(&hop.target, &jump_targets, &opts)).await;
    // Done waiting either way: the prompt goes away.
    state.auth_notices.lock().expect("poisoned").remove(&host_id);
    let conn = res?;
    crate::tunnels_cmd::host_connected(app, host_id, &conn);
    Ok(conn)
}

/// A login notice for a host that is still connecting.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostAuthNotice {
    pub host_id: HostId,
    pub notice: client::AuthNotice,
}

/// What servers are saying to hosts that are logging in right now (a
/// Tailscale SSH check's approval link). Polled by the UI; cheap.
#[tauri::command]
#[specta::specta]
pub fn ssh_auth_notices(state: State<'_, AppState>) -> Vec<HostAuthNotice> {
    let mut m = state.auth_notices.lock().expect("poisoned");
    m.retain(|_, (_, at)| at.elapsed() < std::time::Duration::from_secs(600));
    m.iter().map(|(id, (n, _))| HostAuthNotice { host_id: *id, notice: n.clone() }).collect()
}

/// The host's final hop and its ProxyJump chain, with remembered secrets.
#[allow(clippy::result_large_err)]
pub(crate) fn resolve_host(
    state: &AppState,
    host_id: HostId,
) -> Result<(nightgrid_core::ssh::resolve::Hop, Vec<nightgrid_core::ssh::resolve::Hop>), ConnectError> {
    let store = state.store();
    let remembering: HashSet<HostId> = store
        .list_hosts()
        .map_err(|e| ConnectError::Other { message: e.to_string() })?
        .into_iter()
        .filter(|h| h.remember_secret)
        .map(|h| h.id)
        .collect();
    let lookup = |h: Option<HostId>, k: SecretKind, label: &str| state.lookup_secret(&remembering, h, k, label);
    resolve(&store, host_id, state.home.as_deref(), &lookup).map_err(|e| ConnectError::Other { message: e.to_string() })
}

/// Builds the reconnect-capable connector for a stored host.
fn connector_for(app: AppHandle, host_id: HostId) -> Connector {
    Arc::new(move |_fresh: bool| -> ConnectFuture {
        let app = app.clone();
        Box::pin(async move { connect_host(&app, host_id).await })
    })
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)] // a Tauri command: arguments come from the UI
pub async fn term_open_ssh(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    cols: u16,
    rows: u16,
    edit_token: Option<String>,
    edit: bool,
    hints: bool,
    on_event: Channel<TermMessage>,
) -> CmdResult<TermOpen> {
    // With a token, the session-only hook (nothing installed on the host)
    // makes `nano FILE` / `$EDITOR` open files in the app's editor and/or
    // reports the program in front for key hints.
    let opts = edit_hook::HookOptions { edit, hints };
    let command = match edit_token {
        Some(t) if edit_hook::valid_token(&t) && opts.any() => Some(edit_hook::bootstrap_command(&t, opts)),
        Some(_) if opts.any() => return Err(CommandError::msg("invalid session token")),
        _ => None,
    };
    let connector = connector_for(app.clone(), host_id);
    match term::spawn_ssh(connector, command, cols, rows, sink_for(on_event)).await {
        Ok(handle) => {
            let id = state.next_term_id();
            state.terms.lock().expect("poisoned").insert(id, handle);
            Ok(TermOpen::Opened { id })
        }
        Err(e) if e.needs_user() => Ok(TermOpen::NeedsUser { error: e }),
        Err(e) => Err(CommandError::msg(e.to_string())),
    }
}

/// Opens a terminal tab with an interactive shell inside a container
/// (`docker exec -it`, bash if present, else sh). Reconnects re-run it.
#[tauri::command]
#[specta::specta]
pub async fn term_open_docker_exec(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    container: String,
    cols: u16,
    rows: u16,
    on_event: Channel<TermMessage>,
) -> CmdResult<TermOpen> {
    let docker = match crate::docker_cmd::docker_for(&app, host_id).await {
        Ok(d) => d,
        Err(e) => match e.connect {
            Some(c) if c.needs_user() => return Ok(TermOpen::NeedsUser { error: *c }),
            _ => return Err(CommandError::msg(e.message)),
        },
    };
    let command = crate::docker_cmd::exec_command(&docker, &container)?;
    let connector = connector_for(app.clone(), host_id);
    match term::spawn_ssh(connector, Some(command), cols, rows, sink_for(on_event)).await {
        Ok(handle) => {
            let id = state.next_term_id();
            state.terms.lock().expect("poisoned").insert(id, handle);
            Ok(TermOpen::Opened { id })
        }
        Err(e) if e.needs_user() => Ok(TermOpen::NeedsUser { error: e }),
        Err(e) => Err(CommandError::msg(e.to_string())),
    }
}

/// Opens a terminal tab on a guest's console (`virsh console`, `qm
/// terminal`, `pct enter`). With `as_root` it runs under sudo and sudo's own
/// prompt asks in the terminal (nothing goes on the command line).
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn term_open_vm_console(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    kind: nightgrid_core::system::vms::VmKind,
    id: String,
    as_root: bool,
    cols: u16,
    rows: u16,
    on_event: Channel<TermMessage>,
) -> CmdResult<TermOpen> {
    let command = nightgrid_core::system::vms::console_command(kind, &id, as_root)
        .map_err(|e| CommandError::msg(e.to_string()))?;
    let connector = connector_for(app.clone(), host_id);
    match term::spawn_ssh(connector, Some(command), cols, rows, sink_for(on_event)).await {
        Ok(handle) => {
            let id = state.next_term_id();
            state.terms.lock().expect("poisoned").insert(id, handle);
            Ok(TermOpen::Opened { id })
        }
        Err(e) if e.needs_user() => Ok(TermOpen::NeedsUser { error: e }),
        Err(e) => Err(CommandError::msg(e.to_string())),
    }
}

fn with_term(state: &AppState, id: TermId, f: impl FnOnce(&term::TermHandle)) -> CmdResult<()> {
    let terms = state.terms.lock().expect("poisoned");
    let h = terms.get(&id).ok_or_else(|| CommandError::msg("terminal not found"))?;
    f(h);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn term_write(state: State<'_, AppState>, id: TermId, data: String) -> CmdResult<()> {
    with_term(&state, id, |h| h.write(data.into_bytes()))
}

#[tauri::command]
#[specta::specta]
pub fn term_resize(state: State<'_, AppState>, id: TermId, cols: u16, rows: u16) -> CmdResult<()> {
    with_term(&state, id, |h| h.resize(cols, rows))
}

/// The program in front of a local terminal (key hints); `None` for SSH,
/// whose shell hook reports it instead.
#[tauri::command]
#[specta::specta]
pub async fn term_foreground(state: State<'_, AppState>, id: TermId) -> CmdResult<Option<String>> {
    let handle = state.terms.lock().expect("poisoned").get(&id).cloned();
    Ok(match handle {
        Some(h) => h.foreground().await,
        None => None,
    })
}

#[tauri::command]
#[specta::specta]
pub fn term_reconnect(state: State<'_, AppState>, id: TermId) -> CmdResult<()> {
    with_term(&state, id, |h| h.reconnect())
}

#[tauri::command]
#[specta::specta]
pub fn term_close(state: State<'_, AppState>, id: TermId) -> CmdResult<()> {
    if let Some(h) = state.terms.lock().expect("poisoned").remove(&id) {
        h.close();
    }
    Ok(())
}

/// Records a host key the user accepted. `replace` is required for a changed
/// key and is only sent after the user typed the host name to confirm.
#[tauri::command]
#[specta::specta]
pub fn ssh_trust_host_key(
    state: State<'_, AppState>,
    host: String,
    port: u16,
    openssh: String,
    replace: bool,
) -> CmdResult<()> {
    state.known_hosts.trust_openssh(&host, port, &openssh, replace)?;
    tracing::info!(%host, port, replace, "host key trusted by user");
    Ok(())
}

#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PromptKind {
    Password,
    Passphrase,
}

/// Supplies a password/passphrase for a hop (`target` label from the
/// prompt). Kept in memory for this run; written to the OS keychain only when
/// `remember` is set and the hop is a stored host.
#[tauri::command]
#[specta::specta]
pub fn ssh_provide_secret(
    state: State<'_, AppState>,
    target: String,
    kind: PromptKind,
    secret: String,
    remember: bool,
) -> CmdResult<()> {
    let kind = match kind {
        PromptKind::Password => SecretKind::HostPassword,
        PromptKind::Passphrase => SecretKind::KeyPassphrase,
    };
    let secret = Arc::new(Secret::new(secret));
    state.secret_cache.lock().expect("poisoned").insert((kind, target.clone()), secret.clone());
    if remember {
        let host = state.label_hosts.lock().expect("poisoned").get(&target).copied();
        if let Some(id) = host {
            state.keychain.set(&SecretRef::new(kind, id.to_string()), &secret)?;
            state.store().set_remember_secret(id, true)?;
        }
    }
    Ok(())
}
