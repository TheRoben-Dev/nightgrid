//! Tray / menu bar icon: host status with quick connect, saved tunnels
//! (click to start/stop), the active alert count, Show and Quit.
//!
//! The menu is rebuilt from the alerts loop (every 15 s) only when its
//! content changed, and right after a tunnel is toggled from it. Quick
//! connect and "alerts" are events for the UI, which opens the terminal or
//! the alerts panel; the tray itself never connects to anything.

use std::sync::Mutex;

use nightgrid_core::hosts::HostId;
use nightgrid_core::settings::AppSettings;
use serde::Serialize;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::metrics_cmd::PollState;
use crate::state::AppState;
use crate::tunnels_cmd::TunnelState;

const ID: &str = "main";
/// Event for the UI: open a terminal on this host.
pub const EVENT_CONNECT: &str = "tray://connect";
/// Event for the UI: open the alerts panel.
pub const EVENT_ALERTS: &str = "tray://alerts";

/// Last menu content, to skip rebuilding an unchanged menu.
static LAST: Mutex<Option<String>> = Mutex::new(None);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectPayload {
    host_id: HostId,
}

struct HostRow {
    id: HostId,
    name: String,
    status: &'static str,
}

struct TunnelRow {
    id: i32,
    label: String,
    running: bool,
}

struct Content {
    hosts: Vec<HostRow>,
    tunnels: Vec<TunnelRow>,
    alerts: (usize, usize),
}

impl Content {
    fn read(app: &AppHandle) -> Self {
        let state = app.state::<AppState>();
        let polls = state.metrics.poll_states();
        let connected = state.pool.connected();
        let hosts = state
            .store()
            .list_hosts()
            .unwrap_or_default()
            .into_iter()
            .map(|h| {
                let status = match polls.get(&h.id).map(|p| &p.0) {
                    Some(PollState::Ok) => "online",
                    Some(PollState::Offline) => "OFFLINE",
                    Some(PollState::NeedsLogin) => "needs login",
                    Some(PollState::Error) => "error",
                    Some(PollState::Suspended) => "suspended",
                    Some(PollState::Rebooting) => "rebooting",
                    Some(PollState::ShuttingDown) => "shutting down",
                    Some(PollState::Asleep) => "asleep",
                    Some(PollState::Away) => "away",
                    _ if connected.contains(&h.id) => "connected",
                    _ => "",
                };
                HostRow { id: h.id, name: h.name, status }
            })
            .collect();
        let tunnels = crate::tunnels_cmd::tunnels_list(app.state())
            .unwrap_or_default()
            .into_iter()
            .filter(|t| !t.temporary)
            .map(|t| TunnelRow {
                id: t.tunnel.id,
                label: format!("{}  {}", t.tunnel.name, t.spec),
                running: matches!(t.state, TunnelState::Running { .. } | TunnelState::Starting),
            })
            .collect();
        Self { hosts, tunnels, alerts: state.alerts.active_count() }
    }

    fn signature(&self) -> String {
        let mut s = format!("{:?};", self.alerts);
        for h in &self.hosts {
            s.push_str(&format!("h{}:{}:{};", h.id, h.name, h.status));
        }
        for t in &self.tunnels {
            s.push_str(&format!("t{}:{}:{};", t.id, t.label, t.running));
        }
        s
    }

