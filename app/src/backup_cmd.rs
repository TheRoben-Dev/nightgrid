//! Encrypted config export/import. Files are picked with native dialogs on
//! the Rust side (the webview gets no file system or dialog permission), or
//! read from / written to the optional sync folder.

use std::path::{Path, PathBuf};

use nightgrid_core::backup::{self, BackupSummary, ImportMode};
use nightgrid_core::settings::AppSettings;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupWritten {
    pub path: String,
    pub secrets: u32,
    pub bytes: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupPreview {
    pub path: String,
    pub summary: BackupSummary,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupImported {
    pub added: u32,
    pub skipped: u32,
    pub secrets: u32,
    pub settings: bool,
    /// The config was imported but the keychain step failed.
    pub secrets_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupFolder {
    pub path: String,
    /// Unix seconds the backup file in it was last written, if there is one.
    pub modified: Option<f64>,
}

fn folder_file(state: &AppState) -> CmdResult<PathBuf> {
    let s = AppSettings::load(&state.store())?;
    let dir = s.backup_folder.ok_or_else(|| CommandError::msg("Pick a sync folder first."))?;
    Ok(Path::new(&dir).join(backup::FILE_NAME))
}

fn modified(path: &Path) -> Option<f64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as f64)
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

async fn pick_save(app: &AppHandle) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let name = format!("nightgrid-backup-{}.age", chrono::Local::now().format("%Y-%m-%d"));
    app.dialog()
        .file()
        .set_title("Save Nightgrid backup")
        .set_file_name(name)
        .add_filter("Nightgrid backup", &["age"])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

async fn pick_open(app: &AppHandle) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Open Nightgrid backup").add_filter("Nightgrid backup", &["age"]).pick_file(
        move |p| {
            let _ = tx.send(p);
        },
    );
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

/// Writes next to the target and renames, so a sync tool never sees half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("age.partial");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn read_backup(path: &Path) -> CmdResult<Vec<u8>> {
    let meta = std::fs::metadata(path).map_err(|e| CommandError::msg(format!("Couldn't read the file: {e}")))?;
    if meta.len() > backup::MAX_FILE_BYTES {
        return Err(CommandError::msg("That file is too big to be a Nightgrid backup."));
    }
    std::fs::read(path).map_err(|e| CommandError::msg(format!("Couldn't read the file: {e}")))
}

/// Encrypts the config (and saved secrets, if asked) to a file picked in a
/// save dialog, or to the sync folder. `None` when the dialog was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn backup_export(
    app: AppHandle,
    passphrase: String,
    include_secrets: bool,
    to_folder: bool,
) -> CmdResult<Option<BackupWritten>> {
    if passphrase.chars().count() < backup::MIN_PASSPHRASE {
        return Err(CommandError::msg(format!("Use a passphrase of at least {} characters.", backup::MIN_PASSPHRASE)));
    }
    let state = app.state::<AppState>();
    let path = if to_folder {
        folder_file(&state)?
    } else {
        match pick_save(&app).await {
            Some(p) => p,
            None => return Ok(None),
        }
    };
    let mut bundle = {
        let store = state.store();
        let mut b = backup::snapshot(&store, env!("CARGO_PKG_VERSION"), now())?;
        if include_secrets {
            backup::collect_secrets(&store, &state.keychain, &mut b).map_err(|e| {
                CommandError::msg(format!(
                    "Couldn't read saved secrets from the keychain ({e}). Export without them, or fix the keychain."
                ))
            })?;
        }
        b
    };
    let secrets = bundle.secrets.len() as u32;
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let r = backup::encrypt(&bundle, &passphrase);
        bundle.secrets.clear();
        r
    })
    .await
    .map_err(|e| CommandError::msg(e.to_string()))??;
    write_atomic(&path, &bytes).map_err(|e| CommandError::msg(format!("Couldn't write the backup: {e}")))?;
    tracing::info!(path = %path.display(), secrets, "config exported");
    // Resets the export reminder.
    {
        let store = state.store();
        if let Ok(mut s) = AppSettings::load(&store) {
            s.last_profile_export = Some(now() as f64);
            s.profile_reminder_snooze = None;
            if let Err(e) = s.save(&store) {
                tracing::warn!("couldn't record the export time: {e}");
            }
        }
    }
    Ok(Some(BackupWritten { path: path.display().to_string(), secrets, bytes: bytes.len() as u32 }))
}

