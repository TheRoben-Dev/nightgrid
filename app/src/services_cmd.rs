//! Services board: CRUD, a background health checker, and Homepage import.
//!
//! The checker is one task that wakes every `services_interval_secs`, checks
//! every enabled service (8 at a time), stores the result and pushes it to the
//! UI channel. Edits wake it early so a new service gets a status right away.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use nightgrid_core::files::text::read_text;
use nightgrid_core::files::transfer::Side;
use nightgrid_core::files::{Fs, FsError};
use nightgrid_core::hosts::HostId;
use nightgrid_core::services::check::{self, CheckResult};
use nightgrid_core::services::homepage::{self, HomepageParse, ImportSummary};
use nightgrid_core::services::{CheckPoint, Service, ServiceId, ServiceInput};
use nightgrid_core::settings::AppSettings;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Notify;

use crate::commands::CmdResult;
use crate::docker_cmd::docker_for;
use crate::files_cmd::{fs_for, FilesError, FilesResult};
use crate::state::AppState;

/// Service check history is kept this long.
const RETENTION_DAYS: i64 = 30;
const PARALLEL: usize = 8;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub service_id: ServiceId,
    /// Unix seconds.
    #[specta(type = u32)]
    pub ts: i64,
    pub result: CheckResult,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServiceEvent {
    Status(ServiceStatus),
    /// The service was deleted or its checks were turned off.
    Cleared {
        service_id: ServiceId,
    },
}

#[derive(Default)]
pub struct ServicesHub {
    channel: Mutex<Option<Channel<ServiceEvent>>>,
    last: Mutex<HashMap<ServiceId, ServiceStatus>>,
    wake: Arc<Notify>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl ServicesHub {
    /// Latest check result per service.
    pub fn latest(&self) -> HashMap<ServiceId, ServiceStatus> {
        self.last.lock().expect("poisoned").clone()
    }

    fn send(&self, ev: ServiceEvent) {
        match &ev {
            ServiceEvent::Status(s) => {
                self.last.lock().expect("poisoned").insert(s.service_id, s.clone());
            }
            ServiceEvent::Cleared { service_id } => {
                self.last.lock().expect("poisoned").remove(service_id);
            }
        }
        if let Some(ch) = self.channel.lock().expect("poisoned").as_ref() {
            let _ = ch.send(ev);
        }
    }
}

async fn run_check(app: &AppHandle, svc: &Service) -> ServiceStatus {
    let result = check::check(&svc.url, &svc.expect_status, svc.allow_untrusted, check::DEFAULT_TIMEOUT).await;
    let ts = check::now_unix();
    let state = app.state::<AppState>();
    if let Err(e) = state.store().record_check(svc.id, ts, result.up, result.status, result.latency_ms) {
        // The service may have been deleted mid-check.
        tracing::debug!(service = svc.id, "check not recorded: {e}");
    }
    ServiceStatus { service_id: svc.id, ts, result }
}

fn start_checker(app: &AppHandle, hub: &Arc<ServicesHub>) {
    let mut task = hub.task.lock().expect("poisoned");
    if task.is_some() {
        return;
    }
    let app = app.clone();
    let hub = hub.clone();
    *task = Some(tauri::async_runtime::spawn(async move {
        let mut last_prune = 0i64;
        loop {
            let (settings, services) = {
                let state = app.state::<AppState>();
                let store = state.store();
                let settings = AppSettings::load(&store).unwrap_or_default().effective();
                (settings, store.list_services().unwrap_or_default())
            };
            if settings.services_checks {
                let checks = services.into_iter().filter(|s| s.check_enabled).map(|s| {
                    let app = app.clone();
                    async move { run_check(&app, &s).await }
                });
                let mut stream = Box::pin(futures_util::stream::iter(checks).buffer_unordered(PARALLEL));
                while let Some(status) = stream.next().await {
                    hub.send(ServiceEvent::Status(status));
                }
            }
            let now = check::now_unix();
            if now - last_prune > 3600 {
                last_prune = now;
                let state = app.state::<AppState>();
                let _ = state.store().prune_service_checks(now, RETENTION_DAYS);
            }
            let wait = Duration::from_secs(u64::from(settings.services_interval_secs.max(15)));
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = hub.wake.notified() => {}
            }
        }
    }));
}

/// Registers the UI channel (replacing any earlier one), re-sends the latest
/// statuses and starts the background checker.
#[tauri::command]
#[specta::specta]
pub fn services_subscribe(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<ServiceEvent>,
) -> CmdResult<()> {
    let hub = state.services.clone();
    for s in hub.last.lock().expect("poisoned").values() {
        let _ = on_event.send(ServiceEvent::Status(s.clone()));
    }
    *hub.channel.lock().expect("poisoned") = Some(on_event);
    start_checker(&app, &hub);
    Ok(())
}

