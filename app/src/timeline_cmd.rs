//! Per-host incident timeline (M10l): recorded events from the local database
//! merged with what the host's own logs and Docker remember.

use nightgrid_core::alerts::{Alert, Change, Severity};
use nightgrid_core::docker::DockerError;
use nightgrid_core::hosts::HostId;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::silence::Silence;
use nightgrid_core::system::SystemError;
use nightgrid_core::timeline::{self, EventKind, HostEvent, Level};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::system_cmd::SysResult;
use crate::term_cmd::connect_host;

/// Records one event for `host` now. Failures are logged, never raised: the
/// timeline must not break the action it describes.
pub(crate) fn record(app: &AppHandle, host: HostId, kind: EventKind, level: Level, title: &str, detail: &str) {
    let state = app.state::<AppState>();
    let r = state.store().event_add(host, now_unix(), kind, level, title, detail);
    if let Err(e) = r {
        tracing::warn!(host = %host, "couldn't record a timeline event: {e}");
    }
    crate::journal_cmd::from_timeline(app, host, kind, level, title, detail);
}

/// Alerts that fired or cleared on a host go on that host's timeline.
pub(crate) fn record_alerts(app: &AppHandle, changes: &[Change], silences: &[Silence], now: i64) {
    for c in changes {
        let (a, fired, worse): (&Alert, bool, bool) = match c {
            Change::Fired(a) => (a, true, false),
            Change::Worse(a) => (a, true, true),
            Change::Resolved(a) => (a, false, false),
            Change::Updated(_) => continue,
        };
        let Some(host) = a.host_id else { continue };
        let (kind, level) = if fired {
            (EventKind::Alert, if a.severity == Severity::Critical { Level::Critical } else { Level::Warn })
        } else {
            (EventKind::Recovered, Level::Ok)
        };
        let title = format!("{}: {}", a.kind.label(), a.title);
        let detail = if worse {
            format!("got worse: {}", a.detail)
        } else if fired {
            a.detail.clone()
        } else {
            format!("cleared after {}", timeline::span(a.resolved.unwrap_or(a.since) - a.since))
        };
        let state = app.state::<AppState>();
        let at = if worse {
            now
        } else if fired {
            a.since
        } else {
            a.resolved.unwrap_or_else(now_unix)
        };
        // Acknowledged or muted alerts stay on the timeline, marked (D-196).
        let marked = nightgrid_core::silence::covering(silences, a, now).map(|s| s.mode);
        let r = state.store().event_add_marked(host, at, kind, level, &title, &detail, marked);
        if let Err(e) = r {
            tracing::warn!(host = %host, "couldn't record an alert on the timeline: {e}");
        }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub events: Vec<HostEvent>,
    /// What couldn't be read (journal permissions, Docker, an offline host).
    pub notes: Vec<String>,
    /// Unix seconds the timeline starts at.
    #[specta(type = u32)]
    pub since: i64,
}

/// The host's timeline for the last `days` days (1–90): recorded events
/// always; the host's own history and Docker's when it answers.
#[tauri::command]
#[specta::specta]
pub async fn timeline_get(app: AppHandle, host_id: HostId, days: u32) -> SysResult<Timeline> {
    let days = days.clamp(1, 90) as i64;
    let now = now_unix();
    let since = now - days * 86_400;
    let recorded = app.state::<AppState>().store().events_for(host_id, since).map_err(SystemError::other)?;
    let mut notes = Vec::new();
    let mut read = Vec::new();
    match connect_host(&app, host_id).await {
        Err(e) => notes.push(format!("Only what Nightgrid recorded: the host didn't answer ({e}).")),
        Ok(conn) if conn.is_windows() => notes.push(
            "Only what Nightgrid recorded: reading a Windows host's own event logs here isn't available yet.".into(),
        ),
        Ok(conn) => {
            match conn.exec(&timeline::host_script(since)).await {
                Ok((_, out, _)) => {
                    let h = timeline::parse_host(&String::from_utf8_lossy(&out), since);
                    read.extend(h.events);
                    notes.extend(h.notes);
                }
                Err(e) => notes.push(format!("Couldn't read the host's logs: {e}")),
            }
            let state = app.state::<AppState>();
            match state.docker.get(host_id, conn).await {
                Ok(d) => match tokio::time::timeout(std::time::Duration::from_secs(20), d.events(since, now)).await {
                    Ok(Ok(raw)) => read.extend(timeline::container_events(raw)),
                    Ok(Err(e)) => notes.push(format!("Docker events: {e}")),
                    Err(_) => notes.push("Docker events took too long; container entries are missing.".into()),
                },
                Err(DockerError::NotInstalled) => {}
                Err(e) => notes.push(format!("Docker: {e}")),
            }
        }
    }
    Ok(Timeline { events: timeline::merge(recorded, read), notes, since })
}

/// Forgets what Nightgrid recorded for the host (the host's own logs stay).
#[tauri::command]
#[specta::specta]
pub fn timeline_clear(state: State<'_, AppState>, host_id: HostId) -> CmdResult<()> {
    state.store().events_clear(host_id)?;
    Ok(())
}
