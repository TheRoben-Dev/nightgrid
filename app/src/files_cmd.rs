//! File browser, editor, bookmark and transfer commands. Paths are strings in
//! the side's own syntax (POSIX on hosts, native locally); the UI never joins
//! paths itself.

use std::sync::Arc;

use nightgrid_core::bookmarks::{Bookmark, BookmarkId};
use nightgrid_core::files::text::{self, SaveResult, Stamp, TextFile};
use nightgrid_core::files::transfer::{ConflictPolicy, JobId, JobView, NewJob, Resolver, Side, Transfers};
use nightgrid_core::files::{Entry, Fs, FsError};
use nightgrid_core::hosts::HostId;
use nightgrid_core::ssh::client::ConnectError;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::term_cmd::connect_host;

/// File command error: a message, plus the structured cause so the UI can
/// show the host-key / password prompt or a save conflict.
#[derive(Debug, Serialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct FilesError {
    pub message: String,
    pub connect: Option<Box<ConnectError>>,
    pub fs: Option<Box<FsError>>,
}

impl From<FsError> for FilesError {
    fn from(e: FsError) -> Self {
        Self { message: e.to_string(), connect: None, fs: Some(Box::new(e)) }
    }
}

impl From<ConnectError> for FilesError {
    fn from(e: ConnectError) -> Self {
        Self { message: e.to_string(), connect: Some(Box::new(e)), fs: None }
    }
}

impl From<nightgrid_core::Error> for FilesError {
    fn from(e: nightgrid_core::Error) -> Self {
        Self { message: e.to_string(), connect: None, fs: None }
    }
}

pub(crate) type FilesResult<T> = Result<T, FilesError>;

/// Opens the filesystem for a side, connecting to the host if needed.
pub(crate) async fn fs_for(app: &AppHandle, side: Side) -> FilesResult<Fs> {
    match side {
        Side::Local => Ok(Fs::Local),
        Side::Host { id } => {
            let conn = connect_host(app, id).await?;
            let state = app.state::<AppState>();
            Ok(Fs::Remote(state.sftp.get(id, conn).await?))
        }
    }
}

/// The transfer queue, created on first use with a resolver bound to the app.
fn transfers(app: &AppHandle) -> Transfers {
    let state = app.state::<AppState>();
    state
        .transfers
        .get_or_init(|| {
            let app = app.clone();
            let resolver: Resolver = Arc::new(move |side| {
                let app = app.clone();
                Box::pin(async move {
                    fs_for(&app, side)
                        .await
                        .map_err(|e| e.fs.map(|b| *b).unwrap_or(FsError::Other { message: e.message }))
                })
            });
            Transfers::new(resolver, 3)
        })
        .clone()
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// Canonical absolute path of the directory.
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}

#[tauri::command]
#[specta::specta]
pub async fn fs_home(app: AppHandle, side: Side) -> FilesResult<String> {
    Ok(fs_for(&app, side).await?.home().await?)
}

/// Lists a directory (`~` and relative paths are resolved).
#[tauri::command]
#[specta::specta]
pub async fn fs_list(app: AppHandle, side: Side, path: String) -> FilesResult<Listing> {
    let fs = fs_for(&app, side).await?;
    let path = fs.canonicalize(&path).await?;
    let entries = fs.list(&path).await?;
    Ok(Listing { parent: fs.parent(&path), path, entries })
}

#[tauri::command]
#[specta::specta]
pub async fn fs_mkdir(app: AppHandle, side: Side, dir: String, name: String) -> FilesResult<String> {
    check_name(&name)?;
    let fs = fs_for(&app, side).await?;
    let path = fs.join(&dir, &name);
    fs.mkdir(&path).await?;
    Ok(path)
}

#[tauri::command]
#[specta::specta]
pub async fn fs_rename(app: AppHandle, side: Side, path: String, new_name: String) -> FilesResult<String> {
    check_name(&new_name)?;
    let fs = fs_for(&app, side).await?;
    let dir = fs.parent(&path).ok_or_else(|| FilesError::from(FsError::other("cannot rename the root")))?;
    let to = fs.join(&dir, &new_name);
    fs.rename(&path, &to).await?;
    Ok(to)
}