async fn decrypt_file(path: PathBuf, passphrase: String) -> CmdResult<backup::Bundle> {
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<backup::Bundle> {
        let bytes = read_backup(&path)?;
        Ok(backup::decrypt(&bytes, &passphrase)?)
    })
    .await
    .map_err(|e| CommandError::msg(e.to_string()))?
}

/// Picks a backup (dialog or sync folder), decrypts it and returns what it
/// holds. The path is remembered for [`backup_import`].
#[tauri::command]
#[specta::specta]
pub async fn backup_open(app: AppHandle, passphrase: String, from_folder: bool) -> CmdResult<Option<BackupPreview>> {
    let state = app.state::<AppState>();
    let path = if from_folder {
        let p = folder_file(&state)?;
        if !p.exists() {
            return Err(CommandError::msg("There's no backup in the sync folder yet."));
        }
        p
    } else {
        match pick_open(&app).await {
            Some(p) => p,
            None => return Ok(None),
        }
    };
    let summary = decrypt_file(path.clone(), passphrase).await?.summary();
    *state.pending_backup.lock().expect("poisoned") = Some(path.clone());
    Ok(Some(BackupPreview { path: path.display().to_string(), summary }))
}

/// Imports the backup picked with [`backup_open`]. The app should restart
/// afterwards so every view and background task reloads.
#[tauri::command]
#[specta::specta]
pub async fn backup_import(app: AppHandle, passphrase: String, mode: ImportMode) -> CmdResult<BackupImported> {
    let state = app.state::<AppState>();
    let path = state
        .pending_backup
        .lock()
        .expect("poisoned")
        .clone()
        .ok_or_else(|| CommandError::msg("Open a backup first."))?;
    let bundle = decrypt_file(path.clone(), passphrase).await?;
    let (outcome, secrets) = {
        let mut store = state.store();
        let before = AppSettings::load(&store)?;
        let outcome = backup::import(&mut store, &bundle, mode)?;
        if outcome.settings_applied {
            // The sync folder and the export reminder belong to this machine; keep ours.
            let mut s = AppSettings::load(&store)?;
            s.backup_folder = before.backup_folder;
            s.last_profile_export = before.last_profile_export;
            s.profile_reminder_snooze = before.profile_reminder_snooze;
            s.save(&store)?;
        }
        // The database is already committed: a keychain failure must not
        // read as a failed import.
        let secrets = backup::restore_secrets(&state.keychain, &bundle, &outcome).map_err(|e| {
            tracing::warn!("restoring keychain entries failed: {e}");
            e.to_string()
        });
        (outcome, secrets)
    };
    *state.pending_backup.lock().expect("poisoned") = None;
    let added = outcome.added.values().sum();
    let skipped = outcome.skipped.values().sum();
    let (secrets, secrets_error) = match secrets {
        Ok(n) => (n as u32, None),
        Err(e) => (0, Some(e)),
    };
    tracing::info!(path = %path.display(), ?mode, added, skipped, secrets, "config imported");
    Ok(BackupImported { added, skipped, secrets, settings: outcome.settings_applied, secrets_error })
}

/// Picks the sync folder with a native dialog and saves it in settings.
#[tauri::command]
#[specta::specta]
pub async fn backup_pick_folder(app: AppHandle) -> CmdResult<Option<BackupFolder>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Sync folder for backups").pick_folder(move |p| {
        let _ = tx.send(p);
    });
    let Some(dir) = rx.await.ok().flatten().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let state = app.state::<AppState>();
    let store = state.store();
    let mut s = AppSettings::load(&store)?;
    s.backup_folder = Some(dir.display().to_string());
    s.save(&store)?;
    Ok(Some(BackupFolder { path: dir.display().to_string(), modified: modified(&dir.join(backup::FILE_NAME)) }))
}

/// The sync folder and when its backup was last written.
#[tauri::command]
#[specta::specta]
pub fn backup_folder(state: State<'_, AppState>) -> CmdResult<Option<BackupFolder>> {
    let s = AppSettings::load(&state.store())?;
    Ok(s.backup_folder.map(|dir| {
        let m = modified(&Path::new(&dir).join(backup::FILE_NAME));
        BackupFolder { path: dir, modified: m }
    }))
}

/// Forgets the sync folder (the files in it stay).
#[tauri::command]
#[specta::specta]
pub fn backup_clear_folder(state: State<'_, AppState>) -> CmdResult<()> {
    let store = state.store();
    let mut s = AppSettings::load(&store)?;
    s.backup_folder = None;
    s.save(&store)?;
    Ok(())
}

/// Restarts the app (after an import).
#[tauri::command]
#[specta::specta]
pub fn app_restart(app: AppHandle) {
    app.restart();
}
