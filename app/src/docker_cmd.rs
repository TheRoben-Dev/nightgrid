//! Docker commands: one engine per host, reached over the host's pooled SSH
//! connection (see `nightgrid_core::docker`).

use std::sync::Arc;

use nightgrid_core::docker::compose::DeployInput;
use nightgrid_core::docker::compose::{ComposeOp, ImageChange, Stack};
use nightgrid_core::docker::proxy::ProxyReport;
use nightgrid_core::docker::registry::{self, ImageUpdate, RemoteImage};
use nightgrid_core::docker::snapshot::{Snapshot, SnapshotId};
use nightgrid_core::docker::{
    valid_container_ref, Action, ContainerDetail, ContainerStats, ContainerView, DockerError, DockerHost, EngineInfo,
    LogLine, LogQuery,
};
use nightgrid_core::files::path::shell_quote;
use nightgrid_core::files::text::save_text;
use nightgrid_core::files::Fs;
use nightgrid_core::hosts::HostId;
use nightgrid_core::ssh::client::ConnectError;
use nightgrid_core::stack_dirs::{valid_stack_dir, StackDir, StackDirId};
use nightgrid_core::timeline::{EventKind, Level};
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
    // Names for the timeline (ids mean nothing there); looked up first since
    // a removed container is gone afterwards.
    let known: std::collections::HashMap<String, String> =
        d.containers().await.map(|l| l.into_iter().map(|c| (c.id, c.name)).collect()).unwrap_or_default();
    let runs = ids.into_iter().map(|id| {
        let d = d.clone();
        async move {
            let error = d.act(&id, action).await.err().map(|e| e.to_string());
            ActionResult { id, error }
        }
    });
    let results = futures_util::future::join_all(runs).await;
    let failed = results.iter().filter(|r| r.error.is_some()).count();
    let names: Vec<&str> = results
        .iter()
        .map(|r| {
            known
                .get(&r.id)
                .or_else(|| known.iter().find(|(k, _)| k.starts_with(&r.id)).map(|(_, v)| v))
                .unwrap_or(&r.id)
                .as_str()
        })
        .take(6)
        .collect();
    let mut title = format!("{} {}", cap(action.verb()), names.join(", "));
    if results.len() > names.len() {
        title.push_str(&format!(" and {} more", results.len() - names.len()));
    }
    let (level, detail) = if failed == 0 {
        (Level::Info, "from Nightgrid".to_owned())
    } else {
        (Level::Warn, format!("from Nightgrid, {failed} failed"))
    };
    crate::timeline_cmd::record(&app, host_id, EventKind::Container, level, &title, &detail);
    Ok(results)
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn compose_word(op: ComposeOp) -> &'static str {
    match op {
        ComposeOp::Up => "Started",
        ComposeOp::Pull => "Pulled images for",
        ComposeOp::Restart => "Restarted",
        ComposeOp::Down => "Took down",
        ComposeOp::Update => "Updated",
    }
}

/// What a finished compose run puts on the timeline.
fn stack_outcome(code: Option<i32>, error: Option<&str>) -> (Level, String) {
    match (code, error) {
        (_, Some(e)) => (Level::Warn, format!("failed: {e}")),
        (Some(0), None) => (Level::Info, "from Nightgrid".into()),
        (Some(c), None) => (Level::Warn, format!("from Nightgrid, exit code {c}")),
        (None, None) => (Level::Warn, "from Nightgrid, didn't finish".into()),
    }
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
        if op == ComposeOp::Update {
            match take_snapshot(&app2, host_id, &d, &stack, "before update").await {
                Ok(s) => sink(vec![format!(
                    "Snapshot saved ({} services, {} files) — roll back from Snapshots.",
                    s.services.len(),
                    s.files.len()
                )]),
                Err(e) => sink(vec![format!("Couldn't save a snapshot first: {}", e.message)]),
            }
        }
        let done = match d.compose(&stack, op, rx, &mut sink).await {
            Ok((code, changes)) => ComposeEvent::Done { code: Some(code), changes, error: None },
            Err(e) => ComposeEvent::Done { code: None, changes: None, error: Some(e.to_string()) },
        };
        if let ComposeEvent::Done { code, changes, error } = &done {
            let (level, mut detail) = stack_outcome(*code, error.as_deref());
            if let Some(c) = changes.as_ref().filter(|c| !c.is_empty()) {
                let names: Vec<&str> = c.iter().map(|x| x.service.as_str()).collect();
                detail.push_str(&format!("; new images: {}", names.join(", ")));
            }
            let title = format!("{} stack {}", compose_word(op), stack.name);
            crate::timeline_cmd::record(&app2, host_id, EventKind::Stack, level, &title, &detail);
        }
        let _ = on_event.send(done);
        app2.state::<AppState>().compose_runs.lock().expect("poisoned").remove(&run_id);
    });
    Ok(run_id)
}

