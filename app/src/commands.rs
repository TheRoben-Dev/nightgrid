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
    let current = AppSettings::load(&store)?;
    settings.backup_folder = current.backup_folder;
    // The status page restarts its server on change: only through its own command.
    settings.status_page = current.status_page;
    // The built-in Tailscale client starts and stops through its own commands.
    settings.tailnet_enabled = current.tailnet_enabled;
    settings.tailnet_accept_routes = current.tailnet_accept_routes;
    settings.tailnet_hostname = current.tailnet_hostname;
    settings.tailnet_control_url = current.tailnet_control_url;
    let rules_changed = current.alerts != settings.alerts;
    settings.save(&store)?;
    drop(store);
    crate::tray::apply(&app);
    state.alerts.wake();
    if rules_changed {
        crate::node_cmd::rules_changed(&app);
    }
    tracing::debug!(theme = %settings.theme, "settings saved");
    Ok(settings)
}

/// Writes a diagnostics report (versions, settings, alerts, the newest app
/// logs; secrets removed) to a file the user picks. Returns its path, or
/// None when the dialog was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn diagnostics_export(app: tauri::AppHandle) -> CmdResult<Option<String>> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let state = app.state::<AppState>();
    let info = app_info(state.clone())?;
    let (settings, counts) = {
        let store = state.store();
        let s = AppSettings::load(&store).unwrap_or_default();
        let hosts = store.list_hosts().map(|h| h.len()).unwrap_or(0);
        let services = store.list_services().map(|v| v.len()).unwrap_or(0);
        (
            serde_json::to_string_pretty(&s).unwrap_or_default(),
            vec![("hosts".to_owned(), hosts), ("services".to_owned(), services)],
        )
    };
    let alerts: Vec<String> = {
        let mut v: Vec<String> = state
            .alerts
            .active()
            .iter()
            .map(|a| format!("ACTIVE {} · {} · {} · since {}", a.kind.label(), a.title, a.detail, a.since))
            .collect();
        v.extend(state.alerts.history().iter().take(50).map(|a| {
            format!(
                "RESOLVED {} · {} · {} · {}–{}",
                a.kind.label(),
                a.title,
                a.detail,
                a.since,
                a.resolved.unwrap_or(0)
            )
        }));
        v
    };
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&state.log_dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "log")).collect()
        })
        .unwrap_or_default();
    files.sort();
    let logs = files
        .iter()
        .rev()
        .take(3)
        .rev()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            let bytes = std::fs::read(p).ok()?;
            Some((name, String::from_utf8_lossy(&bytes).into_owned()))
        })
        .collect();
    let now = chrono::Local::now();
    let text = nightgrid_core::diagnostics::report(&nightgrid_core::diagnostics::Input {
        version: info.version,
        os: info.os,
        arch: info.arch,
        schema: i64::from(info.schema_version),
        keychain: if info.keychain.available {
            "available".into()
        } else {
            info.keychain.detail.unwrap_or_else(|| "unavailable".into())
        },
        generated: now.format("%Y-%m-%d %H:%M:%S %z").to_string(),
        counts,
        settings,
        alerts,
        logs,
    });
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Save Nightgrid diagnostics")
        .set_file_name(format!("nightgrid-diagnostics-{}.txt", now.format("%Y-%m-%d")))
        .add_filter("Text", &["txt"])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(path) = rx.await.ok().flatten().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&path, text).map_err(nightgrid_core::Error::Io)?;
    Ok(Some(path.display().to_string()))
}