/// Deletes files and folders (recursively). The UI confirms first, naming
/// every target.
#[tauri::command]
#[specta::specta]
pub async fn fs_delete(app: AppHandle, side: Side, paths: Vec<String>) -> FilesResult<()> {
    let fs = fs_for(&app, side).await?;
    for p in &paths {
        if fs.parent(p).is_none() {
            return Err(FsError::other(format!("refusing to delete {p}")).into());
        }
    }
    for p in &paths {
        tracing::info!(?side, path = %p, "delete");
        fs.remove(p).await?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn fs_read_text(app: AppHandle, side: Side, path: String) -> FilesResult<TextFile> {
    let fs = fs_for(&app, side).await?;
    Ok(text::read_text(&fs, &path).await?)
}

/// Saves atomically. `expect` is the stamp from opening; a mismatch returns
/// a `changed` error unless the user chose to overwrite (`expect: null`).
#[tauri::command]
#[specta::specta]
pub async fn fs_save_text(
    app: AppHandle,
    side: Side,
    path: String,
    content: String,
    expect: Option<Stamp>,
) -> FilesResult<SaveResult> {
    let fs = fs_for(&app, side).await?;
    let r = text::save_text(&fs, &path, &content, expect).await?;
    tracing::info!(?side, %path, atomic = r.atomic, "saved");
    Ok(r)
}

fn check_name(name: &str) -> FilesResult<()> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\0'])
        || (cfg!(windows) && name.contains(['\\', ':', '*', '?', '"', '<', '>', '|']));
    if bad || name.len() > 255 {
        return Err(FsError::other(format!("invalid name: {name:?}")).into());
    }
    Ok(())
}

// --- bookmarks -------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn bookmarks_list(state: State<'_, AppState>, host_id: Option<HostId>) -> CmdResult<Vec<Bookmark>> {
    Ok(state.store().list_bookmarks(host_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn bookmark_add(
    state: State<'_, AppState>,
    host_id: Option<HostId>,
    path: String,
    label: Option<String>,
) -> CmdResult<Bookmark> {
    Ok(state.store().add_bookmark(host_id, &path, label.as_deref())?)
}

#[tauri::command]
#[specta::specta]
pub fn bookmark_delete(state: State<'_, AppState>, id: BookmarkId) -> CmdResult<()> {
    Ok(state.store().delete_bookmark(id)?)
}

// --- transfers -------------------------------------------------------------

/// Streams the job list to the UI (replaces any previous subscriber).
#[tauri::command]
#[specta::specta]
pub async fn transfers_subscribe(app: AppHandle, on_event: Channel<Vec<JobView>>) -> CmdResult<()> {
    transfers(&app).subscribe(Arc::new(move |jobs| {
        let _ = on_event.send(jobs);
    }));
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn transfers_add(app: AppHandle, jobs: Vec<NewJob>) -> CmdResult<Vec<JobId>> {
    if jobs.len() > 10_000 {
        return Err(CommandError::msg("too many items"));
    }
    Ok(transfers(&app).add(jobs))
}

#[tauri::command]
#[specta::specta]
pub async fn transfer_pause(app: AppHandle, id: JobId) -> CmdResult<()> {
    transfers(&app).pause(id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn transfer_resume(app: AppHandle, id: JobId) -> CmdResult<()> {
    transfers(&app).resume(id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn transfer_cancel(app: AppHandle, id: JobId) -> CmdResult<()> {
    transfers(&app).cancel(id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn transfer_resolve(app: AppHandle, id: JobId, policy: ConflictPolicy, all: bool) -> CmdResult<()> {
    transfers(&app).resolve(id, policy, all);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn transfers_clear(app: AppHandle) -> CmdResult<()> {
    transfers(&app).clear_finished();
    Ok(())
}
