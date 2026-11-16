//! The `.env` vault (M10w): variables per compose stack, encrypted with a
//! keychain-held key, written to the host's `.env` only on deploy.

use nightgrid_core::docker::compose::{check_stack, Stack};
use nightgrid_core::envvault::{self, EnvDiff, EnvVar, EnvVarId, EnvVarInput, Zeroizing};
use nightgrid_core::files::path::shell_quote;
use nightgrid_core::files::text::save_text;
use nightgrid_core::files::Fs;
use nightgrid_core::hosts::HostId;
use nightgrid_core::timeline::{EventKind, Level};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::CmdResult;
use crate::docker_cmd::{docker_for, take_snapshot, DockerCmdError, DockerResult};
use crate::state::AppState;
use crate::term_cmd::connect_host;

fn env_path(stack: &Stack) -> String {
    format!("{}/.env", stack.dir.trim_end_matches('/'))
}

fn core_err(e: nightgrid_core::Error) -> DockerCmdError {
    DockerCmdError::msg(e.to_string())
}

/// The host's `.env` for `stack`: (exists, variables).
async fn read_host_env(
    app: &AppHandle,
    host_id: HostId,
    stack: &Stack,
) -> DockerResult<(bool, Vec<(String, Zeroizing<String>)>)> {
    check_stack(stack)?;
    let d = docker_for(app, host_id).await?;
    let p = shell_quote(&env_path(stack));
    let (code, out, err) = d
        .conn
        .exec(&format!("if [ -f {p} ]; then cat -- {p}; else echo '@@none' >&2; fi"))
        .await
        .map_err(|e| DockerCmdError::msg(e.to_string()))?;
    let err = String::from_utf8_lossy(&err);
    if err.contains("@@none") {
        return Ok((false, Vec::new()));
    }
    if code != 0 {
        return Err(DockerCmdError::msg(format!("couldn't read {}: {}", env_path(stack), err.trim())));
    }
    let text = Zeroizing::new(String::from_utf8_lossy(&out).into_owned());
    Ok((true, envvault::parse_env(&text)))
}

#[tauri::command]
#[specta::specta]
pub fn env_vars_list(state: State<'_, AppState>, host_id: HostId, stack: String) -> CmdResult<Vec<EnvVar>> {
    Ok(state.store().env_vars(&state.keychain, host_id, &stack)?)
}

/// Stacks that have variables in the vault: (host id, stack, count).
#[tauri::command]
#[specta::specta]
pub fn env_stacks(state: State<'_, AppState>) -> CmdResult<Vec<(HostId, String, u32)>> {
    Ok(state.store().env_stacks()?)
}

/// Adds or changes a variable; `old_name` renames. `value: None` keeps it.
#[tauri::command]
#[specta::specta]
pub fn env_var_set(
    state: State<'_, AppState>,
    host_id: HostId,
    stack: String,
    old_name: Option<String>,
    input: EnvVarInput,
) -> CmdResult<EnvVar> {
    Ok(state.store().set_env_var(&state.keychain, host_id, &stack, old_name.as_deref(), &input)?)
}

#[tauri::command]
#[specta::specta]
pub fn env_var_delete(state: State<'_, AppState>, id: EnvVarId) -> CmdResult<()> {
    Ok(state.store().delete_env_var(id)?)
}

/// One value, for "click to reveal" (rule 6).
#[tauri::command]
#[specta::specta]
pub fn env_var_reveal(state: State<'_, AppState>, id: EnvVarId) -> CmdResult<String> {
    Ok(state.store().env_value(&state.keychain, id)?.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn env_vars_order(state: State<'_, AppState>, host_id: HostId, stack: String, ids: Vec<EnvVarId>) -> CmdResult<()> {
    Ok(state.store().order_env_vars(host_id, &stack, &ids)?)
}

/// Copies the host's current `.env` into the vault (new names added,
/// existing ones updated). Returns how many variables were read.
#[tauri::command]
#[specta::specta]
pub async fn env_import_host(app: AppHandle, host_id: HostId, stack: Stack) -> DockerResult<u32> {
    let (exists, vars) = read_host_env(&app, host_id, &stack).await?;
    if !exists {
        return Err(DockerCmdError::msg(format!("there is no {} on the host", env_path(&stack))));
    }
    let state = app.state::<AppState>();
    let n = state.store().import_env(&state.keychain, host_id, &stack.name, &vars).map_err(core_err)?;
    tracing::info!(host = %host_id, stack = %stack.name, vars = n, "env imported into vault");
    Ok(n)
}

/// What deploying would change in the host's `.env`, by variable name.
#[tauri::command]
#[specta::specta]
pub async fn env_deploy_preview(app: AppHandle, host_id: HostId, stack: Stack) -> DockerResult<EnvDiff> {
    let (exists, current) = read_host_env(&app, host_id, &stack).await?;
    let state = app.state::<AppState>();
    let new = state.store().env_values(&state.keychain, host_id, &stack.name).map_err(core_err)?;
    Ok(envvault::diff(&current, &new, exists))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EnvDeployed {
    pub path: String,
    /// A stack snapshot was saved first (roll back from Snapshots).
    pub snapshot: bool,
    pub snapshot_error: Option<String>,
}

/// Writes the vault's variables to the host's `.env` (mode 600) after a
/// stack snapshot. Starting the stack again is the normal "Up" afterwards.
/// The UI shows the preview first and names the stack and host (rule 5).
#[tauri::command]
#[specta::specta]
pub async fn env_deploy(app: AppHandle, host_id: HostId, stack: Stack) -> DockerResult<EnvDeployed> {
    check_stack(&stack)?;
    let text = {
        let state = app.state::<AppState>();
        let vars = state.store().env_values(&state.keychain, host_id, &stack.name).map_err(core_err)?;
        if vars.is_empty() {
            return Err(DockerCmdError::msg("the vault has no variables for this stack"));
        }
        envvault::render(&vars)
    };
    let d = docker_for(&app, host_id).await?;
    let (snapshot, snapshot_error) = match take_snapshot(&app, host_id, &d, &stack, "before .env deploy").await {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.message)),
    };
    let conn = connect_host(&app, host_id).await?;
    let fs = Fs::Remote(
        app.state::<AppState>().sftp.get(host_id, conn).await.map_err(|e| DockerCmdError::msg(e.to_string()))?,
    );
    let path = env_path(&stack);
    // Create it private before any secret goes in.
    let (code, _, err) = d
        .conn
        .exec(&format!("umask 077; : >> {p} && chmod 600 {p}", p = shell_quote(&path)))
        .await
        .map_err(|e| DockerCmdError::msg(e.to_string()))?;
    if code != 0 {
        return Err(DockerCmdError::msg(format!("couldn't prepare {path}: {}", String::from_utf8_lossy(&err).trim())));
    }
    save_text(&fs, &path, &text, None).await.map_err(|e| DockerCmdError::msg(e.to_string()))?;
    tracing::info!(host = %host_id, stack = %stack.name, "env deployed from vault");
    crate::timeline_cmd::record(
        &app,
        host_id,
        EventKind::Stack,
        Level::Info,
        &format!("Wrote .env for {}", stack.name),
        &path,
    );
    Ok(EnvDeployed { path, snapshot, snapshot_error })
}