async fn take_snapshot(
    app: &AppHandle,
    host_id: HostId,
    d: &DockerHost,
    stack: &Stack,
    reason: &str,
) -> DockerResult<Snapshot> {
    let services = d.capture(&stack.name).await?;
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_secs() as i64).unwrap_or(0);
    let (files, _skipped) = d.save_files(stack, stamp).await?;
    let state = app.state::<AppState>();
    let snap = state
        .store()
        .save_snapshot(host_id, &stack.name, reason, &services, &files)
        .map_err(|e| DockerCmdError::msg(e.to_string()))?;
    Ok(snap)
}

#[tauri::command]
#[specta::specta]
pub fn docker_snapshots(state: State<'_, AppState>, host_id: HostId, project: String) -> CmdResult<Vec<Snapshot>> {
    Ok(state.store().list_snapshots(host_id, &project)?)
}

/// Saves a snapshot of the stack now: images in the local database, compose
/// files and `.env` copied on the host.
#[tauri::command]
#[specta::specta]
pub async fn docker_snapshot_take(app: AppHandle, host_id: HostId, stack: Stack) -> DockerResult<Snapshot> {
    nightgrid_core::docker::compose::check_stack(&stack)?;
    let d = docker_for(&app, host_id).await?;
    take_snapshot(&app, host_id, &d, &stack, "manual").await
}

/// Forgets a snapshot (the copies on the host age out with newer snapshots).
#[tauri::command]
#[specta::specta]
pub fn docker_snapshot_delete(state: State<'_, AppState>, id: SnapshotId) -> CmdResult<()> {
    Ok(state.store().delete_snapshot(id)?)
}

/// Rolls a stack back to a snapshot, output on the same channel as compose
/// runs. Confirmed in the UI with the stack, host, services and files named
/// (rule 5); a "before rollback" snapshot is taken first.
#[tauri::command]
#[specta::specta]
pub async fn docker_stack_rollback(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    stack: Stack,
    snapshot: SnapshotId,
    on_event: Channel<ComposeEvent>,
) -> DockerResult<ComposeRunId> {
    nightgrid_core::docker::compose::check_stack(&stack)?;
    let snap = state.store().get_snapshot(snapshot).map_err(|e| DockerCmdError::msg(e.to_string()))?;
    if snap.host_id != host_id || snap.project != stack.name {
        return Err(DockerCmdError::msg("that snapshot belongs to another stack"));
    }
    let d = docker_for(&app, host_id).await?;
    tracing::info!(host = %host_id, stack = %stack.name, snapshot = snap.id, "stack rollback");
    let run_id = state.next_term_id();
    let (tx, rx) = tokio::sync::watch::channel(false);
    state.compose_runs.lock().expect("poisoned").insert(run_id, tx);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let mut sink = move |lines: Vec<String>| {
            let _ = ch.send(ComposeEvent::Lines { lines });
        };
        let done = match take_snapshot(&app2, host_id, &d, &stack, "before rollback").await {
            Err(e) => ComposeEvent::Done {
                code: None,
                changes: None,
                error: Some(format!("couldn't save the current state first, nothing was changed: {}", e.message)),
            },
            Ok(_) => {
                sink(vec!["Saved the current state as a snapshot first.".into()]);
                let done = match d.rollback(&stack, &snap, rx, &mut sink).await {
                    Ok(code) => ComposeEvent::Done { code: Some(code), changes: None, error: None },
                    Err(e) => ComposeEvent::Done { code: None, changes: None, error: Some(e.to_string()) },
                };
                if let ComposeEvent::Done { code, error, .. } = &done {
                    let (level, detail) = stack_outcome(*code, error.as_deref());
                    let title = format!("Rolled back stack {} to snapshot #{}", stack.name, snap.id);
                    crate::timeline_cmd::record(&app2, host_id, EventKind::Stack, level, &title, &detail);
                }
                done
            }
        };
        let _ = on_event.send(done);
        app2.state::<AppState>().compose_runs.lock().expect("poisoned").remove(&run_id);
    });
    Ok(run_id)
}

