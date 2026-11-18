//! Background chores of the 1.1 features, run from the alert loop's tick
//! (every 15 s) whether alert rules are on or not. Each one decides for
//! itself whether anything is due and never blocks the loop: real work is
//! spawned. Features turned off in Settings → Features do nothing here.

use nightgrid_core::hosts::HostId;
use nightgrid_core::settings::AppSettings;
use tauri::{AppHandle, Manager as _};

use crate::state::AppState;

pub(crate) fn tick(app: &AppHandle, cfg: &AppSettings, now: i64) {
    let connected: Vec<HostId> = app.state::<AppState>().pool.connected();
    crate::posture_cmd::due(app, cfg, &connected, now);
    crate::secrets_cmd::due(app, cfg, now);
    crate::drills_cmd::due(app, cfg, &connected, now);
    crate::capacity_cmd::due(app, cfg, &connected, now);
}
