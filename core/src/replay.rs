//! Incident replay (1.1, D-173): step through an outage or a burst of
//! alerts minute by minute. Everything comes from what Nightgrid already
//! recorded: uptime outages, the host timeline (alerts, restarts, OOM
//! kills, deploys), the journal of changes, notices and the metric history.
//! Logs for the window are read from the host on request (log search).

use serde::{Deserialize, Serialize};

use crate::depgraph::Node;
use crate::hosts::HostId;
use crate::journal::ChangeEntry;
use crate::metrics::history::Point;
use crate::notify::Notice;
use crate::timeline::{EventKind, HostEvent, Level};
use crate::uptime::Outage;

/// Alerts closer together than this are one incident.
pub const CLUSTER_GAP: i64 = 30 * 60;
/// Context before and after an incident.
pub const PAD: i64 = 30 * 60;
/// Longest window replayed (a longer outage shows its first and last part).
pub const MAX_WINDOW: i64 = 24 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum IncidentKind {
    /// A host or service was down (uptime).
    Outage,
    /// Alerts on one host close together.
    Alerts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Incident {
    pub kind: IncidentKind,
    pub host_id: Option<HostId>,
    pub service_id: Option<i32>,
    pub title: String,
    pub detail: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub start: i64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub end: i64,
    pub level: Level,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReplaySource {
    Event,
    Change,
    Outage,
    Notice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReplayEvent {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    pub source: ReplaySource,
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub host_id: Option<HostId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Replay {
    pub incident: Incident,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub from: i64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub to: i64,
    /// The host's metrics over the window (minute points when recent).
    pub metrics: Vec<Point>,
    /// Oldest first.
    pub events: Vec<ReplayEvent>,
}

/// Outages as incidents. `names` gives a node's display name.
pub fn from_outages(
    outages: &[Outage],
    name: impl Fn(&Node) -> String,
    host_of: impl Fn(&Node) -> Option<HostId>,
) -> Vec<Incident> {
    outages
        .iter()
        .map(|o| {
            let (service_id, what) = match o.node {
                Node::Host(_) => (None, "host"),
                Node::Service(s) => (Some(s), "service"),
            };
            Incident {
                kind: IncidentKind::Outage,
                host_id: host_of(&o.node),
                service_id,
                title: format!("{} down", name(&o.node)),
                detail: if o.detail.is_empty() { format!("The {what} didn't answer.") } else { o.detail.clone() },
                start: o.start,
                end: o.end.max(o.start),
                level: Level::Critical,
            }
        })
        .collect()
}

/// Clusters a host's alert events (newest first or any order) into
/// incidents: alerts less than [`CLUSTER_GAP`] apart are one, ending at the
/// last recovery inside the cluster or its last alert.
pub fn from_alerts(host: HostId, host_name: &str, events: &[HostEvent]) -> Vec<Incident> {
    let mut ev: Vec<&HostEvent> =
        events.iter().filter(|e| matches!(e.kind, EventKind::Alert | EventKind::Recovered)).collect();
    ev.sort_by_key(|e| e.at);
    let mut out: Vec<Incident> = Vec::new();
    let mut cur: Option<(Incident, Vec<String>)> = None;
    for e in ev {
        match (&mut cur, e.kind) {
            (Some((inc, titles)), _) if e.at - inc.end <= CLUSTER_GAP => {
                inc.end = e.at;
                if e.kind == EventKind::Alert {
                    if e.level == Level::Critical {
                        inc.level = Level::Critical;
                    }
                    if !titles.contains(&e.title) {
                        titles.push(e.title.clone());
                    }
                }
            }
            (_, EventKind::Alert) => {
                if let Some((inc, titles)) = cur.take() {
                    out.push(finish(inc, titles));
                }
                let inc = Incident {
                    kind: IncidentKind::Alerts,
                    host_id: Some(host),
                    service_id: None,
                    title: String::new(),
                    detail: String::new(),
                    start: e.at,
                    end: e.at,
                    level: if e.level == Level::Critical { Level::Critical } else { Level::Warn },
                };
                cur = Some((inc, vec![e.title.clone()]));
            }
            // A recovery with no alert before it in range.
            _ => {}
        }
    }
    if let Some((inc, titles)) = cur.take() {
        out.push(finish(inc, titles));
    }
    for i in &mut out {
        i.title = format!("{host_name}: {}", i.title);
    }
    out
}

fn finish(mut inc: Incident, titles: Vec<String>) -> Incident {
    inc.title = match titles.len() {
        1 => titles[0].clone(),
        n => format!("{} and {} more", titles[0], n - 1),
    };
    inc.detail = titles.join("; ");
    inc
}

/// Newest first, at most `limit`.
pub fn sort_incidents(mut v: Vec<Incident>, limit: usize) -> Vec<Incident> {
    v.sort_by_key(|i| std::cmp::Reverse(i.start));
    v.truncate(limit);
    v
}

/// The window replayed for an incident.
pub fn window(i: &Incident) -> (i64, i64) {
    let from = i.start - PAD;
    let to = (i.end + PAD).min(from + MAX_WINDOW);
    (from, to)
}

fn level_of_notice(n: &Notice) -> Level {
    match n.level {
        crate::notify::NoticeLevel::Info => Level::Info,
        crate::notify::NoticeLevel::Ok => Level::Ok,
        crate::notify::NoticeLevel::Warn => Level::Warn,
        crate::notify::NoticeLevel::Critical => Level::Critical,
    }
}

/// Puts everything recorded in the window on one line, oldest first.
pub fn events(
    host: Option<HostId>,
    from: i64,
    to: i64,
    host_events: &[HostEvent],
    changes: &[ChangeEntry],
    outages: &[(Outage, String)],
    notices: &[Notice],
) -> Vec<ReplayEvent> {
    let inside = |t: i64| t >= from && t <= to;
    let mut out: Vec<ReplayEvent> = Vec::new();
    out.extend(host_events.iter().filter(|e| inside(e.at)).map(|e| ReplayEvent {
        at: e.at,
        source: ReplaySource::Event,
        level: e.level,
        title: e.title.clone(),
        detail: e.detail.clone(),
        host_id: host,
    }));
    out.extend(changes.iter().filter(|c| inside(c.at)).map(|c| ReplayEvent {
        at: c.at,
        source: ReplaySource::Change,
        level: if c.ok { Level::Info } else { Level::Warn },
        title: c.title.clone(),
        detail: c.detail.clone(),
        host_id: c.host_id,
    }));
    for (o, name) in outages.iter().filter(|(o, _)| o.end >= from && o.start <= to) {
        if inside(o.start) {
            out.push(ReplayEvent {
                at: o.start,
                source: ReplaySource::Outage,
                level: Level::Critical,
                title: format!("{name} went down"),
                detail: o.detail.clone(),
                host_id: None,
            });
        }
        if inside(o.end) && o.end > o.start {
            out.push(ReplayEvent {
                at: o.end,
                source: ReplaySource::Outage,
                level: Level::Ok,
                title: format!("{name} last seen down"),
                detail: String::new(),
                host_id: None,
            });
        }
    }
    out.extend(
        notices.iter().filter(|n| inside(n.at) && (host.is_none() || n.host_id.is_none() || n.host_id == host)).map(
            |n| ReplayEvent {
                at: n.at,
                source: ReplaySource::Notice,
                level: level_of_notice(n),
                title: n.title.clone(),
                detail: n.body.clone(),
                host_id: n.host_id,
            },
        ),
    );
    out.sort_by_key(|e| e.at);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Origin;

    fn ev(at: i64, kind: EventKind, level: Level, title: &str) -> HostEvent {
        HostEvent {
            at,
            kind,
            level,
            title: title.into(),
            detail: String::new(),
            origin: Origin::Nightgrid,
            silenced: None,
        }
    }

    #[test]
    fn clusters_alerts() {
        let e = vec![
            ev(1000, EventKind::Alert, Level::Warn, "CPU HIGH"),
            ev(1600, EventKind::Alert, Level::Critical, "MEMORY"),
            ev(2000, EventKind::Recovered, Level::Ok, "CPU HIGH"),
            ev(9000, EventKind::Alert, Level::Warn, "DISK"),
            ev(8000, EventKind::Recovered, Level::Ok, "stray"),
        ];
        let inc = from_alerts(1, "server01", &e);
        assert_eq!(inc.len(), 2);
        assert_eq!((inc[0].start, inc[0].end, inc[0].level), (1000, 2000, Level::Critical));
        assert_eq!(inc[0].title, "server01: CPU HIGH and 1 more");
        assert_eq!(inc[1].title, "server01: DISK");
        let (f, t) = window(&inc[0]);
        assert_eq!((f, t), (1000 - PAD, 2000 + PAD));
    }

    #[test]
    fn merges_the_window() {
        let o = Outage { id: 1, node: Node::Service(3), start: 500, end: 900, detail: "timeout".into() };
        let incs = from_outages(std::slice::from_ref(&o), |_| "Jellyfin".into(), |_| Some(1));
        assert_eq!((incs[0].title.as_str(), incs[0].service_id, incs[0].host_id), ("Jellyfin down", Some(3), Some(1)));
        let hev = vec![
            ev(450, EventKind::Container, Level::Warn, "jellyfin died"),
            ev(5000, EventKind::Boot, Level::Info, "late"),
        ];
        let r = events(Some(1), 0, 1000, &hev, &[], &[(o, "Jellyfin".into())], &[]);
        let titles: Vec<&str> = r.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["jellyfin died", "Jellyfin went down", "Jellyfin last seen down"]);
    }
}