/// Asks each image's registry whether its tag moved, with version and
/// release-notes hints. Only on request (rule 7: registries).
#[tauri::command]
#[specta::specta]
pub async fn docker_stack_check_updates(
    app: AppHandle,
    host_id: HostId,
    stack: Stack,
) -> DockerResult<Vec<ImageUpdate>> {
    nightgrid_core::docker::compose::check_stack(&stack)?;
    let d = docker_for(&app, host_id).await?;
    let locals = d.local_images(&stack.name).await?;
    let checks = locals.iter().map(registry::check);
    Ok(futures_util::future::join_all(checks).await)
}

/// Reverse proxy routes (Caddy, Traefik, Nginx Proxy Manager) on the host,
/// mapped to containers, with certificate expiry. Read-only.
#[tauri::command]
#[specta::specta]
pub async fn docker_proxy_routes(app: AppHandle, host_id: HostId) -> DockerResult<ProxyReport> {
    let d = docker_for(&app, host_id).await?;
    Ok(d.proxy_routes().await?)
}

/// An image's declared ports, volumes and env names from its registry, for
/// the deploy wizard (rule 7: registries). Nothing is pulled.
#[tauri::command]
#[specta::specta]
pub async fn docker_image_lookup(app: AppHandle, host_id: HostId, image: String) -> DockerResult<RemoteImage> {
    let d = docker_for(&app, host_id).await?;
    let (os, arch) = d.platform().await?;
    registry::inspect(image.trim(), &os, &arch).await.map_err(DockerCmdError::msg)
}

/// Deploy wizard: creates `<parent>/<name>/`, writes compose.yaml (and .env,
/// mode 600) and checks it with `docker compose config -q`. Existing files
/// are only replaced with `overwrite`, which the UI confirms naming them
/// (rule 5). Starting the stack is the normal "Up" afterwards.
#[tauri::command]
#[specta::specta]
pub async fn docker_stack_deploy(app: AppHandle, host_id: HostId, input: DeployInput) -> DockerResult<Stack> {
    let stack = input.stack()?;
    let d = docker_for(&app, host_id).await?;
    let conn = connect_host(&app, host_id).await?;
    let fs = Fs::Remote(
        app.state::<AppState>().sftp.get(host_id, conn).await.map_err(|e| DockerCmdError::msg(e.to_string()))?,
    );
    let compose_path = format!("{}/compose.yaml", stack.dir);
    let env_path = format!("{}/.env", stack.dir);
    if !input.overwrite {
        let mut existing = Vec::new();
        for p in [compose_path.as_str(), env_path.as_str()] {
            if (p == compose_path || input.env.is_some()) && fs.stat(p).await.ok().flatten().is_some() {
                existing.push(p.to_owned());
            }
        }
        for f in nightgrid_core::docker::compose::COMPOSE_FILES {
            let p = format!("{}/{f}", stack.dir);
            if p != compose_path && fs.stat(&p).await.ok().flatten().is_some() {
                existing.push(p);
            }
        }
        if !existing.is_empty() {
            return Err(DockerCmdError::msg(format!("already exists: {}", existing.join(", "))));
        }
    }
    let (code, _, err) = d
        .conn
        .exec(&format!("mkdir -p -- {}", shell_quote(&stack.dir)))
        .await
        .map_err(|e| DockerCmdError::msg(e.to_string()))?;
    if code != 0 {
        return Err(DockerCmdError::msg(format!(
            "couldn't create {}: {}",
            stack.dir,
            String::from_utf8_lossy(&err).trim()
        )));
    }
    if input.overwrite {
        tracing::info!(host = %host_id, dir = %stack.dir, "deploy overwrites stack files");
    }
    save_text(&fs, &compose_path, &input.compose, None).await.map_err(|e| DockerCmdError::msg(e.to_string()))?;
    if let Some(env) = &input.env {
        // Create it private before any secret goes in.
        let _ = d.conn.exec(&format!("umask 077; : >> {p}; chmod 600 {p}", p = shell_quote(&env_path))).await;
        save_text(&fs, &env_path, env, None).await.map_err(|e| DockerCmdError::msg(e.to_string()))?;
    }
    if let Err(msg) = d.validate_stack(&stack).await? {
        return Err(DockerCmdError::msg(format!("compose.yaml was saved, but docker compose rejects it:\n{msg}")));
    }
    let title = format!("Deployed stack files for {}", stack.name);
    crate::timeline_cmd::record(&app, host_id, EventKind::Stack, Level::Info, &title, &stack.dir);
    Ok(stack)
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
