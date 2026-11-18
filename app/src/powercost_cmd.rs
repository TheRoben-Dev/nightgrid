//! Power & cost (M12d): estimates from power profiles, the hourly metric
//! tier and the UPS readings the alert loop already takes. See
//! `core/src/powercost.rs`. Nothing here connects to a host.

use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::ChangeArea;
use nightgrid_core::powercost::{self, PowerOverview, PowerProfile, UpsPower};
use nightgrid_core::services::check::now_unix;
use tauri::{AppHandle, State};

use crate::commands::CmdResult;
use crate::journal_cmd::{self, Change};
use crate::state::AppState;

/// Every host's estimate over the last `days` (1–90), with idle windows in
/// local time (`utc_offset_mins` east of UTC), and the UPSes' output.
#[tauri::command]
#[specta::specta]
pub async fn power_overview(state: State<'_, AppState>, days: u32, utc_offset_mins: i32) -> CmdResult<PowerOverview> {
    let days = days.clamp(1, 90);
    let offset = utc_offset_mins.clamp(-14 * 60, 14 * 60) * 60;
    let now = now_unix();
    let from = now - i64::from(days) * 86_400;
    let store = state.store();
    let profiles = store.power_profiles()?;
    let hosts = store.list_hosts()?;
    let mut out = Vec::with_capacity(hosts.len());
    for h in &hosts {
        let pts = store.metric_hourly(h.id, from, now)?;
        let p = profiles.iter().find(|p| p.host_id == h.id).cloned();
        out.push(powercost::estimate(h.id, &h.name, p, &pts, offset));
    }
    let mut ups = Vec::new();
    for w in store.ups_list()? {
        let members: Vec<HostId> = match w.group_id {
            Some(g) => hosts.iter().filter(|h| h.group_id == Some(g)).map(|h| h.id).collect(),
            None => vec![w.host_id],
        };
        let est: Vec<f32> = out.iter().filter(|e| members.contains(&e.host_id)).filter_map(|e| e.avg_w).collect();
        let reading = state.ups.reading(w.id);
        let (watts, measured) =
            reading.as_ref().and_then(|(r, _)| powercost::ups_watts(r)).map_or((None, false), |(w, m)| (Some(w), m));
        ups.push(UpsPower {
            id: w.id,
            label: w.label.clone(),
            watts,
            measured,
            age: reading.map_or(0, |(_, at)| (now - at).max(0) as u32),
            hosts: members,
            estimated_w: (!est.is_empty()).then(|| est.iter().sum()),
        });
    }
    Ok(PowerOverview { days, hosts: out, ups })
}

#[tauri::command]
#[specta::specta]
pub async fn power_profile_set(
    app: AppHandle,
    state: State<'_, AppState>,
    host_id: HostId,
    idle_w: f32,
    max_w: f32,
) -> CmdResult<()> {
    state.store().power_profile_set(&PowerProfile { host_id, idle_w, max_w })?;
    let title = format!("Power profile: {idle_w:.0}–{max_w:.0} W");
    journal_cmd::record(&app, Change::new(host_id, ChangeArea::Power, &title));
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn power_profile_clear(state: State<'_, AppState>, host_id: HostId) -> CmdResult<()> {
    state.store().power_profile_clear(host_id)?;
    Ok(())
}
