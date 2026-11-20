//! Starter blueprints and runbooks (D-177). A starter is built into an
//! ordinary blueprint for one host, then planned and applied by
//! `blueprint_cmd` like a blueprint file. See `core/src/starters.rs`.

use nightgrid_core::blueprint::BlueprintSummary;
use nightgrid_core::hosts::HostId;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::starters::{self, StarterCatalog, StarterValue};
use nightgrid_core::system::SystemError;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::system_cmd::SysResult;
use crate::term_cmd::connect_posix;

fn other(e: impl std::fmt::Display) -> crate::system_cmd::SystemCmdError {
    SystemError::other(e.to_string()).into()
}

/// The bundled starters.
#[tauri::command]
#[specta::specta]
pub fn starters_list() -> StarterCatalog {
    starters::bundled()
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StarterTarget {
    /// Suggested folder for the starter's stacks.
    pub parent: String,
    /// Where the suggestion came from, in plain words.
    pub why: String,
}

/// Suggests where a starter's stacks go on `host_id`: its first stack
/// folder, else `~/stacks` (the login user can create that without sudo).
#[tauri::command]
#[specta::specta]
pub async fn starter_target(app: AppHandle, host_id: HostId) -> SysResult<StarterTarget> {
    let dirs = app.state::<AppState>().store().list_stack_dirs(host_id).unwrap_or_default();
    if let Some(d) = dirs.iter().find(|d| starters::valid_parent(&d.path)) {
        return Ok(StarterTarget { parent: d.path.clone(), why: "this host's stack folder".into() });
    }
    let conn = connect_posix(&app, host_id, "blueprints").await?;
    let (_, out, _) = conn.exec("printf '%s' \"$HOME\"").await.map_err(other)?;
    let home = String::from_utf8_lossy(&out).trim().to_owned();
    if !starters::valid_parent(&home) {
        return Err(other("couldn't read the login user's home folder; type a folder instead"));
    }
    let rest = dirs.iter().find_map(|d| d.path.strip_prefix('~')).map(|r| r.trim_start_matches('/'));
    match rest {
        Some(r) if !r.is_empty() => Ok(StarterTarget {
            parent: format!("{}/{r}", home.trim_end_matches('/')),
            why: "this host's stack folder".into(),
        }),
        _ => Ok(StarterTarget {
            parent: format!("{}/stacks", home.trim_end_matches('/')),
            why: "a folder in the login user's home, created if needed".into(),
        }),
    }
}

/// Builds starter `id` with the user's values into the opened blueprint,
/// ready for `blueprint_plan` on `host_id`. Nothing is written to the host.
#[tauri::command]
#[specta::specta]
pub fn starter_open(id: String, values: Vec<StarterValue>, parent: String) -> SysResult<BlueprintSummary> {
    let catalog = starters::bundled();
    let s = catalog.blueprints.iter().find(|s| s.id == id).ok_or_else(|| other("no such starter"))?;
    let bp = starters::build(s, &values, &parent, now_unix()).map_err(other)?;
    let summary = bp.summary();
    crate::blueprint_cmd::set_opened(bp);
    Ok(summary)
}
