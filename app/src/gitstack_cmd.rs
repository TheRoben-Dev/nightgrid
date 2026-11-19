//! Stacks from git (M10w): track a compose stack's repository on the host,
//! check for new commits, review the diff and deploy a chosen commit. Git
//! runs on the host with its own credentials; Nightgrid stores no token.

use std::time::Duration;

use nightgrid_core::docker::compose::Stack;
use nightgrid_core::docker::gitstack::{self, GitStack, GitStackId, GitStackInput, GitStatus};
use nightgrid_core::hosts::HostId;
use nightgrid_core::timeline::{EventKind, Level};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::CmdResult;
use crate::docker_cmd::{docker_for, take_snapshot, DockerCmdError, DockerResult};
use crate::state::AppState;
use crate::term_cmd::connect_posix;

/// Clones and fetches can be slow; nothing should hang forever.
const GIT_TIMEOUT: Duration = Duration::from_secs(300);

fn core_err(e: nightgrid_core::Error) -> DockerCmdError {
    DockerCmdError::msg(e.to_string())
}

async fn run(app: &AppHandle, host_id: HostId, script: &str) -> DockerResult<String> {
    let conn = connect_posix(app, host_id, "docker.compose").await?;
    let (_, out, err) = tokio::time::timeout(GIT_TIMEOUT, conn.exec(script))
        .await
        .map_err(|_| DockerCmdError::msg("git took longer than 5 minutes; try again, or run it on the host"))?
        .map_err(|e| DockerCmdError::msg(e.to_string()))?;
    let mut text = String::from_utf8_lossy(&out).into_owned();
    // The scripts capture git's own errors; stderr only matters when the
    // script never got going (no markers at all).
    if !text.lines().any(|l| l.starts_with("@@")) {
        for line in String::from_utf8_lossy(&err).lines() {
            text.push_str("\n@@err ");
            text.push_str(line);
        }
    }
    Ok(text)
}

fn git_of(app: &AppHandle, id: GitStackId) -> DockerResult<GitStack> {
    app.state::<AppState>().store().git_stack(id).map_err(core_err)
}

