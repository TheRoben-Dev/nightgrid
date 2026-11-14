//! Docker commands: one engine per host, reached over the host's pooled SSH
//! connection (see `nightgrid_core::docker`).

use std::sync::Arc;

use nightgrid_core::docker::compose::{ComposeOp, ImageChange, Stack};
use nightgrid_core::docker::{
    valid_container_ref, Action, ContainerDetail, ContainerStats, ContainerView, DockerError, DockerHost, EngineInfo,
    LogLine, LogQuery,
};
use nightgrid_core::hosts::HostId;
use nightgrid_core::ssh::client::ConnectError;
use nightgrid_core::stack_dirs::{valid_stack_dir, StackDir, StackDirId};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::term_cmd::connect_host;

/// Docker command error: a message plus the structured cause, so the UI can
/// show a host-key / password prompt or the docker-group hint.
#[derive(Debug, Serialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct DockerCmdError {
    pub message: String,
    pub connect: Option<Box<ConnectError>>,
    pub docker: Option<Box<DockerError>>,
}

impl From<DockerError> for DockerCmdError {
    fn from(e: DockerError) -> Self {
        Self { message: e.to_string(), connect: None, docker: Some(Box::new(e)) }
    }
}

impl From<ConnectError> for DockerCmdError {
    fn from(e: ConnectError) -> Self {
        Self { message: e.to_string(), connect: Some(Box::new(e)), docker: None }
    }
}

impl DockerCmdError {
    fn msg(m: impl Into<String>) -> Self {
        Self { message: m.into(), connect: None, docker: None }
    }
}

type DockerResult<T> = Result<T, DockerCmdError>;

pub(crate) async fn docker_for(app: &AppHandle, host_id: HostId) -> DockerResult<Arc<DockerHost>> {
    let conn = connect_host(app, host_id).await?;
    let state = app.state::<AppState>();
    Ok(state.docker.get(host_id, conn).await?)
}

fn check_ref(id: &str) -> DockerResult<()> {
    if valid_container_ref(id) {
        Ok(())
    } else {
        Err(DockerCmdError::msg(format!("invalid container reference: {id:?}")))
    }
}

