use nightgrid_core::secrets::KeyringStore;
use nightgrid_core::settings::AppSettings;
use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::state::AppState;

/// Error returned to the frontend. Messages are user-safe and secret-free.
#[derive(Debug, thiserror::Error, Serialize, Type)]
#[error("{message}")]
pub struct CommandError {
    pub message: String,
}

impl From<nightgrid_core::Error> for CommandError {
    fn from(e: nightgrid_core::Error) -> Self {
        Self { message: e.to_string() }
    }
}

impl CommandError {
    pub fn msg(m: impl Into<String>) -> Self {
        Self { message: m.into() }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KeychainStatus {
    pub available: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
    pub log_dir: String,
    pub schema_version: i32,
    pub keychain: KeychainStatus,
    /// Linux only: whether the WebKitGTK DMA-BUF renderer was disabled.
    pub dmabuf_disabled: bool,
    /// Whether an SSH agent socket/pipe is configured for this process.
    pub agent_hint: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let schema_version = state.store().schema_version()? as i32;
    let keychain = match KeyringStore::available() {
        Ok(()) => KeychainStatus { available: true, detail: None },
        Err(e) => KeychainStatus { available: false, detail: Some(e) },
    };
    let agent_hint = if cfg!(windows) {
        Some(std::env::var("SSH_AUTH_SOCK").unwrap_or_else(|_| r"\\.\pipe\openssh-ssh-agent (or Pageant)".into()))
    } else {
        std::env::var("SSH_AUTH_SOCK").ok()
    };
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        data_dir: state.data_dir.display().to_string(),
        log_dir: state.log_dir.display().to_string(),
        schema_version,
        keychain,
        dmabuf_disabled: std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some(),
        agent_hint,
    })
}

#[tauri::command]
#[specta::specta]
pub fn settings_get(state: State<'_, AppState>) -> CmdResult<AppSettings> {
    Ok(AppSettings::load(&state.store())?)
}

#[tauri::command]
#[specta::specta]
pub fn settings_set(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut settings: AppSettings,
) -> CmdResult<AppSettings> {
    let store = state.store();
    // The sync folder is only changed through the backup commands, so a stale
    // copy in the UI can't clear it.
    settings.backup_folder = AppSettings::load(&store)?.backup_folder;
    settings.save(&store)?;
    drop(store);
    crate::tray::apply(&app);
    state.alerts.wake();
    tracing::debug!(theme = %settings.theme, "settings saved");
    Ok(settings)
}
