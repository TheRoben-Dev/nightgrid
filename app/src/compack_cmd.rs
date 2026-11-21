//! Component packs (feature store part 2, D-207): fetch the signed packs CI
//! publishes to the rolling `components-latest` release for the components
//! a user has installed, keep them in the data folder, and serve their files
//! at `/components/<id>/<built>/…` next to the interface. Whether a pack is
//! used is the interface's call (`ui/src/lib/componentUi.ts`); without one
//! (offline, nothing newer, doesn't fit) the copy built into the app is used.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use nightgrid_core::compack::{self, Index, Pack};
use nightgrid_core::http::{self, Request};
use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::commands::{CmdResult, CommandError};
use crate::update_cmd::PUBKEY;

/// Same origin as the app's own updates (rule 7): this repository's releases.
const BASE: &str = "https://github.com/TheRoben-Dev/nightgrid/releases/download/components-latest/";

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

struct Served {
    info: ComponentPackInfo,
    files: BTreeMap<String, Vec<u8>>,
}

/// Shared between the asset provider and the commands.
pub struct ComponentPacks {
    data_dir: Option<PathBuf>,
    served: RwLock<BTreeMap<String, Arc<Served>>>,
    busy: tokio::sync::Mutex<()>,
    last: Mutex<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPackInfo {
    pub id: String,
    /// Unix time of the newest commit to the component's files.
    pub built: u32,
    pub commit: String,
    pub notes: Vec<String>,
    pub entry: String,
    pub style: bool,
    /// What it needs from the interface (JSON), checked by the interface.
    pub requires: String,
    pub bytes: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPackCheck {
    pub packs: Vec<ComponentPackInfo>,
    /// Components that got a newer pack just now.
    pub updated: Vec<String>,
    /// When packs were last checked this run (RFC 3339).
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// This build can't verify packs (no release key, or a dev build).
    pub not_configured: bool,
}

fn n32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

fn served(p: &Pack) -> Result<Served, String> {
    let files = compack::unpack(p)?;
    let bytes = files.values().map(Vec::len).sum::<usize>() as u64;
    Ok(Served {
        info: ComponentPackInfo {
            id: p.id.clone(),
            built: n32(p.built),
            commit: p.commit.clone(),
            notes: p.notes.clone(),
            entry: p.entry.clone(),
            style: p.style,
            requires: p.requires.to_string(),
            bytes: n32(bytes),
        },
        files,
    })
}

fn enabled() -> bool {
    !cfg!(debug_assertions) && crate::update_cmd::configured()
}

impl ComponentPacks {
    /// Reads every stored pack before the window loads. `identifier` gives
    /// the data folder the same way Tauri's `app_data_dir` does.
    pub fn load(identifier: &str) -> Self {
        let data_dir = dirs::data_dir().map(|d| d.join(identifier));
        let me = Self {
            data_dir: data_dir.clone(),
            served: RwLock::new(BTreeMap::new()),
            busy: tokio::sync::Mutex::new(()),
            last: Mutex::new((None, None)),
        };
        if !enabled() {
            return me;
        }
        let Some(dir) = data_dir else { return me };
        let mut map = BTreeMap::new();
        for c in nightgrid_core::components::COMPONENTS {
            match compack::load_stored(&dir, PUBKEY, c.id, app_version())
                .and_then(|p| p.map(|p| served(&p)).transpose())
            {
                Ok(Some(s)) => {
                    tracing::info!(component = c.id, built = s.info.built, "component pack stored");
                    map.insert(c.id.to_owned(), Arc::new(s));
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(component = c.id, "component pack not used: {e}"),
            }
        }
        *me.served.write().unwrap() = map;
        me
    }

    /// A served file (`/components/<id>/<built>/<file>`), when the stored
    /// pack of that component has exactly that build.
    pub fn file(&self, key: &str) -> Option<Vec<u8>> {
        let (id, built, file) = compack::served_path(key)?;
        let map = self.served.read().unwrap();
        let s = map.get(id)?;
        (u64::from(s.info.built) == built).then(|| s.files.get(file).cloned()).flatten()
    }

    fn list(&self) -> Vec<ComponentPackInfo> {
        self.served.read().unwrap().values().map(|s| s.info.clone()).collect()
    }

    fn have(&self, id: &str) -> Option<u64> {
        self.served.read().unwrap().get(id).map(|s| u64::from(s.info.built))
    }

    fn check_result(&self, updated: Vec<String>) -> ComponentPackCheck {
        let (checked_at, error) = self.last.lock().unwrap().clone();
        ComponentPackCheck { packs: self.list(), updated, checked_at, error, not_configured: !enabled() }
    }
}

async fn get(name: &str, max: usize, secs: u64) -> Result<Vec<u8>, String> {
    let res =
        http::send_following(Request::get(format!("{BASE}{name}")).max_body(max).timeout(Duration::from_secs(secs)), 5)
            .await?;
    match res.status {
        200 => Ok(res.body),
        404 => Err("no component patches have been published yet".into()),
        s => Err(format!("GitHub answered {s}")),
    }
}

/// Downloads newer packs for `ids`; returns the ones updated.
async fn fetch(packs: &ComponentPacks, ids: &[String]) -> Result<Vec<String>, String> {
    let Some(dir) = packs.data_dir.clone() else { return Err("no data folder".into()) };
    let index: Index = serde_json::from_slice(&get(compack::INDEX_FILE, 256 * 1024, 15).await?)
        .map_err(|e| format!("unexpected patch list: {e}"))?;
    if index.format != compack::FORMAT {
        return Ok(Vec::new());
    }
    let mut updated = Vec::new();
    for e in index.components.iter().filter(|e| ids.contains(&e.id)) {
        let have = packs.have(&e.id);
        if !compack::wanted(e, have, app_version()) {
            continue;
        }
        let sig = String::from_utf8(get(&compack::sig_file(&e.id), 64 * 1024, 15).await?)
            .map_err(|_| "signature isn't text".to_owned())?;
        let gz = get(&compack::pack_file(&e.id), compack::MAX_DOWNLOAD, 120).await?;
        // The index is only a hint: what counts is the signed pack.
        let pack = compack::open(PUBKEY, &e.id, &gz, &sig, app_version())?;
        if have.is_some_and(|h| pack.built <= h) {
            continue;
        }
        let s = served(&pack)?;
        compack::store(&dir, &e.id, &gz, &sig).map_err(|err| format!("couldn't save the {} patch: {err}", e.id))?;
        tracing::info!(component = %e.id, built = pack.built, "component pack updated");
        packs.served.write().unwrap().insert(e.id.clone(), Arc::new(s));
        updated.push(e.id.clone());
    }
    Ok(updated)
}

/// The stored packs.
#[tauri::command]
#[specta::specta]
pub fn component_packs(packs: State<'_, Arc<ComponentPacks>>) -> Vec<ComponentPackInfo> {
    packs.list()
}

/// Looks for newer packs of the given (installed) components and stores them.
#[tauri::command]
#[specta::specta]
pub async fn component_packs_check(
    packs: State<'_, Arc<ComponentPacks>>,
    ids: Vec<String>,
) -> CmdResult<ComponentPackCheck> {
    if !enabled() {
        return Ok(packs.check_result(Vec::new()));
    }
    let Ok(_guard) = packs.busy.try_lock() else {
        return Err(CommandError::msg("A patch check is already running."));
    };
    let res = fetch(&packs, &ids).await;
    let now = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).ok();
    let (updated, error) = match res {
        Ok(u) => (u, None),
        Err(e) => {
            tracing::warn!("component patch check failed: {e}");
            (Vec::new(), Some(e))
        }
    };
    *packs.last.lock().unwrap() = (now, error);
    Ok(packs.check_result(updated))
}

/// Deletes a component's stored pack (removing the component, or going back
/// to the built-in copy).
#[tauri::command]
#[specta::specta]
pub fn component_pack_remove(packs: State<'_, Arc<ComponentPacks>>, id: String) -> CmdResult<Vec<ComponentPackInfo>> {
    if let Some(dir) = &packs.data_dir {
        compack::remove(dir, &id).map_err(|e| CommandError::msg(format!("Couldn't remove the patch: {e}")))?;
    }
    packs.served.write().unwrap().remove(&id);
    Ok(packs.list())
}