#[tauri::command]
#[specta::specta]
pub async fn docker_info(app: AppHandle, host_id: HostId) -> DockerResult<EngineInfo> {
    Ok(docker_for(&app, host_id).await?.info().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn docker_containers(app: AppHandle, host_id: HostId) -> DockerResult<Vec<ContainerView>> {
    Ok(docker_for(&app, host_id).await?.containers().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn docker_stats(app: AppHandle, host_id: HostId, ids: Vec<String>) -> DockerResult<Vec<ContainerStats>> {
    if ids.len() > 500 {
        return Err(DockerCmdError::msg("too many containers"));
    }
    for id in &ids {
        check_ref(id)?;
    }
    Ok(docker_for(&app, host_id).await?.stats(&ids).await)
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub id: String,
    pub error: Option<String>,
}

/// Runs `action` on each container. The UI has already shown a confirmation
/// naming every target for destructive actions (security rule 5).
#[tauri::command]
#[specta::specta]
pub async fn docker_action(
    app: AppHandle,
    host_id: HostId,
    ids: Vec<String>,
    action: Action,
) -> DockerResult<Vec<ActionResult>> {
    if ids.is_empty() || ids.len() > 200 {
        return Err(DockerCmdError::msg("pick between 1 and 200 containers"));
    }
    for id in &ids {
        check_ref(id)?;
    }
    let d = docker_for(&app, host_id).await?;
    if action.is_destructive() {
        tracing::info!(host = %host_id, action = action.verb(), targets = ?ids, "docker action");
    }
    let runs = ids.into_iter().map(|id| {
        let d = d.clone();
        async move {
            let error = d.act(&id, action).await.err().map(|e| e.to_string());
            ActionResult { id, error }
        }
    });
    Ok(futures_util::future::join_all(runs).await)
}

#[tauri::command]
#[specta::specta]
pub async fn docker_inspect(app: AppHandle, host_id: HostId, id: String) -> DockerResult<ContainerDetail> {
    check_ref(&id)?;
    Ok(docker_for(&app, host_id).await?.inspect(&id).await?)
}

/// What the log channel carries.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LogEvent {
    Lines {
        lines: Vec<LogLine>,
    },
    /// The stream ended (container stopped, backlog sent, or an error).
    End {
        error: Option<String>,
    },
}

pub type LogStreamId = u32;

/// Starts streaming a container's logs; stop with `docker_logs_stop`.
#[tauri::command]
#[specta::specta]
pub async fn docker_logs_start(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    id: String,
    query: LogQuery,
    on_event: Channel<LogEvent>,
) -> DockerResult<LogStreamId> {
    check_ref(&id)?;
    let d = docker_for(&app, host_id).await?;
    let stream_id = state.next_term_id();
    let app2 = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = d.logs(&id, query, move |lines| ch.send(LogEvent::Lines { lines }).is_ok()).await;
        let _ = on_event.send(LogEvent::End { error: r.err().map(|e| e.to_string()) });
        app2.state::<AppState>().log_streams.lock().expect("poisoned").remove(&stream_id);
    });
    state.log_streams.lock().expect("poisoned").insert(stream_id, task);
    Ok(stream_id)
}

#[tauri::command]
#[specta::specta]
pub fn docker_logs_stop(state: State<'_, AppState>, stream: LogStreamId) -> CmdResult<()> {
    if let Some(t) = state.log_streams.lock().expect("poisoned").remove(&stream) {
        t.abort();
    }
    Ok(())
}

/// Forgets the cached engine (e.g. after adding the user to the docker group
/// and reconnecting).
#[tauri::command]
#[specta::specta]
pub fn docker_forget(state: State<'_, AppState>, host_id: HostId) -> CmdResult<()> {
    state.docker.drop_host(host_id);
    Ok(())
}

pub(crate) fn exec_command(d: &DockerHost, container: &str) -> CmdResult<String> {
    if !valid_container_ref(container) {
        return Err(CommandError::msg("invalid container reference"));
    }
    Ok(nightgrid_core::docker::exec_shell_command(container, &d.route))
}

#[tauri::command]
#[specta::specta]
pub fn stack_dirs_list(state: State<'_, AppState>, host_id: HostId) -> CmdResult<Vec<StackDir>> {
    Ok(state.store().list_stack_dirs(host_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn stack_dirs_add(state: State<'_, AppState>, host_id: HostId, path: String) -> CmdResult<StackDir> {
    let path = path.trim();
    if !valid_stack_dir(path) {
        return Err(CommandError::msg("use an absolute folder (/opt/stacks) or one under ~ (~/stacks)"));
    }
    Ok(state.store().add_stack_dir(host_id, path)?)
}

#[tauri::command]
#[specta::specta]
pub fn stack_dirs_delete(state: State<'_, AppState>, id: StackDirId) -> CmdResult<()> {
    Ok(state.store().delete_stack_dir(id)?)
}

/// Compose stacks on the host: from container labels plus the host's stack
/// folders.
#[tauri::command]
#[specta::specta]
pub async fn docker_stacks(app: AppHandle, host_id: HostId) -> DockerResult<Vec<Stack>> {
    let dirs: Vec<String> = {
        let state = app.state::<AppState>();
        let list = state.store().list_stack_dirs(host_id).map_err(|e| DockerCmdError::msg(e.to_string()))?;
        list.into_iter().map(|d| d.path).collect()
    };
    Ok(docker_for(&app, host_id).await?.stacks(&dirs).await?)
}

/// What a compose run's channel carries.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ComposeEvent {
    Lines {
        lines: Vec<String>,
    },
    /// The command finished. `code` 0 = success; `changes` only for update.
    Done {
        code: Option<i32>,
        changes: Option<Vec<ImageChange>>,
        error: Option<String>,
    },
}

pub type ComposeRunId = u32;

/// Starts a compose command for `stack`. Down / restart / update were
/// confirmed in the UI with the stack and host named (security rule 5).
#[tauri::command]
#[specta::specta]
pub async fn docker_compose_run(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    stack: Stack,
    op: ComposeOp,
    on_event: Channel<ComposeEvent>,
) -> DockerResult<ComposeRunId> {
    nightgrid_core::docker::compose::check_stack(&stack)?;
    let d = docker_for(&app, host_id).await?;
    if op.is_destructive() {
        tracing::info!(host = %host_id, stack = %stack.name, op = ?op, "compose action");
    }
    let run_id = state.next_term_id();
    let (tx, rx) = tokio::sync::watch::channel(false);
    state.compose_runs.lock().expect("poisoned").insert(run_id, tx);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let mut sink = move |lines: Vec<String>| {
            let _ = ch.send(ComposeEvent::Lines { lines });
        };
        let done = match d.compose(&stack, op, rx, &mut sink).await {
            Ok((code, changes)) => ComposeEvent::Done { code: Some(code), changes, error: None },
            Err(e) => ComposeEvent::Done { code: None, changes: None, error: Some(e.to_string()) },
        };
        let _ = on_event.send(done);
        app2.state::<AppState>().compose_runs.lock().expect("poisoned").remove(&run_id);
    });
    Ok(run_id)
}

/// Sends Ctrl-C to a running compose command.
#[tauri::command]
#[specta::specta]
pub fn docker_compose_cancel(state: State<'_, AppState>, run: ComposeRunId) -> CmdResult<()> {
    if let Some(tx) = state.compose_runs.lock().expect("poisoned").get(&run) {
        let _ = tx.send(true);
    }
    Ok(())
}
