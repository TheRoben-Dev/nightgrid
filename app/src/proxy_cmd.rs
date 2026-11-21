//! Reverse proxy route editor (M10v): Caddyfiles (in a Caddy container or on
//! the host) and Nightgrid's Traefik file. See `nightgrid_core::docker::proxy_edit`.

use std::sync::Arc;

use nightgrid_core::confdiff::{self, FileDiff};
use nightgrid_core::docker::proxy_edit::{
    self, apply_caddy, CaddyEdit, CaddyTarget, EditTargets, SaveOutcome, TraefikRoute, TraefikTarget,
};
use nightgrid_core::docker::DockerHost;
use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::{self, ChangeArea as Area};
use nightgrid_core::system::sudo;
use nightgrid_core::system::SystemError;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::journal_cmd::{record, Change};
use crate::state::AppState;
use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_host;

fn other(e: impl std::fmt::Display) -> SystemError {
    SystemError::other(e.to_string())
}

async fn docker(app: &AppHandle, host_id: HostId) -> SysResult<Arc<DockerHost>> {
    let conn = connect_host(app, host_id).await?;
    let state = app.state::<AppState>();
    let user = crate::docker_cmd::docker_user_of(&state, host_id);
    Ok(state.docker.get(host_id, conn, user.as_deref()).await.map_err(other)?)
}

async fn read_caddy(d: &DockerHost, proxy_name: &str) -> SysResult<CaddyTarget> {
    Ok(if proxy_name == "host" { d.caddy_on_host().await } else { d.caddy_in_container(proxy_name).await }
        .map_err(other)?)
}

/// Every Caddyfile and Traefik instance on the host that routes can be edited in.
#[tauri::command]
#[specta::specta]
pub async fn proxy_edit_targets(app: AppHandle, host_id: HostId) -> SysResult<EditTargets> {
    let d = docker(&app, host_id).await?;
    Ok(d.edit_targets().await.map_err(other)?)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaddyPreview {
    pub text: String,
    pub diff: FileDiff,
}

/// A Caddyfile with `edit` applied and the diff, shown before saving.
/// Nothing is written.
#[tauri::command]
#[specta::specta]
pub fn caddy_edit_preview(text: String, edit: CaddyEdit) -> SysResult<CaddyPreview> {
    let new = apply_caddy(&text, &edit).map_err(other)?;
    let diff = confdiff::line_diff("Caddyfile", text.as_bytes(), new.as_bytes(), 3);
    Ok(CaddyPreview { text: new, diff })
}

/// Writes a new Caddyfile: refused if it changed since `base_hash` was read;
/// checked with `caddy validate`, loaded with `caddy reload`, and put back if
/// Caddy can't load it. The host's own Caddy needs sudo.
#[tauri::command]
#[specta::specta]
pub async fn caddy_save(
    app: AppHandle,
    host_id: HostId,
    proxy_name: String,
    text: String,
    base_hash: String,
    summary: String,
    sudo: Option<SudoInput>,
) -> SysResult<SaveOutcome> {
    let d = docker(&app, host_id).await?;
    let current = read_caddy(&d, &proxy_name).await?;
    if current.hash != base_hash {
        return Err(SystemError::other("the Caddyfile changed since it was opened: refresh and try again").into());
    }
    apply_caddy("", &CaddyEdit::Replace { text: text.clone() }).map_err(other)?;
    let on_host = proxy_name == "host";
    let script = proxy_edit::caddy_write_script(&current.path, &text, on_host).map_err(other)?;
    // The summary names the change, never the file's text (rule 1).
    let summary: String = summary.chars().take(120).collect();
    tracing::info!(host = %host_id, proxy = %proxy_name, change = %summary, "caddyfile write");
    let outcome = if on_host {
        let conn = connect_host(&app, host_id).await?;
        let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
        let o = sudo::run(&conn, &auth, &script).await?;
        proxy_edit::parse_write(&o.stdout, &o.stderr)
    } else {
        let (_, out) = d.exec_script(&proxy_name, &script).await.map_err(other)?;
        proxy_edit::parse_write(&out, &out)
    };
    let title = format!("Caddyfile: {}", if summary.trim().is_empty() { "edited" } else { summary.trim() });
    let detail =
        format!("{} ({})", current.path, if proxy_name == "host" { "on the host" } else { proxy_name.as_str() });
    let detail = if outcome.ok { detail } else { format!("{detail}: {}", outcome.message) };
    let diff = if outcome.ok { journal::text_diff(&current.text, &text) } else { None };
    record(&app, Change::new(host_id, Area::Proxy, &title).detail(&detail).diff(diff).ok(outcome.ok));
    Ok(outcome)
}

/// Replaces Nightgrid's Traefik routes on `proxy_name` (removes the file
/// when there are none). Refused if the file changed since `base_hash`.
#[tauri::command]
#[specta::specta]
pub async fn traefik_save(
    app: AppHandle,
    host_id: HostId,
    proxy_name: String,
    routes: Vec<TraefikRoute>,
    base_hash: String,
) -> SysResult<TraefikTarget> {
    let routes = routes.into_iter().map(TraefikRoute::normalized).collect::<Result<Vec<_>, _>>().map_err(other)?;
    let d = docker(&app, host_id).await?;
    let t = d.traefik_target(&proxy_name).await.map_err(other)?;
    if let Some(b) = t.blocked {
        return Err(SystemError::other(b).into());
    }
    if t.hash != base_hash {
        return Err(
            SystemError::other("Nightgrid's Traefik file changed since it was opened: refresh and try again").into()
        );
    }
    let dir = t.config.directory.clone().unwrap_or_default();
    let text = if routes.is_empty() { None } else { Some(proxy_edit::traefik_yaml(&routes).map_err(other)?) };
    let script = proxy_edit::traefik_write_script(&dir, text.as_deref()).map_err(other)?;
    tracing::info!(host = %host_id, proxy = %proxy_name, routes = routes.len(), "traefik routes write");
    let (_, out) = d.exec_script(&proxy_name, &script).await.map_err(other)?;
    if !out.contains("@@ok") {
        return Err(
            SystemError::other(format!("couldn't write {dir}/{}: {}", proxy_edit::TRAEFIK_FILE, out.trim())).into()
        );
    }
    let title = format!("Traefik routes saved: {} route{}", routes.len(), if routes.len() == 1 { "" } else { "s" });
    record(&app, Change::new(host_id, Area::Proxy, &title).detail(&format!("{dir} ({proxy_name})")));
    Ok(d.traefik_target(&proxy_name).await.map_err(other)?)
}