#[tauri::command]
#[specta::specta]
pub fn git_stacks_list(state: State<'_, AppState>, host_id: HostId) -> CmdResult<Vec<GitStack>> {
    Ok(state.store().git_stacks(host_id)?)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GitStackAdded {
    pub git: GitStack,
    /// The folder already held a checkout of this repository.
    pub adopted: bool,
    /// `None` when the folder has no compose file yet.
    pub stack: Option<Stack>,
}

/// Clones the repository into the folder on the host (or adopts a checkout
/// of the same URL there) and starts tracking it.
#[tauri::command]
#[specta::specta]
pub async fn git_stack_add(app: AppHandle, host_id: HostId, input: GitStackInput) -> DockerResult<GitStackAdded> {
    let i = gitstack::normalize(&input).map_err(core_err)?;
    let out = run(&app, host_id, &gitstack::clone_script(&i)).await?;
    let m = gitstack::parse_clone(&out).map_err(DockerCmdError::msg)?;
    let git = app.state::<AppState>().store().add_git_stack(host_id, &i).map_err(core_err)?;
    let stack = m
        .stack_dir
        .as_deref()
        .filter(|_| m.compose_file.is_some())
        .map(|d| gitstack::stack_for(d, m.compose_file.as_deref()));
    tracing::info!(host = %host_id, dir = %i.dir, adopted = m.adopted, "git stack added");
    crate::timeline_cmd::record(
        &app,
        host_id,
        EventKind::Stack,
        Level::Info,
        &format!("{} {}", if m.adopted { "Tracking git checkout" } else { "Cloned" }, i.url),
        &git.stack_dir(),
    );
    Ok(GitStackAdded { git, adopted: m.adopted, stack })
}

/// Fetches the tracked branch and says what's new.
#[tauri::command]
#[specta::specta]
pub async fn git_stack_check(app: AppHandle, id: GitStackId) -> DockerResult<GitStatus> {
    let g = git_of(&app, id)?;
    let out = run(&app, g.host_id, &gitstack::status_script(&g)).await?;
    gitstack::parse_status(&out).map_err(DockerCmdError::msg)
}

/// The diff from the checkout to `target`, limited to the stack's folder.
#[tauri::command]
#[specta::specta]
pub async fn git_stack_diff(app: AppHandle, id: GitStackId, target: String) -> DockerResult<String> {
    if !gitstack::valid_commit(&target) {
        return Err(DockerCmdError::msg("not a commit id"));
    }
    let g = git_of(&app, id)?;
    let conn = connect_posix(&app, g.host_id, "docker.compose").await?;
    let (_, out, _) = tokio::time::timeout(GIT_TIMEOUT, conn.exec(&gitstack::diff_script(&g, &target)))
        .await
        .map_err(|_| DockerCmdError::msg("git diff took too long"))?
        .map_err(|e| DockerCmdError::msg(e.to_string()))?;
    let text = String::from_utf8_lossy(&out).into_owned();
    if text.starts_with("@@nogit") {
        return Err(DockerCmdError::msg("git isn't installed on the host"));
    }
    if text.starts_with("@@unknown") {
        return Err(DockerCmdError::msg("that commit isn't on the host; check for changes again"));
    }
    Ok(text)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GitDeployed {
    pub git: GitStack,
    pub from: Option<String>,
    pub to: String,
    /// The stack to bring up, when the folder has a compose file.
    pub stack: Option<Stack>,
    /// A stack snapshot was saved first.
    pub snapshot: bool,
    pub snapshot_error: Option<String>,
}

/// Moves the checkout to `target` (a reviewed commit, forward or back) after a
/// stack snapshot. The UI confirms naming the stack, host and commit (rule 5)
/// and then runs the normal "Up".
#[tauri::command]
#[specta::specta]
pub async fn git_stack_deploy(app: AppHandle, id: GitStackId, target: String) -> DockerResult<GitDeployed> {
    if !gitstack::valid_commit(&target) {
        return Err(DockerCmdError::msg("not a commit id"));
    }
    let g = git_of(&app, id)?;
    let before = gitstack::parse_locate(&run(&app, g.host_id, &gitstack::locate_script(&g)).await?);
    let (snapshot, snapshot_error) = match &before {
        Some(stack) => match docker_for(&app, g.host_id).await {
            Ok(d) => match take_snapshot(&app, g.host_id, &d, stack, "before git deploy").await {
                Ok(_) => (true, None),
                Err(e) => (false, Some(e.message)),
            },
            Err(e) => (false, Some(e.message)),
        },
        None => (false, Some("no compose file in the folder yet".to_owned())),
    };
    let out = run(&app, g.host_id, &gitstack::deploy_script(&g, &target)).await?;
    let m = gitstack::parse_deploy(&out).map_err(DockerCmdError::msg)?;
    let to = m.to.clone().unwrap_or_default();
    let git = app.state::<AppState>().store().mark_git_deployed(id, &to).map_err(core_err)?;
    let stack = m
        .stack_dir
        .as_deref()
        .filter(|_| m.compose_file.is_some())
        .map(|d| gitstack::stack_for(d, m.compose_file.as_deref()));
    let short = |c: &str| c.chars().take(8).collect::<String>();
    let name = stack.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| git.stack_dir());
    tracing::info!(host = %g.host_id, dir = %g.dir, to = %short(&to), "git stack deployed");
    crate::timeline_cmd::record(
        &app,
        g.host_id,
        EventKind::Stack,
        Level::Info,
        &format!("Deployed {} at {}", name, short(&to)),
        &format!("{} {} → {}", g.url, m.from.as_deref().map(short).unwrap_or_default(), short(&to)),
    );
    Ok(GitDeployed { git, from: m.from, to, stack, snapshot, snapshot_error })
}

/// Stops tracking; the checkout and the stack stay on the host.
#[tauri::command]
#[specta::specta]
pub fn git_stack_remove(state: State<'_, AppState>, id: GitStackId) -> CmdResult<()> {
    Ok(state.store().delete_git_stack(id)?)
}
