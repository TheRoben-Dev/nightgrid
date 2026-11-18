//! Library content updates (D-174): fetch the signed content pack that CI
//! publishes to the rolling `kb-latest` release, verify it with the release
//! key, and keep it in the data folder. The UI prefers it over the bundled
//! Library when it's newer; without it (offline, turned off, nothing newer)
//! the bundled copy is used as before.

use std::collections::BTreeMap;
use std::time::Duration;

use nightgrid_core::http::{self, Request};
use nightgrid_core::kbpack::{self, KbCatalogEntry, Meta, Pack};
use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::update_cmd::PUBKEY;

/// Same origin as the app's own updates (rule 7): this repository's releases.
const BASE: &str = "https://github.com/TheRoben-Dev/nightgrid/releases/download/kb-latest/";

#[derive(Default)]
pub struct KbPacks {
    busy: tokio::sync::Mutex<()>,
    last: std::sync::Mutex<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KbPackInfo {
    /// Unix time of the content's commit.
    pub version: u32,
    pub min_app: String,
    pub commit: String,
    pub articles: u32,
    pub figures: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KbPackStatus {
    /// The stored pack, when there is one and it verifies.
    pub installed: Option<KbPackInfo>,
    /// When content was last checked this run (RFC 3339).
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// The check just stored a newer pack.
    pub updated: bool,
    /// This build has no release key, so packs can't be verified.
    pub not_configured: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KbPackContent {
    pub version: u32,
    pub catalog: Vec<KbCatalogEntry>,
    /// `shelf/slug.md` and `figures/name.svg` → text (empty unless asked for).
    pub files: BTreeMap<String, String>,
}

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn info(p: &Pack) -> KbPackInfo {
    let n = |v: usize| u32::try_from(v).unwrap_or(u32::MAX);
    KbPackInfo {
        version: p.version,
        min_app: p.min_app.clone(),
        commit: p.commit.clone(),
        articles: n(p.articles()),
        figures: n(p.figures()),
    }
}

fn stored(state: &AppState) -> Option<Pack> {
    if !crate::update_cmd::configured() {
        return None;
    }
    kbpack::load_stored(&state.data_dir, PUBKEY, app_version())
}

fn status(state: &AppState, packs: &KbPacks, updated: bool) -> KbPackStatus {
    let (checked_at, error) = packs.last.lock().unwrap().clone();
    KbPackStatus {
        installed: stored(state).as_ref().map(info),
        checked_at,
        error,
        updated,
        not_configured: !crate::update_cmd::configured(),
    }
}

async fn get(name: &str, max: usize, secs: u64) -> Result<Vec<u8>, String> {
    let res =
        http::send_following(Request::get(format!("{BASE}{name}")).max_body(max).timeout(Duration::from_secs(secs)), 5)
            .await?;
    match res.status {
        200 => Ok(res.body),
        404 => Err("no Library content has been published yet".into()),
        s => Err(format!("GitHub answered {s}")),
    }
}

/// Downloads and stores a newer pack; Ok(false) when there's nothing newer.
async fn fetch(state: &AppState, bundled: u32) -> Result<bool, String> {
    let meta: Meta = serde_json::from_slice(&get(kbpack::META_FILE, 64 * 1024, 15).await?)
        .map_err(|e| format!("unexpected content info: {e}"))?;
    let have = stored(state).map(|p| p.version);
    if !kbpack::wanted(&meta, have, bundled, app_version()) {
        return Ok(false);
    }
    let sig = String::from_utf8(get(kbpack::SIG_FILE, 64 * 1024, 15).await?)
        .map_err(|_| "signature isn't text".to_owned())?;
    let gz = get(kbpack::PACK_FILE, kbpack::MAX_DOWNLOAD, 120).await?;
    // The meta is only a hint: the version that counts is the signed one.
    let pack = kbpack::open(PUBKEY, &gz, &sig, app_version())?;
    if pack.version <= bundled || have.is_some_and(|v| pack.version <= v) {
        return Ok(false);
    }
    kbpack::store(&state.data_dir, &gz, &sig).map_err(|e| format!("couldn't save the content: {e}"))?;
    tracing::info!(version = pack.version, articles = pack.articles(), "Library content updated");
    Ok(true)
}

#[tauri::command]
#[specta::specta]
pub fn kb_pack_status(state: State<'_, AppState>, packs: State<'_, KbPacks>) -> KbPackStatus {
    status(&state, &packs, false)
}

/// Looks for newer Library content. `bundled` is the version built into this
/// app's UI, so a pack older than the app's own copy is never fetched.
#[tauri::command]
#[specta::specta]
pub async fn kb_pack_check(
    state: State<'_, AppState>,
    packs: State<'_, KbPacks>,
    bundled: u32,
) -> CmdResult<KbPackStatus> {
    if !crate::update_cmd::configured() {
        return Ok(status(&state, &packs, false));
    }
    let Ok(_guard) = packs.busy.try_lock() else {
        return Err(CommandError::msg("A content check is already running."));
    };
    let res = fetch(&state, bundled).await;
    let now = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).ok();
    let (updated, error) = match res {
        Ok(u) => (u, None),
        Err(e) => {
            tracing::warn!("Library content check failed: {e}");
            (false, Some(e))
        }
    };
    *packs.last.lock().unwrap() = (now, error);
    Ok(status(&state, &packs, updated))
}

/// The stored pack's catalog, and its files when `files` is true.
#[tauri::command]
#[specta::specta]
pub fn kb_pack_load(state: State<'_, AppState>, files: bool) -> Option<KbPackContent> {
    let p = stored(&state)?;
    Some(KbPackContent { version: p.version, catalog: p.catalog, files: if files { p.files } else { BTreeMap::new() } })
}

/// Deletes the stored pack: the Library goes back to the copy bundled with the app.
#[tauri::command]
#[specta::specta]
pub fn kb_pack_remove(state: State<'_, AppState>, packs: State<'_, KbPacks>) -> CmdResult<KbPackStatus> {
    kbpack::remove(&state.data_dir).map_err(|e| CommandError::msg(format!("Couldn't remove the content: {e}")))?;
    Ok(status(&state, &packs, false))
}
