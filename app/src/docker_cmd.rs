//! Docker commands: one engine per host, reached over the host's pooled SSH
//! connection (see `nightgrid_core::docker`).

use std::sync::Arc;

use nightgrid_core::docker::{
    valid_container_ref, Action, ContainerDetail, ContainerStats, ContainerView, DockerError, DockerHost, EngineInfo,
    LogLine, LogQuery,
};
use nightgrid_core::hosts::HostId;
use nightgrid_core::ssh::client::ConnectError;
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
