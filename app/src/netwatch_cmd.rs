//! Scheduled network watch (D-145): every few minutes (Settings → Alerts),
//! the chosen scanner runs the same discovery as the network map, records
//! it in the device inventory, and reports new devices, devices missing
//! from the expected map, and risks the scan shows. The scan runs in its
//! own task so a slow sweep never holds up the other alerts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nightgrid_core::alerts::Condition;
use nightgrid_core::netsec::NetFinding;
use nightgrid_core::settings::NetWatchSettings;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

#[derive(Default)]
pub struct NetWatchHub {
    /// When the last scan started (unix seconds).
    round: Mutex<i64>,
    running: AtomicBool,
    conds: Mutex<Option<Vec<Condition>>>,
    last: Mutex<Option<NetWatchRun>>,
}

/// The last scheduled scan, for the Network view.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NetWatchRun {
    #[specta(type = f64)]
    pub at: i64,
    pub scanner: String,
    pub devices: u32,
    pub new_devices: u32,
    pub missing: u32,
    pub findings: Vec<NetFinding>,
    pub error: Option<String>,
}

/// The watch's current conditions. Starts a scan in the background when one
/// is due; `None` until the first scan has finished.
pub fn evaluate(app: &AppHandle, cfg: &NetWatchSettings, now: i64) -> Option<Vec<Condition>> {
    let hub = app.state::<AppState>().netwatch.clone();
    let due = {
        let mut round = hub.round.lock().expect("poisoned");
        let due = now - *round >= i64::from(cfg.minutes.clamp(10, 1440)) * 60;
        if due && !hub.running.swap(true, Ordering::SeqCst) {
            *round = now;
            true
        } else {
            false
        }
    };
    if due {
        let app = app.clone();
        let cfg = cfg.clone();
        tauri::async_runtime::spawn(async move {
            scan(&app, &hub, &cfg, now).await;
            hub.running.store(false, Ordering::SeqCst);
        });
    }
    app.state::<AppState>().netwatch.conds.lock().expect("poisoned").clone()
}

/// Forgets the conditions, so turning the watch back on starts fresh.
pub fn reset(app: &AppHandle) {
    let hub = app.state::<AppState>().netwatch.clone();
    *hub.conds.lock().expect("poisoned") = None;
    *hub.round.lock().expect("poisoned") = 0;
}

async fn scan(app: &AppHandle, hub: &Arc<NetWatchHub>, cfg: &NetWatchSettings, now: i64) {
    let state = app.state::<AppState>();
    let (origin, scanner, res) = match cfg.scanner {
        Some(id) => {
            let name = state.store().get_host(id).ok().flatten().map_or_else(|| format!("host {id}"), |h| h.name);
            let res = match crate::term_cmd::connect_host(app, id).await {
                Ok(conn) => nightgrid_core::netmap::discover(&conn, cfg.sweep, &[]).await,
                Err(e) => Err(e.to_string()),
            };
            (id.to_string(), name, res)
        }
        None => (
            "local".to_owned(),
            "this computer".to_owned(),
            nightgrid_core::netmap::discover_local(cfg.sweep, &[]).await,
        ),
    };
    let run = match res {
        Ok(d) => {
            let d = crate::system_cmd::with_inventory(app, &origin, d);
            let expected: Vec<String> = state
                .store()
                .netmap_layout()
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.expected)
                .map(|e| e.key)
                .collect();
            let conds = nightgrid_core::netsec::watch_conditions(&d, &expected, cfg, &scanner, now);
            let run = NetWatchRun {
                at: now,
                scanner,
                devices: d.devices.len() as u32,
                new_devices: conds.iter().filter(|c| c.key.starts_with("netwatch:new:")).count() as u32,
                missing: conds.iter().filter(|c| c.key.starts_with("netwatch:missing:")).count() as u32,
                findings: d.findings,
                error: None,
            };
            *hub.conds.lock().expect("poisoned") = Some(conds);
            run
        }
        Err(e) => {
            // Keep the last conditions: a failed scan says nothing new.
            tracing::warn!("network watch: {e}");
            NetWatchRun { at: now, scanner, devices: 0, new_devices: 0, missing: 0, findings: vec![], error: Some(e) }
        }
    };
    *hub.last.lock().expect("poisoned") = Some(run);
}

/// The last scheduled network scan, if one ran since the app started.
#[tauri::command]
#[specta::specta]
pub fn netmap_watch_last(state: State<'_, AppState>) -> Option<NetWatchRun> {
    state.netwatch.last.lock().expect("poisoned").clone()
}
