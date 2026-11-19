//! Signed interface patches (D-183). CI publishes the built frontend as a
//! signed pack (rolling `ui-latest` release) when main has interface-only
//! changes since its version's release. The app downloads and verifies it,
//! and on the next start serves it in place of the interface built into the
//! binary. It only fits the exact app version and command interface it was
//! built for (see `nightgrid_core::uipack`).
//!
//! Safety nets, all ending in the bundled interface:
//! - the pack is verified with the release key on every start;
//! - the setting can turn patches off (mirrored to a file read at start-up);
//! - the interface must report in (`ui_patch_ready`) within a minute, or the
//!   pack is set aside and the window reloads with the bundled interface;
//! - a pack that took two starts without reporting in is never used again.
//!
//! The pack only supplies web assets under the app's own origin. The CSP,
//! capabilities and command allow-list are compiled into the app, and the
//! CSP hashes always come from the bundled interface, so a pack can't widen
//! what the page may do.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nightgrid_core::http::{self, Request};
use nightgrid_core::uipack::{self, BootState, Meta, Pack, Target};
use serde::Serialize;
use specta::Type;
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
use tauri::{Assets, Manager, Runtime, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::update_cmd::PUBKEY;

/// Same origin as the app's own updates (rule 7): this repository's releases.
const BASE: &str = "https://github.com/TheRoben-Dev/nightgrid/releases/download/ui-latest/";
/// How long the interface has to report in before the bundled one takes over.
const WATCHDOG: Duration = Duration::from_secs(60);

/// The typed command interface this app was built with.
const BINDINGS: &str = include_str!("../../ui/src/lib/bindings.ts");

fn api_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| uipack::api_hash(BINDINGS))
}

fn bundled_built() -> u64 {
    env!("NIGHTGRID_UI_BUILT").parse().unwrap_or(0)
}

fn target() -> Target<'static> {
    Target { app_version: env!("CARGO_PKG_VERSION"), api_hash: api_hash(), bundled_built: bundled_built() }
}

/// What the running interface came from.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UiPackInfo {
    /// Unix time of the commit it was built from.
    pub built: u32,
    pub commit: String,
    pub notes: Vec<String>,
}

fn info(p: &Pack) -> UiPackInfo {
    UiPackInfo { built: u32::try_from(p.built).unwrap_or(u32::MAX), commit: p.commit.clone(), notes: p.notes.clone() }
}

/// Decided once at start-up, before the window loads anything.
struct Active {
    info: UiPackInfo,
    built: u64,
    files: BTreeMap<String, Vec<u8>>,
}

/// Shared between the asset provider and the commands.
pub struct UiPatches {
    data_dir: Option<PathBuf>,
    active: Option<Active>,
    /// False once the pack is set aside this run (the watchdog fired).
    serving: AtomicBool,
    ready: AtomicBool,
    /// Why the stored pack wasn't used this start, if there was one.
    skipped: Option<String>,
    busy: tokio::sync::Mutex<()>,
    last: Mutex<(Option<String>, Option<String>)>,
}

impl UiPatches {
    /// Reads the stored pack (if any) before the app is built. `identifier`
    /// gives the data folder the same way Tauri's `app_data_dir` does.
    pub fn load(identifier: &str) -> Self {
        let data_dir = dirs::data_dir().map(|d| d.join(identifier));
        let mut me = Self {
            data_dir: data_dir.clone(),
            active: None,
            serving: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            skipped: None,
            busy: tokio::sync::Mutex::new(()),
            last: Mutex::new((None, None)),
        };
        // Dev builds load the UI from Vite; a build without a key can't verify.
        if cfg!(debug_assertions) || !crate::update_cmd::configured() {
            return me;
        }
        let Some(dir) = data_dir else { return me };
        match choose(&dir) {
            Ok(Some(a)) => {
                me.active = Some(a);
                me.serving.store(true, Ordering::SeqCst);
            }
            Ok(None) => {}
            Err(e) => me.skipped = Some(e),
        }
        me
    }

    fn serving(&self) -> Option<&Active> {
        self.active.as_ref().filter(|_| self.serving.load(Ordering::SeqCst))
    }
}

