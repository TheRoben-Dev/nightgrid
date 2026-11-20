//! SSH key manager (M10k): the user's keys, generating new ones, and each
//! host's `authorized_keys`. Rotation is driven by the UI as
//! push → verify → switch → remove, one host at a time, so a host is never
//! left without a working key.

use std::sync::Arc;
use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::secrets::Secret;
use nightgrid_core::ssh::client::{self, Auth};
use nightgrid_core::ssh::keys::{self, AuthorizedKey, SshKey};
use nightgrid_core::win::keys as win_keys;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

const EXEC_TIMEOUT: Duration = Duration::from_secs(15);

/// The user's keys and where they were looked for.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyList {
    /// The `.ssh` directory that was scanned (and where new keys go).
    pub dir: String,
    pub keys: Vec<SshKey>,
    /// Why agent keys are missing, when the agent couldn't be reached.
    pub agent_error: Option<String>,
}

/// What pushing a key to a host did.
#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PushOutcome {
    Added,
    AlreadyThere,
}

fn ssh_dir(state: &AppState) -> CmdResult<std::path::PathBuf> {
    state.home.as_ref().map(|h| h.join(".ssh")).ok_or_else(|| CommandError::msg("no home directory"))
}

async fn list(state: &AppState) -> CmdResult<KeyList> {
    let dir = ssh_dir(state)?;
    let files = keys::scan_dir(&dir);
    let agent_path = state.connect_options().agent_path;
    let (agent, agent_error) = match keys::agent_keys(agent_path.as_deref()).await {
        Ok(a) => (a, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    Ok(KeyList { dir: dir.display().to_string(), keys: keys::merge_agent(files, &agent), agent_error })
}

/// The public key with this fingerprint, from `~/.ssh` or the agent.
async fn find(state: &AppState, fingerprint: &str) -> CmdResult<String> {
    list(state)
        .await?
        .keys
        .into_iter()
        .find(|k| k.fingerprint == fingerprint)
        .map(|k| k.public)
        .ok_or_else(|| CommandError::msg("that key isn't in ~/.ssh or the agent any more"))
}

/// Runs the POSIX `command`, or on a Windows host the PowerShell `windows`
/// script, and returns stdout.
async fn exec_either(app: &AppHandle, host_id: HostId, command: &str, windows: &str) -> CmdResult<String> {
    let conn = crate::term_cmd::connect_host(app, host_id).await.map_err(|e| CommandError::msg(e.to_string()))?;
    if !conn.is_windows() {
        return exec(app, host_id, command).await;
    }
    let o = tokio::time::timeout(EXEC_TIMEOUT, nightgrid_core::win::run(&conn, windows))
        .await
        .map_err(|_| CommandError::msg("the host took too long to answer"))?
        .map_err(CommandError::msg)?;
    if o.code != 0 {
        return Err(CommandError::msg(format!("exit {}: {}", o.code, o.error_text())));
    }
    Ok(o.stdout)
}

/// The host's `authorized_keys` text (the file sshd reads, on Windows too).
pub(crate) async fn read_authorized(app: &AppHandle, host_id: HostId) -> CmdResult<String> {
    exec_either(app, host_id, keys::READ_AUTHORIZED, &win_keys::read_script()).await
}

pub(crate) async fn exec(app: &AppHandle, host_id: HostId, command: &str) -> CmdResult<String> {
    let conn = crate::term_cmd::connect_host(app, host_id).await.map_err(|e| CommandError::msg(e.to_string()))?;
    let (code, out, err) = tokio::time::timeout(EXEC_TIMEOUT, conn.exec(command))
        .await
        .map_err(|_| CommandError::msg("the host took too long to answer"))?
        .map_err(|e| CommandError::msg(e.to_string()))?;
    if code != 0 {
        let err = String::from_utf8_lossy(&err);
        return Err(CommandError::msg(format!("exit {code}: {}", err.trim())));
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[tauri::command]
#[specta::specta]
pub async fn keys_list(state: State<'_, AppState>) -> CmdResult<KeyList> {
    list(&state).await
}

/// Generates an ed25519 key in `~/.ssh`. The passphrase is used once to
/// encrypt the file and is not stored.
#[tauri::command]
#[specta::specta]
pub fn keys_generate(
    state: State<'_, AppState>,
    name: String,
    comment: String,
    passphrase: Option<String>,
) -> CmdResult<SshKey> {
    // Wrapped so the passphrase is wiped when this returns.
    let pass = passphrase.map(Secret::new);
    Ok(keys::generate(&ssh_dir(&state)?, name.trim(), &comment, pass.as_ref().map(Secret::expose))?)
}

/// The keys in a host's `~/.ssh/authorized_keys`.
#[tauri::command]
#[specta::specta]
pub async fn keys_authorized(app: AppHandle, host_id: HostId) -> CmdResult<Vec<AuthorizedKey>> {
    let text = exec_either(&app, host_id, keys::READ_AUTHORIZED, &win_keys::read_script()).await?;
    Ok(keys::parse_authorized(&text))
}

/// Adds a key to a host's `authorized_keys` (no-op when it's there).
#[tauri::command]
#[specta::specta]
pub async fn keys_push(app: AppHandle, host_id: HostId, fingerprint: String) -> CmdResult<PushOutcome> {
    let key = find(&app.state::<AppState>(), &fingerprint).await?;
    let out = exec_either(&app, host_id, &keys::add_command(&key)?, &win_keys::add_script(&key)?).await?;
    let added = out.contains("added");
    if added {
        let title = format!("Authorized key {fingerprint}");
        crate::journal_cmd::record(
            &app,
            crate::journal_cmd::Change::new(host_id, nightgrid_core::journal::ChangeArea::Keys, &title),
        );
    }
    Ok(if added { PushOutcome::Added } else { PushOutcome::AlreadyThere })
}

/// Removes a key from a host's `authorized_keys`. The UI confirms first,
/// naming the host and the key. Returns how many lines were removed.
#[tauri::command]
#[specta::specta]
pub async fn keys_remove(app: AppHandle, host_id: HostId, fingerprint: String) -> CmdResult<u32> {
    // The key may be gone from this machine already; take it from the host's list.
    let state = app.state::<AppState>();
    let key = match find(&state, &fingerprint).await {
        Ok(k) => k,
        Err(_) => {
            let text = exec_either(&app, host_id, keys::READ_AUTHORIZED, &win_keys::read_script()).await?;
            keys::find_authorized(&text, &fingerprint).ok_or_else(|| CommandError::msg("that key isn't on the host"))?
        }
    };
    let out = exec_either(&app, host_id, &keys::remove_command(&key)?, &win_keys::remove_script(&key)?).await?;
    let n = keys::parse_removed(&out).ok_or_else(|| CommandError::msg(format!("unexpected answer: {}", out.trim())))?;
    if n > 0 {
        let title = format!("Removed key {fingerprint}");
        crate::journal_cmd::record(
            &app,
            crate::journal_cmd::Change::new(host_id, nightgrid_core::journal::ChangeArea::Keys, &title),
        );
    }
    Ok(n)
}

/// Logs in to the host with only this key file (same jump hosts), then
/// disconnects. Proves the key works before the old one is removed.
#[tauri::command]
#[specta::specta]
pub async fn keys_verify(
    state: State<'_, AppState>,
    host_id: HostId,
    file: String,
    passphrase: Option<String>,
) -> CmdResult<()> {
    let (mut hop, jumps) =
        crate::term_cmd::resolve_host(&state, host_id).map_err(|e| CommandError::msg(e.to_string()))?;
    hop.target.auth = Auth::Key {
        path: file.into(),
        passphrase: passphrase.filter(|p| !p.is_empty()).map(|p| Arc::new(Secret::new(p))),
        // Proves the key alone: no password fallback.
        password: None,
    };
    let opts = state.connect_options();
    let jumps: Vec<_> = jumps.into_iter().map(|h| h.target).collect();
    let conn = client::connect(&hop.target, &jumps, &opts).await.map_err(|e| CommandError::msg(e.to_string()))?;
    conn.disconnect().await;
    Ok(())
}
