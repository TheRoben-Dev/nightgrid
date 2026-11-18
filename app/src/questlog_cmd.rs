//! The quest log (1.3, D-175): Library guides started as quests, with the
//! steps done so far. The UI works out the current step from the guide text;
//! this only keeps the record (local, part of the profile export).

use nightgrid_core::hosts::HostId;
use nightgrid_core::questlog::{Quest, QuestId};
use nightgrid_core::services::check::now_unix;
use tauri::State;

use crate::commands::CmdResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn quests_list(state: State<'_, AppState>) -> CmdResult<Vec<Quest>> {
    Ok(state.store().quests()?)
}

/// Starts a guide as a quest on a host (or none); an existing one is returned.
#[tauri::command]
#[specta::specta]
pub fn quest_start(
    state: State<'_, AppState>,
    article_id: String,
    host_id: Option<HostId>,
    title: String,
    ticks: Vec<String>,
) -> CmdResult<Quest> {
    let q = state.store().start_quest(&article_id, host_id, &title, &ticks, now_unix())?;
    tracing::info!(id = q.id, article = %q.article_id, "quest started");
    Ok(q)
}

/// Saves a quest's done steps; `complete` marks it finished.
#[tauri::command]
#[specta::specta]
pub fn quest_ticks(state: State<'_, AppState>, id: QuestId, ticks: Vec<String>, complete: bool) -> CmdResult<Quest> {
    Ok(state.store().set_quest_ticks(id, &ticks, complete, now_unix())?)
}

#[tauri::command]
#[specta::specta]
pub fn quest_abandon(state: State<'_, AppState>, id: QuestId) -> CmdResult<bool> {
    Ok(state.store().delete_quest(id)?)
}
