//! Custom command widgets (M10x): read-only commands on your hosts shown as
//! dashboard tiles. The command is checked again before every run.

use std::time::Duration;

use nightgrid_core::cmdwidget::{self, CmdReading, CmdWidget, CmdWidgetId};
use nightgrid_core::system::SystemError;
use tauri::{AppHandle, Manager, State};

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::system_cmd::SysResult;
use crate::term_cmd::connect_host;

/// Longest a widget's command may run.
const RUN_TIMEOUT: Duration = Duration::from_secs(30);

fn other(e: impl std::fmt::Display) -> SystemError {
    SystemError::other(e)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_widgets_list(state: State<'_, AppState>) -> CmdResult<Vec<CmdWidget>> {
    Ok(state.store().cmd_widgets()?)
}

/// Adds (`id` 0) or updates a widget.
#[tauri::command]
#[specta::specta]
pub fn cmd_widget_save(state: State<'_, AppState>, widget: CmdWidget) -> CmdResult<CmdWidget> {
    let saved = state.store().save_cmd_widget(&widget)?;
    tracing::info!(id = saved.id, host = %saved.host_id, "command widget saved");
    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_widget_delete(state: State<'_, AppState>, id: CmdWidgetId) -> CmdResult<bool> {
    Ok(state.store().delete_cmd_widget(id)?)
}

async fn run(app: &AppHandle, w: &CmdWidget) -> SysResult<CmdReading> {
    cmdwidget::validate(w).map_err(other)?;
    let conn = connect_host(app, w.host_id).await?;
    let (code, out, err) = tokio::time::timeout(RUN_TIMEOUT, conn.exec(w.command.trim()))
        .await
        .map_err(|_| other("the command took longer than 30 s"))?
        .map_err(other)?;
    let now = nightgrid_core::services::check::now_unix();
    Ok(cmdwidget::read(w, code, &String::from_utf8_lossy(&out), &String::from_utf8_lossy(&err), now))
}

/// Runs a saved widget's command once.
#[tauri::command]
#[specta::specta]
pub async fn cmd_widget_run(app: AppHandle, id: CmdWidgetId) -> SysResult<CmdReading> {
    let w = {
        let state = app.state::<AppState>();
        let found = state.store().cmd_widget(id).map_err(other)?;
        found.ok_or_else(|| other("that widget was deleted"))?
    };
    run(&app, &w).await
}

/// Runs an unsaved widget once, to check the command and the parse rule.
#[tauri::command]
#[specta::specta]
pub async fn cmd_widget_try(app: AppHandle, widget: CmdWidget) -> SysResult<CmdReading> {
    run(&app, &widget).await
}
