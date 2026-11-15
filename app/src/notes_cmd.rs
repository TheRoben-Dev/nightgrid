//! Notes and runbooks (M10e): CRUD over the local store. Secret-looking text
//! is refused by `nightgrid_core::notes::normalize_note`.

use nightgrid_core::notes::{Note, NoteId, NoteInput};
use tauri::State;

use crate::commands::CmdResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn notes_list(state: State<'_, AppState>) -> CmdResult<Vec<Note>> {
    Ok(state.store().list_notes()?)
}

/// Creates (`id` = null) or updates a note.
#[tauri::command]
#[specta::specta]
pub fn note_save(state: State<'_, AppState>, id: Option<NoteId>, input: NoteInput) -> CmdResult<Note> {
    let mut store = state.store();
    Ok(match id {
        Some(id) => store.update_note(id, &input)?,
        None => store.create_note(&input)?,
    })
}

#[tauri::command]
#[specta::specta]
pub fn note_delete(state: State<'_, AppState>, id: NoteId) -> CmdResult<()> {
    Ok(state.store().delete_note(id)?)
}
