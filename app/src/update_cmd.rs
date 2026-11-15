//! App self-update from GitHub Releases (Tauri updater).
//!
//! The update is checked and installed from Rust; the plugin's own JS
//! commands are not granted to the webview, so the IPC surface stays the two
//! commands below. Bundles are verified against the minisign public key in
//! `app/updater.pub` (and must carry the version they were signed for). Until
//! that key exists, checks report `notConfigured`.

use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Public half of the release signing key (minisign, base64 as Tauri expects).
pub const PUBKEY: &str = include_str!("../updater.pub");

pub fn configured() -> bool {
    !PUBKEY.trim().is_empty()
}

/// The update found by the last check, kept for `update_install`.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    /// Release notes as plain text (rendered as text, never HTML).
    pub notes: Option<String>,
    /// RFC 3339 publish date, when the manifest has one.
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// This build has no update key (dev builds, or before signing was set up).
    NotConfigured,
    UpToDate {
        current: String,
    },
    Available(UpdateInfo),
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateProgress {
    /// Bytes so far and the total when known (capped at u32 for the UI).
    Downloading { done: u32, total: Option<u32> },
    /// Download verified; installing and restarting.
    Installing,
}

fn current() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[tauri::command]
#[specta::specta]
pub async fn update_check(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<UpdateStatus, String> {
    if !configured() {
        return Ok(UpdateStatus::NotConfigured);
    }
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| {
        tracing::warn!("update check failed: {e}");
        format!("Couldn't check for updates: {e}")
    })?;
    let status = match &found {
        None => UpdateStatus::UpToDate { current: current() },
        Some(u) => {
            tracing::info!(version = %u.version, "update available");
            UpdateStatus::Available(UpdateInfo {
                version: u.version.clone(),
                current: current(),
                notes: u.body.clone(),
                date: u.date.and_then(|d| d.format(&time::format_description::well_known::Rfc3339).ok()),
            })
        }
    };
    *pending.0.lock().unwrap() = found;
    Ok(status)
}

/// Downloads, verifies and installs the update from the last check, then
/// restarts the app. Returns an error (and leaves the app running) on failure.
#[tauri::command]
#[specta::specta]
pub async fn update_install(app: AppHandle, on_progress: Channel<UpdateProgress>) -> Result<(), String> {
    let update = app
        .state::<PendingUpdate>()
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "No update to install. Check again first.".to_owned())?;
    tracing::info!(version = %update.version, "installing update");
    let mut done: u64 = 0;
    let progress = on_progress.clone();
    update
        .download_and_install(
            |chunk, total| {
                done += chunk as u64;
                let cap = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
                let _ = progress.send(UpdateProgress::Downloading { done: cap(done), total: total.map(cap) });
            },
            || {
                let _ = on_progress.send(UpdateProgress::Installing);
            },
        )
        .await
        .map_err(|e| {
            tracing::warn!("update install failed: {e}");
            format!("Update failed: {e}")
        })?;
    app.restart();
}

#[cfg(test)]
mod tests {
    /// The Tauri CLI signs bundles against the config's key and the app
    /// verifies with `updater.pub`: they must be the same key.
    #[test]
    fn pubkey_matches_config() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["plugins"]["updater"]["pubkey"].as_str().unwrap(), super::PUBKEY.trim());
    }
}
