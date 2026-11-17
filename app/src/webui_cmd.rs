//! Opens a service's or container's web UI in its own Nightgrid window
//! (D-147). The window shows remote content, so it gets no capabilities and
//! no IPC: Tauri refuses app commands from remote origins, and the invoke
//! handler in lib.rs also refuses every webview but "main". The main window's
//! strict CSP is untouched (security rule 8).
//!
//! Reaching the page: when a host is given, the address is dialled straight
//! from this computer first; if that fails (the port is bound to loopback on
//! the host, the host is only on the built-in tailnet, or a firewall is in the
//! way) it goes through a temporary SSH tunnel, the same one "Open through
//! tunnel" uses.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::http;
use nightgrid_core::webui::{parse_url, WebUrl};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt as _;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Open web UI windows: URL → window label, so a second open focuses the first.
#[derive(Default)]
pub struct WebWindows {
    open: Mutex<HashMap<String, String>>,
    next: Mutex<u32>,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WebUiOpen {
    /// Whether a window is showing the page.
    pub opened: bool,
    /// What the window loaded (the local end of the tunnel, when tunnelled).
    pub url: String,
    pub via_tunnel: bool,
    /// Why it didn't open in Nightgrid, with a browser as the way out
    /// (an untrusted certificate, which the window can't step past).
    pub problem: Option<String>,
}

const DIRECT_TIMEOUT: Duration = Duration::from_millis(1500);
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// Dials `host:port` with the OS network only: the window's web engine can't
/// use Nightgrid's built-in tailnet, so that path doesn't count here.
async fn os_reachable(u: &WebUrl) -> bool {
    let addr = format!("{}:{}", if u.host.contains(':') { format!("[{}]", u.host) } else { u.host.clone() }, u.port);
    matches!(tokio::time::timeout(DIRECT_TIMEOUT, tokio::net::TcpStream::connect(addr)).await, Ok(Ok(_)))
}

/// Opens `url` in a Nightgrid window. With `host_id`, the URL is as that host
/// sees it (so `http://127.0.0.1:8080` means the host's own port 8080).
#[tauri::command]
#[specta::specta]
pub async fn webui_open(app: AppHandle, host_id: Option<HostId>, url: String, title: String) -> CmdResult<WebUiOpen> {
    let u = parse_url(&url).map_err(CommandError::msg)?;
    let host = host_id.and_then(|id| app.state::<AppState>().store().get_host(id).ok().flatten());
    let where_ = host.as_ref().map_or_else(|| u.host.clone(), |h| h.name.clone());

    let direct = !(host_id.is_some() && u.is_loopback()) && (host_id.is_none() || os_reachable(&u).await);
    let (open_url, via_tunnel) = if direct {
        (url.trim().to_owned(), false)
    } else {
        let id = host_id.expect("direct unless a host is given");
        // The host's own name may not resolve on the host itself (a tailnet
        // name, an ssh alias): its own ports are on loopback there anyway.
        let own = host.as_ref().is_some_and(|h| {
            h.hostname.eq_ignore_ascii_case(&u.host)
                || h.tailnet_address.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(&u.host))
        });
        let dest = if own { "127.0.0.1".to_owned() } else { u.host.clone() };
        let port = crate::tunnels_cmd::tunnel_open_temporary(app.clone(), id, dest, u.port).await?;
        (u.with_endpoint("127.0.0.1", port), true)
    };

    // Ask once before opening, so a dead port or a certificate the window
    // can't show is a clear message instead of a blank window.
    match http::send(http::Request::get(&open_url).timeout(PROBE_TIMEOUT)).await {
        Ok(_) => {}
        Err(e) if e.starts_with("certificate not trusted") || e.starts_with("TLS handshake failed") => {
            let why = if via_tunnel {
                "The page uses HTTPS, and through a tunnel its certificate can't match the address, so a Nightgrid window can't show it.".to_owned()
            } else {
                format!("Its {e}, and a Nightgrid window can't step past that.")
            };
            return Ok(WebUiOpen { opened: false, url: open_url, via_tunnel, problem: Some(why) });
        }
        Err(e) => {
            let msg = if via_tunnel {
                format!("Nothing answers on port {} on {} ({e}). Is the service running?", u.port, where_)
            } else {
                format!("{} didn't answer ({e}).", u.host)
            };
            return Err(CommandError::msg(msg));
        }
    }

    open_window(&app, &open_url, &title)?;
    Ok(WebUiOpen { opened: true, url: open_url, via_tunnel, problem: None })
}

fn open_window(app: &AppHandle, url: &str, title: &str) -> CmdResult<()> {
    let hub = app.state::<WebWindows>();
    let existing = hub.open.lock().expect("poisoned").get(url).cloned();
    if let Some(w) = existing.and_then(|label| app.get_webview_window(&label)) {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let label = {
        let mut n = hub.next.lock().expect("poisoned");
        *n += 1;
        format!("web-{n}")
    };
    let parsed: tauri::Url = url.parse().map_err(|_| CommandError::msg("That address can't be opened."))?;
    let opener = app.clone();
    let title = title.trim();
    let title = if title.is_empty() { url.to_owned() } else { title.chars().take(80).collect() };
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(parsed))
        .title(format!("{title} · NIGHTGRID"))
        .inner_size(1280.0, 860.0)
        .min_inner_size(480.0, 320.0)
        .background_color(tauri::window::Color(7, 8, 11, 255))
        // Only web pages: no file:, custom schemes or app links.
        .on_navigation(|u| matches!(u.scheme(), "http" | "https" | "about" | "blob" | "data"))
        // Pop-ups (target=_blank, window.open) go to the system browser.
        .on_new_window(move |u, _| {
            if matches!(u.scheme(), "http" | "https") {
                let _ = opener.opener().open_url(u.as_str(), None::<&str>);
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()
        .map_err(|e| CommandError::msg(format!("Couldn't open a window: {e}")))?;
    hub.open.lock().expect("poisoned").insert(url.to_owned(), label.clone());
    let app = app.clone();
    let key = url.to_owned();
    window.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            app.state::<WebWindows>().open.lock().expect("poisoned").remove(&key);
        }
    });
    Ok(())
}
