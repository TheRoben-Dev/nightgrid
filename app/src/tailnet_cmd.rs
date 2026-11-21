//! Built-in Tailscale (M10j): starts and stops the `nightgrid-tailnet`
//! sidecar, keeps its state key in the OS keychain and passes its status to
//! the UI (`tailnet://status`). Dialling through it is `core::tailnet::dial`.

use nightgrid_core::secrets::{Secret, SecretKind, SecretRef, SecretStore};
use nightgrid_core::settings::AppSettings;
use nightgrid_core::tailnet::{self, Sidecar, SidecarConfig, TailnetStatus};
use tauri::{AppHandle, Emitter as _, Manager as _, State};

use crate::commands::CmdResult;
use crate::state::AppState;

pub const EVENT_STATUS: &str = "tailnet://status";

#[derive(Default)]
pub struct TailnetHub {
    sidecar: tokio::sync::Mutex<Option<Sidecar>>,
    last: std::sync::Mutex<Option<TailnetStatus>>,
    /// Port profile sync wants forwarded from the tailnet (D-205).
    forward: std::sync::Mutex<Option<u16>>,
}

/// This app's own node on the built-in tailnet, while it runs.
pub fn self_node(state: &AppState) -> Option<nightgrid_core::tailnet::TailnetPeer> {
    state
        .tailnet
        .last
        .lock()
        .expect("poisoned")
        .as_ref()
        .filter(|s| s.state == "running")
        .and_then(|s| s.self_node.clone())
}

/// Profile sync's listener port to reach over the built-in tailnet (`None`
/// = stop). Remembered and sent again whenever the client starts.
pub async fn set_forward(state: &AppState, port: Option<u16>) {
    *state.tailnet.forward.lock().expect("poisoned") = port;
    if let Some(sc) = state.tailnet.sidecar.lock().await.as_mut() {
        let to = port.map(|p| std::net::SocketAddr::from(([127, 0, 0, 1], p)));
        if let Err(e) = sc.forward(to).await {
            tracing::warn!("built-in Tailscale: couldn't forward the sync port: {e}");
        }
    }
}

fn key_ref() -> SecretRef {
    SecretRef::new(SecretKind::TailnetStateKey, "built-in")
}

fn publish(app: &AppHandle, s: TailnetStatus) {
    if let Some(st) = app.try_state::<AppState>() {
        *st.tailnet.last.lock().expect("poisoned") = Some(s.clone());
    }
    let _ = app.emit(EVENT_STATUS, s);
}

fn current(state: &AppState, settings: &AppSettings) -> TailnetStatus {
    if let Some(s) = state.tailnet.last.lock().expect("poisoned").clone() {
        return s;
    }
    let mut s = TailnetStatus::off(if tailnet::sidecar_path().is_none() { "unavailable" } else { "off" });
    s.accept_routes = settings.tailnet_accept_routes;
    s
}

/// Starts the sidecar with the saved settings (stopping a running one first).
async fn start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::load(&state.store()).map_err(|e| e.to_string())?;
    stop(app).await;
    let Some(bin) = tailnet::sidecar_path() else {
        publish(app, TailnetStatus::off("unavailable"));
        return Err("This build doesn't include the built-in Tailscale client.".into());
    };
    let key = match state.keychain.get(&key_ref()).map_err(|e| e.to_string())? {
        Some(k) => k.expose().to_owned(),
        None => {
            let k = tailnet::new_state_key();
            state.keychain.set(&key_ref(), &Secret::new(k.clone())).map_err(|e| {
                format!("The built-in Tailscale client keeps its key in the OS keychain, which isn't available: {e}")
            })?;
            k
        }
    };
    let cfg = SidecarConfig {
        key,
        dir: state.data_dir.join("tailnet"),
        hostname: settings.tailnet_hostname.clone(),
        accept_routes: settings.tailnet_accept_routes,
        control_url: settings.tailnet_control_url.trim().to_owned(),
    };
    let handle = app.clone();
    let sc = Sidecar::start(&bin, &cfg, move |s| publish(&handle, s)).await.map_err(|e| {
        let mut s = TailnetStatus::off("failed");
        s.error = Some(e.to_string());
        publish(app, s);
        format!("Couldn't start the built-in Tailscale client: {e}")
    })?;
    let fwd = *state.tailnet.forward.lock().expect("poisoned");
    let mut sc = sc;
    if let Some(p) = fwd {
        if let Err(e) = sc.forward(Some(std::net::SocketAddr::from(([127, 0, 0, 1], p)))).await {
            tracing::warn!("built-in Tailscale: couldn't forward the sync port: {e}");
        }
    }
    *state.tailnet.sidecar.lock().await = Some(sc);
    Ok(())
}

async fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    let sc = state.tailnet.sidecar.lock().await.take();
    if let Some(sc) = sc {
        sc.stop().await;
        let mut s = TailnetStatus::off("off");
        s.accept_routes = AppSettings::load(&state.store()).map(|c| c.tailnet_accept_routes).unwrap_or(true);
        publish(app, s);
    }
}

/// At launch: start the client if it was on.
pub fn start_if_enabled(app: &AppHandle) {
    let on =
        app.try_state::<AppState>().and_then(|s| AppSettings::load(&s.store()).ok()).is_some_and(|c| c.tailnet_enabled);
    if on {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = start(&app).await {
                tracing::warn!("built-in Tailscale: {e}");
            }
        });
    }
}

#[tauri::command]
#[specta::specta]
pub fn tailnet_status(state: State<'_, AppState>) -> CmdResult<TailnetStatus> {
    let settings = AppSettings::load(&state.store())?;
    Ok(current(&state, &settings))
}

fn save(state: &AppState, f: impl FnOnce(&mut AppSettings)) -> CmdResult<AppSettings> {
    let store = state.store();
    let mut s = AppSettings::load(&store)?;
    f(&mut s);
    s.save(&store)?;
    Ok(s)
}

/// Turns the built-in client on (starts it) or off (stops it; the node
/// stays logged in for next time).
#[tauri::command]
#[specta::specta]
pub async fn tailnet_set_enabled(app: AppHandle, enabled: bool) -> Result<TailnetStatus, String> {
    let state = app.state::<AppState>();
    let settings = save(&state, |s| s.tailnet_enabled = enabled).map_err(|e| e.to_string())?;
    if enabled {
        start(&app).await?;
    } else {
        stop(&app).await;
    }
    Ok(current(&state, &settings))
}

/// Saves the node name, control server and subnet-route choice. A running
/// client picks up routes at once and restarts for the other two.
#[tauri::command]
#[specta::specta]
pub async fn tailnet_configure(
    app: AppHandle,
    hostname: String,
    control_url: String,
    accept_routes: bool,
) -> Result<TailnetStatus, String> {
    let state = app.state::<AppState>();
    let before = AppSettings::load(&state.store()).map_err(|e| e.to_string())?;
    let settings = save(&state, |s| {
        s.tailnet_hostname = hostname.trim().to_owned();
        s.tailnet_control_url = control_url.trim().to_owned();
        s.tailnet_accept_routes = accept_routes;
    })
    .map_err(|e| e.to_string())?;
    let restart = before.tailnet_hostname != settings.tailnet_hostname
        || before.tailnet_control_url != settings.tailnet_control_url;
    let running = state.tailnet.sidecar.lock().await.is_some();
    if running && restart {
        start(&app).await?;
    } else if let Some(sc) = state.tailnet.sidecar.lock().await.as_mut() {
        sc.accept_routes(accept_routes).await.map_err(|e| e.to_string())?;
        publish(&app, sc.status());
    }
    Ok(current(&state, &settings))
}

/// Asks for a login link; it arrives in the status as `loginUrl`.
#[tauri::command]
#[specta::specta]
pub async fn tailnet_login(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut g = state.tailnet.sidecar.lock().await;
    let sc = g.as_mut().ok_or("Turn on the built-in Tailscale client first.")?;
    sc.login().await.map_err(|e| e.to_string())
}

/// Logs this node out of the tailnet.
#[tauri::command]
#[specta::specta]
pub async fn tailnet_logout(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut g = state.tailnet.sidecar.lock().await;
    let sc = g.as_mut().ok_or("The built-in Tailscale client isn't running.")?;
    sc.logout().await.map_err(|e| e.to_string())
}
