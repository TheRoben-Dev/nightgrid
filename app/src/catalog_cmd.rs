//! Service catalog (1.1, D-170): what runs where, read on demand. See
//! `core/src/catalog.rs`. Also the shared "save a report as Markdown"
//! command the 1.1 tabs use.

use std::path::PathBuf;
use std::time::Duration;

use nightgrid_core::backup_jobs::BackupSource;
use nightgrid_core::catalog::{self, CatalogHost, JobPaths};
use nightgrid_core::files::path::shell_quote;
use nightgrid_core::hosts::{Host, HostId};
use tauri::{AppHandle, Manager as _};
use tauri_plugin_dialog::DialogExt as _;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

const HOST_TIMEOUT: Duration = Duration::from_secs(60);
const MARK: &str = "@@NG-UNIT ";

/// The names a URL might use for this host.
fn names(h: &Host) -> Vec<String> {
    let mut v = vec![h.hostname.clone(), h.name.clone()];
    v.extend(h.tailnet_address.clone());
    v.extend(h.ssh_alias.clone());
    v
}

/// `systemctl cat` of each unit job on the host in one exec.
async fn job_paths(app: &AppHandle, host_id: HostId, units: &[(String, String)]) -> Vec<JobPaths> {
    if units.is_empty() {
        return Vec::new();
    }
    let script: String = units
        .iter()
        .enumerate()
        .map(|(i, (_, unit))| format!("echo '{MARK}{i}'; systemctl cat -- {} 2>/dev/null; ", shell_quote(unit)))
        .collect();
    let Ok(conn) = crate::term_cmd::connect_host(app, host_id).await else { return Vec::new() };
    let Ok(Ok((_, out, _))) = tokio::time::timeout(Duration::from_secs(20), conn.exec(&script)).await else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out);
    let mut out = Vec::new();
    for part in text.split(MARK).skip(1) {
        let (idx, body) = part.split_once('\n').unwrap_or((part, ""));
        let Some((name, _)) = idx.trim().parse::<usize>().ok().and_then(|i| units.get(i)) else { continue };
        let paths = catalog::paths_in_unit(body);
        if !paths.is_empty() {
            out.push(JobPaths { name: name.clone(), paths });
        }
    }
    out
}

async fn read_host(app: &AppHandle, h: &Host) -> CatalogHost {
    let mut out = CatalogHost { host_id: h.id, name: h.name.clone(), entries: Vec::new(), error: None, jobs_read: 0 };
    let (services, units) = {
        let state = app.state::<AppState>();
        let store = state.store();
        let services = store.list_services().unwrap_or_default();
        let units: Vec<(String, String)> = store
            .list_backup_jobs()
            .unwrap_or_default()
            .into_iter()
            .filter(|j| j.host_id == h.id && j.source == BackupSource::Unit)
            .map(|j| (j.name, j.target))
            .collect();
        (services, units)
    };
    let work = async {
        let d = crate::docker_cmd::docker_for(app, h.id).await.map_err(|e| e.message)?;
        let containers = d.containers().await.map_err(|e| e.to_string())?;
        let mut mounts = Vec::with_capacity(containers.len());
        for c in &containers {
            if let Ok(det) = d.inspect(&c.id).await {
                let labels = det.labels.into_iter().map(|kv| (kv.key, kv.value)).collect();
                mounts.push((c.id.clone(), det.mounts, labels));
            }
        }
        let routes = match tokio::time::timeout(Duration::from_secs(20), d.proxy_routes()).await {
            Ok(Ok(r)) => r.routes,
            _ => Vec::new(),
        };
        let jobs = job_paths(app, h.id, &units).await;
        Ok::<_, String>((containers, mounts, routes, jobs))
    };
    match tokio::time::timeout(HOST_TIMEOUT, work).await {
        Ok(Ok((containers, mounts, routes, jobs))) => {
            out.jobs_read = jobs.len() as u32;
            out.entries = catalog::build(&containers, &mounts, &names(h), &services, &routes, &jobs);
        }
        Ok(Err(e)) => out.error = Some(e),
        Err(_) => out.error = Some("the host took longer than 60 s to answer".into()),
    }
    out
}

/// Reads the catalog of these hosts (three at a time). Nothing is stored.
#[tauri::command]
#[specta::specta]
pub async fn catalog_build(app: AppHandle, host_ids: Vec<HostId>) -> CmdResult<Vec<CatalogHost>> {
    let hosts: Vec<Host> = {
        let state = app.state::<AppState>();
        let list = state.store().list_hosts()?;
        list.into_iter().filter(|h| host_ids.contains(&h.id)).collect()
    };
    let mut out = Vec::with_capacity(hosts.len());
    for chunk in hosts.chunks(3) {
        out.extend(futures_util::future::join_all(chunk.iter().map(|h| read_host(&app, h))).await);
    }
    Ok(out)
}

/// The catalog as Markdown (for the export and for DRIFT).
#[tauri::command]
#[specta::specta]
pub fn catalog_markdown(hosts: Vec<CatalogHost>) -> String {
    catalog::to_markdown(&hosts, &chrono::Local::now().format("%Y-%m-%d").to_string())
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 80
        && !n.starts_with('.')
        && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && (n.ends_with(".md") || n.ends_with(".html"))
}

/// Saves a report the user asked for where they pick (a Save dialog).
/// `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn report_save(app: AppHandle, name: String, content: String) -> CmdResult<Option<String>> {
    if !valid_name(&name) {
        return Err(CommandError::msg("a report name ends in .md or .html and has no folders"));
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let ext = if name.ends_with(".html") { ("HTML", "html") } else { ("Markdown", "md") };
    app.dialog().file().set_title("Save the report").set_file_name(&name).add_filter(ext.0, &[ext.1]).save_file(
        move |p| {
            let _ = tx.send(p);
        },
    );
    let Some(path): Option<PathBuf> = rx.await.ok().flatten().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| CommandError::msg(format!("Couldn't write {}: {e}", path.display())))?;
    Ok(Some(path.display().to_string()))
}