/// The stored pack to serve this start, if any.
fn choose(dir: &Path) -> Result<Option<Active>, String> {
    if uipack::is_off(dir) {
        return Ok(None);
    }
    let Some(pack) = uipack::load_stored(dir, PUBKEY, &target())? else { return Ok(None) };
    let mut boot = BootState::load(dir);
    let go = boot.begin(pack.built);
    let _ = boot.save(dir);
    if !go {
        return Err(boot.last_error.unwrap_or_else(|| "this patch failed before".into()));
    }
    let files = uipack::unpack(&pack)?;
    Ok(Some(Active { info: info(&pack), built: pack.built, files }))
}

/// Stand-in while the bundled assets are moved into [`UiAssets`].
pub struct NoAssets;

impl<R: Runtime> Assets<R> for NoAssets {
    fn get(&self, _: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }
    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }
    fn csp_hashes(&self, _: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

/// Serves the pack's files while it's in use, the bundled ones otherwise.
pub struct UiAssets<R: Runtime> {
    bundled: Box<dyn Assets<R>>,
    patches: Arc<UiPatches>,
}

impl<R: Runtime> UiAssets<R> {
    pub fn new(bundled: Box<dyn Assets<R>>, patches: Arc<UiPatches>) -> Self {
        Self { bundled, patches }
    }
}

impl<R: Runtime> Assets<R> for UiAssets<R> {
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        match self.patches.serving() {
            // Only the pack's own files: never a mix of two builds.
            Some(a) => a.files.get(key.as_ref()).map(|b| Cow::Borrowed(b.as_slice())),
            None => self.bundled.get(key),
        }
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        self.bundled.iter()
    }

    // Always the bundled interface's: a pack can't add inline scripts or styles.
    fn csp_hashes(&self, html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        self.bundled.csp_hashes(html_path)
    }
}

/// After start-up: log the choice, keep the setting's file in step, and arm
/// the watchdog when a pack is in use.
pub fn start<R: Runtime>(app: &tauri::AppHandle<R>, patches: &Arc<UiPatches>) {
    if let Some(e) = &patches.skipped {
        tracing::warn!("interface patch not used: {e}");
    }
    if let Some(a) = &patches.active {
        tracing::info!(built = a.built, commit = %a.info.commit, "interface patch in use");
        let app = app.clone();
        let patches = patches.clone();
        std::thread::spawn(move || {
            std::thread::sleep(WATCHDOG);
            if patches.ready.load(Ordering::SeqCst) {
                return;
            }
            tracing::warn!("interface patch didn't start within {}s; using the built-in interface", WATCHDOG.as_secs());
            set_aside(&patches, "the interface didn't start with this patch");
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.reload();
            }
        });
    }
}

fn set_aside(patches: &UiPatches, why: &str) {
    patches.serving.store(false, Ordering::SeqCst);
    if let (Some(dir), Some(a)) = (&patches.data_dir, &patches.active) {
        let mut boot = BootState::load(dir);
        boot.fail(a.built, why);
        let _ = boot.save(dir);
    }
}

