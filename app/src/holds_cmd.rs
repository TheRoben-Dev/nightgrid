//! Update holds (D-195): what the user keeps out of updates. Package holds
//! on the host itself (apt-mark, dnf versionlock, hidden Windows updates)
//! run through `system_cmd::system_package_hold`; these keep Nightgrid's
//! own list (hosts, packages where the host can't hold them, images and
//! stacks).

use nightgrid_core::holds::{HoldId, HoldInput, UpdateHold};
use tauri::State;

use crate::commands::CmdResult;
use crate::state::AppState;

/// Every hold still in force (ended snoozes are dropped).
#[tauri::command]
#[specta::specta]
pub fn update_holds_list(state: State<'_, AppState>) -> CmdResult<Vec<UpdateHold>> {
    Ok(state.store().update_holds()?)
}

/// Adds a hold, or changes the one for the same thing.
#[tauri::command]
#[specta::specta]
pub fn update_hold_save(state: State<'_, AppState>, input: HoldInput) -> CmdResult<UpdateHold> {
    tracing::info!(host = %input.host_id, kind = ?input.kind, "update hold");
    Ok(state.store().save_update_hold(&input)?)
}

/// Releases a hold.
#[tauri::command]
#[specta::specta]
pub fn update_hold_delete(state: State<'_, AppState>, id: HoldId) -> CmdResult<()> {
    tracing::info!(%id, "update hold released");
    Ok(state.store().delete_update_hold(id)?)
}