/// Wakes the checker (after settings change, or "check all now").
#[tauri::command]
#[specta::specta]
pub fn services_check_all(state: State<'_, AppState>) -> CmdResult<()> {
    state.services.wake.notify_one();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn services_list(state: State<'_, AppState>) -> CmdResult<Vec<Service>> {
    Ok(state.store().list_services()?)
}

#[tauri::command]
#[specta::specta]
pub fn service_create(state: State<'_, AppState>, input: ServiceInput) -> CmdResult<Service> {
    let s = state.store().create_service(&input)?;
    state.services.wake.notify_one();
    Ok(s)
}

#[tauri::command]
#[specta::specta]
pub fn service_update(state: State<'_, AppState>, id: ServiceId, input: ServiceInput) -> CmdResult<Service> {
    let s = state.store().update_service(id, &input)?;
    if !s.check_enabled {
        state.services.send(ServiceEvent::Cleared { service_id: id });
    }
    state.services.wake.notify_one();
    Ok(s)
}

#[tauri::command]
#[specta::specta]
pub fn service_delete(state: State<'_, AppState>, id: ServiceId) -> CmdResult<()> {
    state.store().delete_service(id)?;
    state.services.send(ServiceEvent::Cleared { service_id: id });
    Ok(())
}

/// Checks one service now (also stored and pushed to the channel).
#[tauri::command]
#[specta::specta]
pub async fn service_check(app: AppHandle, id: ServiceId) -> CmdResult<ServiceStatus> {
    let svc = app.state::<AppState>().store().get_service(id)?;
    let status = run_check(&app, &svc).await;
    app.state::<AppState>().services.send(ServiceEvent::Status(status.clone()));
    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub fn service_history(state: State<'_, AppState>, id: ServiceId, from: f64) -> CmdResult<Vec<CheckPoint>> {
    Ok(state.store().service_checks(id, from as i64)?)
}

// ---- Homepage import ----

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HomepageCandidate {
    pub container: String,
    /// Host directory mounted at the container's `/app/config`.
    pub dir: String,
}

/// Finds Homepage containers on a host and where their config lives.
/// Best effort: no Docker (or no access) just means no suggestions.
#[tauri::command]
#[specta::specta]
pub async fn homepage_detect(app: AppHandle, host_id: HostId) -> CmdResult<Vec<HomepageCandidate>> {
    let Ok(docker) = docker_for(&app, host_id).await else { return Ok(Vec::new()) };
    let Ok(containers) = docker.containers().await else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for c in containers.iter().filter(|c| c.image.to_ascii_lowercase().contains("homepage")) {
        if let Ok(detail) = docker.inspect(&c.id).await {
            for m in detail.mounts.iter().filter(|m| m.destination.trim_end_matches('/') == "/app/config") {
                if !m.source.is_empty() {
                    out.push(HomepageCandidate { container: c.name.clone(), dir: m.source.clone() });
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HomepagePreview {
    /// The services.yaml that was read.
    pub path: String,
    /// Whether settings.yaml was found next to it (for group order).
    pub layout: bool,
    pub parse: HomepageParse,
}

/// `dir` is the config directory or the services.yaml itself.
fn config_paths(fs: &Fs, dir: &str) -> (String, String) {
    let lower = dir.to_ascii_lowercase();
    if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        let parent = match fs {
            Fs::Local => {
                std::path::Path::new(dir).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
            }
            _ => dir.rsplit_once('/').map(|(p, _)| p.to_owned()).unwrap_or_default(),
        };
        return (dir.to_owned(), join(fs, &parent, "settings.yaml"));
    }
    (join(fs, dir, "services.yaml"), join(fs, dir, "settings.yaml"))
}

fn join(fs: &Fs, dir: &str, name: &str) -> String {
    match fs {
        Fs::Local => std::path::Path::new(dir).join(name).to_string_lossy().into_owned(),
        _ => format!("{}/{name}", dir.trim_end_matches('/')),
    }
}

async fn read_config(app: &AppHandle, side: Side, dir: &str) -> FilesResult<HomepagePreview> {
    let fs = fs_for(app, side).await?;
    let (services_path, settings_path) = config_paths(&fs, dir.trim());
    let services = read_text(&fs, &services_path).await?;
    let order = match read_text(&fs, &settings_path).await {
        Ok(t) => homepage::parse_layout_order(&t.content).unwrap_or_default(),
        Err(FsError::NotFound { .. }) => Vec::new(),
        Err(e) => {
            tracing::debug!("settings.yaml not read: {e}");
            Vec::new()
        }
    };
    let parse = homepage::parse_services(&services.content, &order)?;
    Ok(HomepagePreview { path: services.path, layout: !order.is_empty(), parse })
}

/// Reads and parses a Homepage config without changing anything.
#[tauri::command]
#[specta::specta]
pub async fn homepage_preview(app: AppHandle, side: Side, dir: String) -> FilesResult<HomepagePreview> {
    read_config(&app, side, &dir).await
}

/// Re-reads the config and creates/updates services from it. `link_host`
/// ties new services to the host the file was read from.
#[tauri::command]
#[specta::specta]
pub async fn homepage_import(
    app: AppHandle,
    side: Side,
    dir: String,
    link_host: bool,
    prune: bool,
) -> FilesResult<ImportSummary> {
    let preview = read_config(&app, side, &dir).await?;
    let host_id = match side {
        Side::Host { id } if link_host => Some(id),
        _ => None,
    };
    let state = app.state::<AppState>();
    let sum = state.store().import_homepage(&preview.parse.entries, host_id, prune).map_err(FilesError::from)?;
    tracing::info!(added = sum.added, updated = sum.updated, removed = sum.removed, "homepage import");
    state.services.wake.notify_one();
    Ok(sum)
}
