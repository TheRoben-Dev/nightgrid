//! Library quests (M12e): a guide's `{check: …}` steps run against a host.
//! Fixed read-only commands from `core/src/quest.rs`, no sudo, one round trip.

use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::quest::{self, QuestResult};
use tauri::AppHandle;

use crate::commands::{CmdResult, CommandError};

/// Runs the checks on a host and judges each one.
#[tauri::command]
#[specta::specta]
pub async fn quest_check(app: AppHandle, host_id: HostId, checks: Vec<String>) -> CmdResult<Vec<QuestResult>> {
    let (planned, mut bad) = quest::plan(&checks);
    if planned.is_empty() {
        return Ok(bad);
    }
    let conn = crate::term_cmd::connect_host(&app, host_id).await.map_err(|e| CommandError::msg(e.to_string()))?;
    let run = tokio::time::timeout(Duration::from_secs(25), conn.exec(&quest::script(&planned))).await;
    let out = match run {
        Ok(Ok((_, out, _))) => String::from_utf8_lossy(&out).into_owned(),
        Ok(Err(e)) => return Err(CommandError::msg(e.to_string())),
        // Whatever finished in time still counts; the rest is "no answer".
        Err(_) => String::new(),
    };
    let mut res = quest::results(&planned, &out);
    res.append(&mut bad);
    tracing::info!(host = %host_id, checks = res.len(), "quest checks");
    Ok(res)
}
