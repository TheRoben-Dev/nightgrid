//! Incident replay (1.1, D-173). See `core/src/replay.rs`.

use nightgrid_core::depgraph::Node;
use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::ChangeQuery;
use nightgrid_core::metrics::history::{HOURLY, ROLLUP};
use nightgrid_core::replay::{self, Incident, Replay};
use nightgrid_core::services::check::now_unix;
use tauri::State;

use crate::commands::CmdResult;
use crate::state::AppState;

const DAY: i64 = 86_400;

/// Outages and alert bursts of the last `days` (at most 90), newest first.
#[tauri::command]
#[specta::specta]
pub fn replay_incidents(state: State<'_, AppState>, days: u32) -> CmdResult<Vec<Incident>> {
    let now = now_unix();
    let since = now - i64::from(days.clamp(1, 90)) * DAY;
    let store = state.store();
    let hosts = store.list_hosts()?;
    let services = store.list_services()?;
    let name = |n: &Node| match n {
        Node::Host(h) => hosts.iter().find(|x| x.id == *h).map_or_else(|| format!("host {h}"), |x| x.name.clone()),
        Node::Service(s) => {
            services.iter().find(|x| x.id == *s).map_or_else(|| format!("service {s}"), |x| x.name.clone())
        }
    };
    let host_of = |n: &Node| match n {
        Node::Host(h) => Some(*h),
        Node::Service(s) => services.iter().find(|x| x.id == *s).and_then(|x| x.host_id),
    };
    let mut all = replay::from_outages(&store.outages(None, since, now)?, name, host_of);
    for h in &hosts {
        all.extend(replay::from_alerts(h.id, &h.name, &store.events_for(h.id, since)?));
    }
    Ok(replay::sort_incidents(all, 300))
}

/// Everything recorded around one incident.
#[tauri::command]
#[specta::specta]
pub fn replay_load(state: State<'_, AppState>, incident: Incident) -> CmdResult<Replay> {
    let (from, to) = replay::window(&incident);
    let store = state.store();
    let host: Option<HostId> = incident.host_id;
    let metrics = match host {
        Some(h) => {
            let mut m = store.metric_history(h, from, to)?;
            // Minute points are kept for a short while; fall back to coarser ones.
            for res in [ROLLUP, HOURLY] {
                if m.is_empty() {
                    m = store.metric_history_at(h, res, from, to)?;
                }
            }
            m
        }
        None => Vec::new(),
    };
    let host_events = match host {
        Some(h) => store.events_for(h, from)?,
        None => Vec::new(),
    };
    let changes = store.journal_query(&ChangeQuery {
        host_id: host,
        since: Some(from),
        until: Some(to),
        limit: 500,
        ..Default::default()
    })?;
    let hosts = store.list_hosts()?;
    let services = store.list_services()?;
    let outages: Vec<_> = store
        .outages(None, from - replay::MAX_WINDOW, to)?
        .into_iter()
        .map(|o| {
            let n = match o.node {
                Node::Host(h) => {
                    hosts.iter().find(|x| x.id == h).map_or_else(|| format!("host {h}"), |x| x.name.clone())
                }
                Node::Service(s) => {
                    services.iter().find(|x| x.id == s).map_or_else(|| format!("service {s}"), |x| x.name.clone())
                }
            };
            (o, n)
        })
        .collect();
    let notices = store.notices(1000)?;
    let events = replay::events(host, from, to, &host_events, &changes, &outages, &notices);
    Ok(Replay { incident, from, to, metrics, events })
}