    fn menu(&self, app: &AppHandle) -> tauri::Result<Menu<Wry>> {
        let header = MenuItem::with_id(app, "header", "NIGHTGRID", false, None::<&str>)?;
        let (n, crit) = self.alerts;
        let alerts_text = match (n, crit) {
            (0, _) => "No active alerts".to_owned(),
            (n, 0) => format!("{n} active alert{}", if n == 1 { "" } else { "s" }),
            (n, c) => format!("{n} active alert{} ({c} critical)", if n == 1 { "" } else { "s" }),
        };
        let alerts = MenuItem::with_id(app, "alerts", alerts_text, true, None::<&str>)?;

        let host_items: Vec<MenuItem<Wry>> = self
            .hosts
            .iter()
            .map(|h| {
                let text = if h.status.is_empty() { h.name.clone() } else { format!("{}  ·  {}", h.name, h.status) };
                MenuItem::with_id(app, format!("host:{}", h.id), text, true, None::<&str>)
            })
            .collect::<tauri::Result<_>>()?;
        let host_refs: Vec<&dyn IsMenuItem<Wry>> = host_items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
        let hosts = Submenu::with_id_and_items(app, "hosts", "Connect", !host_refs.is_empty(), &host_refs)?;

        let tunnel_items: Vec<CheckMenuItem<Wry>> = self
            .tunnels
            .iter()
            .map(|t| CheckMenuItem::with_id(app, format!("tunnel:{}", t.id), &t.label, true, t.running, None::<&str>))
            .collect::<tauri::Result<_>>()?;
        let tunnel_refs: Vec<&dyn IsMenuItem<Wry>> = tunnel_items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
        let running = self.tunnels.iter().filter(|t| t.running).count();
        let tunnels_label = if running > 0 { format!("Tunnels ({running} on)") } else { "Tunnels".to_owned() };
        let tunnels = Submenu::with_id_and_items(app, "tunnels", tunnels_label, !tunnel_refs.is_empty(), &tunnel_refs)?;

        let show = MenuItem::with_id(app, "show", "Show Nightgrid", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit Nightgrid", true, None::<&str>)?;
        Menu::with_items(
            app,
            &[
                &header,
                &alerts,
                &PredefinedMenuItem::separator(app)?,
                &hosts,
                &tunnels,
                &PredefinedMenuItem::separator(app)?,
                &show,
                &quit,
            ],
        )
    }

    fn tooltip(&self) -> String {
        match self.alerts.0 {
            0 => "Nightgrid".into(),
            n => format!("Nightgrid · {n} alert{}", if n == 1 { "" } else { "s" }),
        }
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        "show" => show_main(app),
        "quit" => app.exit(0),
        "alerts" => {
            show_main(app);
            let _ = app.emit(EVENT_ALERTS, ());
        }
        _ => {
            if let Some(h) = id.strip_prefix("host:").and_then(|v| v.parse().ok()) {
                show_main(app);
                let _ = app.emit(EVENT_CONNECT, ConnectPayload { host_id: h });
            } else if let Some(t) = id.strip_prefix("tunnel:").and_then(|v| v.parse::<i32>().ok()) {
                let running = Content::read(app).tunnels.iter().any(|x| x.id == t && x.running);
                let res = if running {
                    crate::tunnels_cmd::tunnel_stop(app.state(), t)
                } else {
                    crate::tunnels_cmd::tunnel_start(app.clone(), t)
                };
                if let Err(e) = res {
                    tracing::warn!(tunnel = t, "tray tunnel toggle: {e}");
                }
                *LAST.lock().expect("poisoned") = None;
                refresh(app);
            }
        }
    }
}

/// Whether a tray icon exists (close-to-tray only hides when it does).
pub fn exists(app: &AppHandle) -> bool {
    app.tray_by_id(ID).is_some()
}

/// Creates or removes the tray icon to match settings.
pub fn apply(app: &AppHandle) {
    let enabled = AppSettings::load(&app.state::<AppState>().store()).unwrap_or_default().tray_enabled;
    match (enabled, exists(app)) {
        (true, false) => create(app),
        (false, true) => {
            let _ = app.remove_tray_by_id(ID);
            *LAST.lock().expect("poisoned") = None;
        }
        _ => {}
    }
}

fn create(app: &AppHandle) {
    let content = Content::read(app);
    let build = || -> tauri::Result<()> {
        let menu = content.menu(app)?;
        let mut b = TrayIconBuilder::with_id(ID)
            .menu(&menu)
            .tooltip(content.tooltip())
            .show_menu_on_left_click(true)
            .on_menu_event(on_menu);
        #[cfg(target_os = "macos")]
        {
            b = b
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?)
                .icon_as_template(true);
        }
        #[cfg(not(target_os = "macos"))]
        if let Some(icon) = app.default_window_icon() {
            b = b.icon(icon.clone());
        }
        b.build(app)?;
        Ok(())
    };
    // Linux loads libayatana-appindicator at runtime and panics when it's
    // missing; a missing tray must not take the app down with it.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(build)) {
        Ok(Ok(())) => *LAST.lock().expect("poisoned") = Some(content.signature()),
        Ok(Err(e)) => tracing::warn!("tray icon unavailable: {e}"),
        Err(_) => tracing::warn!("tray icon unavailable (no appindicator library?)"),
    }
}

/// Rebuilds the menu if anything in it changed.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(ID) else { return };
    let content = Content::read(app);
    let sig = content.signature();
    {
        let mut last = LAST.lock().expect("poisoned");
        if last.as_deref() == Some(sig.as_str()) {
            return;
        }
        *last = Some(sig);
    }
    match content.menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(content.tooltip()));
        }
        Err(e) => tracing::warn!("tray menu: {e}"),
    }
}
