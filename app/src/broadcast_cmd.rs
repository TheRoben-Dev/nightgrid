//! Transmissions (D-202): fetch the signed notice feed CI publishes to the
//! rolling `notices-latest` release, verify it with the release key and keep
//! it in the data folder, so notices still show offline and after a restart.
//! A feed older than the stored one is refused (no replaying a withdrawn
//! notice); anything that doesn't verify is never shown.

use std::time::Duration;

use nightgrid_core::broadcast::{self, Broadcast, Feed};
use nightgrid_core::http::{self, Request};
use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::update_cmd::PUBKEY;

/// Same origin as the app's own updates (rule 7): this repository's releases.
const BASE: &str = "https://github.com/TheRoben-Dev/nightgrid/releases/download/notices-latest/";

#[derive(Default)]
pub struct Broadcasts {
    busy: tokio::sync::Mutex<()>,
    last: std::sync::Mutex<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastStatus {
    /// Transmissions meant for this app now (system, version, dates), most
    /// urgent first. Dismissed ones are included; the UI filters them.
    pub notices: Vec<Broadcast>,
    /// When the stored feed was published (unix seconds), 0 when there's none.
    pub version: f64,
    /// When the feed was last checked this run (RFC 3339).
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// This build has no release key, so feeds can't be verified.
    pub not_configured: bool,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn stored(state: &AppState) -> Option<Feed> {
    if !crate::update_cmd::configured() {
        return None;
    }
    broadcast::load_stored(&state.data_dir, PUBKEY)
}

fn status(state: &AppState, b: &Broadcasts) -> BroadcastStatus {
    let (checked_at, error) = b.last.lock().unwrap().clone();
    let feed = stored(state);
    BroadcastStatus {
        notices: feed
            .as_ref()
            .map(|f| broadcast::current(f, env!("CARGO_PKG_VERSION"), std::env::consts::OS, now()))
            .unwrap_or_default(),
        version: feed.map_or(0.0, |f| f.version as f64),
        checked_at,
        error,
        not_configured: !crate::update_cmd::configured(),
    }
}

async fn get(name: &str, max: usize) -> Result<Option<Vec<u8>>, String> {
    let res =
        http::send_following(Request::get(format!("{BASE}{name}")).max_body(max).timeout(Duration::from_secs(15)), 5)
            .await?;
    match res.status {
        200 => Ok(Some(res.body)),
        404 => Ok(None),
        s => Err(format!("GitHub answered {s}")),
    }
}

/// Downloads, verifies and stores the feed.
async fn fetch(state: &AppState) -> Result<(), String> {
    let Some(json) = get(broadcast::FEED_FILE, broadcast::MAX_FEED).await? else {
        // Nothing published yet: nothing to show.
        return Ok(());
    };
    let sig = get(broadcast::SIG_FILE, 64 * 1024).await?.ok_or("the notice feed has no signature")?;
    let sig = String::from_utf8(sig).map_err(|_| "signature isn't text".to_owned())?;
    let feed = broadcast::open(PUBKEY, &json, &sig)?;
    if let Some(have) = stored(state) {
        if feed.version < have.version {
            return Err("the published notices are older than the ones already here".into());
        }
        if feed.version == have.version {
            return Ok(());
        }
    }
    broadcast::store(&state.data_dir, &json, &sig).map_err(|e| format!("couldn't save the notices: {e}"))?;
    tracing::info!(version = feed.version, notices = feed.notices.len(), "notice feed updated");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn broadcast_status(state: State<'_, AppState>, b: State<'_, Broadcasts>) -> BroadcastStatus {
    status(&state, &b)
}

/// Looks for new transmissions.
#[tauri::command]
#[specta::specta]
pub async fn broadcast_check(state: State<'_, AppState>, b: State<'_, Broadcasts>) -> CmdResult<BroadcastStatus> {
    if !crate::update_cmd::configured() {
        return Ok(status(&state, &b));
    }
    let Ok(_guard) = b.busy.try_lock() else {
        return Err(CommandError::msg("A notice check is already running."));
    };
    let res = fetch(&state).await;
    let at = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).ok();
    let error = res.err().inspect(|e| tracing::warn!("notice check failed: {e}"));
    *b.last.lock().unwrap() = (at, error);
    Ok(status(&state, &b))
}
