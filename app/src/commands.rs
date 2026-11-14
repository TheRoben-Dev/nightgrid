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

type CmdResult<T> = Result<T, CommandError>;

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
}

#[tauri::command]
#[specta::specta]
pub fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let schema_version = state.store.lock().expect("store poisoned").schema_version()? as i32;
    let keychain = match KeyringStore::available() {
        Ok(()) => KeychainStatus { available: true, detail: None },
        Err(e) => KeychainStatus { available: false, detail: Some(e) },
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
    })
}

#[tauri::command]
#[specta::specta]
pub fn settings_get(state: State<'_, AppState>) -> CmdResult<AppSettings> {
    Ok(AppSettings::load(&state.store.lock().expect("store poisoned"))?)
}

#[tauri::command]
#[specta::specta]
pub fn settings_set(state: State<'_, AppState>, settings: AppSettings) -> CmdResult<AppSettings> {
    settings.save(&state.store.lock().expect("store poisoned"))?;
    tracing::debug!(theme = %settings.theme, "settings saved");
    Ok(settings)
}