/// Mirrors the `uiPatches` setting to the file the next start reads.
pub fn sync_setting(data_dir: &Path, on: bool) {
    if let Err(e) = uipack::set_off(data_dir, !on) {
        tracing::warn!("couldn't save the interface patch setting: {e}");
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UiPatchStatus {
    /// The patch serving the interface right now (None = built-in interface).
    pub running: Option<UiPackInfo>,
    /// A downloaded patch that applies on the next start.
    pub waiting: Option<UiPackInfo>,
    /// Commit time of the interface built into this app.
    pub bundled_built: u32,
    /// Why a downloaded patch isn't in use, when it isn't.
    pub skipped: Option<String>,
    /// When patches were last checked this run (RFC 3339).
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// The check just downloaded a newer patch.
    pub updated: bool,
    /// This build has no release key, so patches can't be verified.
    pub not_configured: bool,
}

fn status(state: &AppState, patches: &UiPatches, updated: bool) -> UiPatchStatus {
    let running = patches.serving().map(|a| a.info.clone());
    let stored = uipack::load_stored(&state.data_dir, PUBKEY, &target()).ok().flatten();
    let bad = BootState::load(&state.data_dir).bad;
    let waiting = stored
        .filter(|p| running.as_ref().map_or(true, |r| u64::from(r.built) != p.built) && !bad.contains(&p.built))
        .map(|p| info(&p));
    let (checked_at, error) = patches.last.lock().unwrap().clone();
    UiPatchStatus {
        running,
        waiting,
        bundled_built: u32::try_from(bundled_built()).unwrap_or(u32::MAX),
        skipped: patches.skipped.clone(),
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
        404 => Err("no interface patch has been published yet".into()),
        s => Err(format!("GitHub answered {s}")),
    }
}

/// Downloads and stores a newer fitting pack; Ok(false) when there's none.
async fn fetch(state: &AppState) -> Result<bool, String> {
    let meta: Meta = serde_json::from_slice(&get(uipack::META_FILE, 64 * 1024, 15).await?)
        .map_err(|e| format!("unexpected patch info: {e}"))?;
    let t = target();
    let have = uipack::load_stored(&state.data_dir, PUBKEY, &t).ok().flatten().map(|p| p.built);
    let bad = BootState::load(&state.data_dir).bad;
    if !uipack::wanted(&meta, &t, have, &bad) {
        return Ok(false);
    }
    let sig = String::from_utf8(get(uipack::SIG_FILE, 64 * 1024, 15).await?)
        .map_err(|_| "signature isn't text".to_owned())?;
    let gz = get(uipack::PACK_FILE, uipack::MAX_DOWNLOAD, 180).await?;
    // The meta is only a hint: what counts is the signed pack.
    let pack = uipack::open(PUBKEY, &gz, &sig, &t)?;
    if have.is_some_and(|h| pack.built <= h) || bad.contains(&pack.built) {
        return Ok(false);
    }
    uipack::unpack(&pack)?;
    uipack::store(&state.data_dir, &gz, &sig).map_err(|e| format!("couldn't save the patch: {e}"))?;
    tracing::info!(built = pack.built, commit = %pack.commit, "interface patch downloaded");
    Ok(true)
}

#[tauri::command]
#[specta::specta]
pub fn ui_patch_status(state: State<'_, AppState>, patches: State<'_, Arc<UiPatches>>) -> UiPatchStatus {
    status(&state, &patches, false)
}

/// The interface is up: a patch in use counts as working.
#[tauri::command]
#[specta::specta]
pub fn ui_patch_ready(patches: State<'_, Arc<UiPatches>>) {
    patches.ready.store(true, Ordering::SeqCst);
    if let (Some(dir), Some(a)) = (&patches.data_dir, patches.serving()) {
        let mut boot = BootState::load(dir);
        boot.ready(a.built);
        let _ = boot.save(dir);
    }
}

/// Looks for a newer interface patch for this exact app.
#[tauri::command]
#[specta::specta]
pub async fn ui_patch_check(
    state: State<'_, AppState>,
    patches: State<'_, Arc<UiPatches>>,
) -> CmdResult<UiPatchStatus> {
    if !crate::update_cmd::configured() {
        return Ok(status(&state, &patches, false));
    }
    let Ok(_guard) = patches.busy.try_lock() else {
        return Err(CommandError::msg("A patch check is already running."));
    };
    let res = fetch(&state).await;
    let now = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).ok();
    let (updated, error) = match res {
        Ok(u) => (u, None),
        Err(e) => {
            tracing::warn!("interface patch check failed: {e}");
            (false, Some(e))
        }
    };
    *patches.last.lock().unwrap() = (now, error);
    Ok(status(&state, &patches, updated))
}

/// Deletes the downloaded patch: the next start uses the built-in interface.
#[tauri::command]
#[specta::specta]
pub fn ui_patch_remove(state: State<'_, AppState>, patches: State<'_, Arc<UiPatches>>) -> CmdResult<UiPatchStatus> {
    uipack::remove(&state.data_dir).map_err(|e| CommandError::msg(format!("Couldn't remove the patch: {e}")))?;
    Ok(status(&state, &patches, false))
}

/// Restarts the app so a downloaded patch (or its removal) takes effect.
#[tauri::command]
#[specta::specta]
pub fn ui_patch_restart(app: tauri::AppHandle) {
    crate::helper_cmd::shutdown(&app);
    app.restart();
}

#[cfg(test)]
mod tests {
    #[test]
    fn pins_to_this_build() {
        let t = super::target();
        assert_eq!(t.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(t.api_hash.len(), 64);
    }
}
