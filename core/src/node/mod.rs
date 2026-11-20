//! Nightgrid Node pairing (M12f, D-165): the app's side of the optional
//! agent from the nightgrid-node project. The app finds the node through the
//! SSH connection it already has, speaks its JSON-lines protocol
//! ([`client`]), fills its own history from the node's ([`sync`]) and
//! installs, updates or removes it ([`install`]). Every step is skipped on a
//! host without the node, with an older one, or with a capability switched
//! off; nothing in the app needs the node.
//!
//! The protocol is documented in nightgrid-node's `docs/PROTOCOL.md`. The
//! types here are the app's own copies of the parts it uses, read
//! tolerantly (unknown fields ignored, missing ones defaulted) so a newer
//! node never breaks an older app.

pub mod client;
pub mod hub;
pub mod install;
pub mod store;
pub mod sync;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::alerts::{Alert, AlertKind, Severity};
use crate::hosts::HostId;
use crate::metrics::history::Point;
use crate::timeline::{EventKind, Level};

/// The node's socket on the host.
pub const SOCKET: &str = "/run/nightgrid-node/node.sock";
/// The group whose members may use the socket.
pub const GROUP: &str = "nightgrid-node";
/// Protocol versions the app speaks: 1 (requests) and 2 (plus streams).
pub const PROTO_MIN: u32 = 1;
pub const PROTO_MAX: u32 = 2;

/// `hello` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Welcome {
    pub proto: u32,
    pub node: NodeIdentity,
    #[serde(default)]
    pub caps: BTreeMap<String, u32>,
}

impl Welcome {
    /// The version of a capability, 0 when the node doesn't list it.
    pub fn cap(&self, name: &str) -> u32 {
        self.caps.get(name).copied().unwrap_or(0)
    }

    pub fn streams(&self) -> bool {
        self.proto >= 2
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NodeIdentity {
    /// Random id made with the node's database; a new id means its history
    /// started over, so the app's sync cursors for the host are reset.
    pub node_id: String,
    pub version: String,
    pub hostname: String,
    pub arch: String,
}

/// `node.info` result (the parts the app shows).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NodeInfo {
    pub node: NodeIdentity,
    pub started: i64,
    pub interval_secs: u32,
    pub coverage: Vec<Coverage>,
    pub rss_bytes: u64,
    pub db_bytes: u64,
    /// Optional access root granted, by name: `ok`, `denied`, `absent`, `off`.
    pub access: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Coverage {
    pub res: i64,
    pub points: u64,
    pub oldest: Option<i64>,
    pub newest: Option<i64>,
}

/// A history point as the node sends it: the app's [`Point`] plus how many
/// samples (minute points) or minutes (rollups) it holds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NodePoint {
    #[serde(flatten)]
    pub point: Point,
    #[serde(default)]
    pub n: i64,
}

/// `metrics.sync` result.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MetricsPage {
    pub res: i64,
    pub points: Vec<NodePoint>,
    pub next: i64,
    pub more: bool,
    /// Oldest bucket this tier still holds.
    pub oldest: Option<i64>,
}

/// An alert as the node keeps it. `kind` and `severity` stay strings so a
/// kind the app doesn't know yet is skipped instead of failing the page.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NodeAlert {
    pub id: u32,
    pub key: String,
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub since: i64,
    pub resolved: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OutageSpan {
    pub start: i64,
    pub end: i64,
    /// `down` or `asleep`.
    pub state: String,
}

/// One entry of the node's timeline.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NodeEvent {
    pub seq: i64,
    pub at: i64,
    pub kind: String,
    pub level: String,
    pub title: String,
    pub detail: String,
    pub alert: Option<NodeAlert>,
    pub outage: Option<OutageSpan>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventsPage {
    pub events: Vec<NodeEvent>,
    pub next: i64,
    pub more: bool,
    /// Oldest `seq` still kept.
    pub oldest: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UptimeHour {
    pub hour: i64,
    pub up: i64,
    pub down: i64,
    pub asleep: i64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UptimePage {
    pub hours: Vec<UptimeHour>,
    pub next: i64,
    pub more: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiskRow {
    pub mount: String,
    pub ts: i64,
    pub used: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiskPage {
    pub rows: Vec<DiskRow>,
    pub next: i64,
    pub more: bool,
}

/// The app's key for a node alert key: node keys have no host segment, so
/// `disk:/srv` becomes `disk:<host>:/srv` (only the first `:` counts).
pub fn app_alert_key(key: &str, host: HostId) -> String {
    match key.split_once(':') {
        Some((source, rest)) => format!("{source}:{host}:{rest}"),
        None => format!("node:{host}:{key}"),
    }
}

/// A node alert as an app alert, or `None` for a kind the app doesn't know.
pub fn app_alert(a: &NodeAlert, host: HostId) -> Option<Alert> {
    let kind: AlertKind = serde_json::from_value(serde_json::Value::String(a.kind.clone())).ok()?;
    let severity = if a.severity == "critical" { Severity::Critical } else { Severity::Warn };
    Some(Alert {
        id: 0,
        key: app_alert_key(&a.key, host),
        kind,
        severity,
        title: a.title.clone(),
        detail: a.detail.clone(),
        host_id: Some(host),
        since: a.since,
        resolved: a.resolved,
        caused_by: None,
        silenced: None,
    })
}

fn level(s: &str) -> Level {
    match s {
        "ok" => Level::Ok,
        "warn" => Level::Warn,
        "critical" => Level::Critical,
        _ => Level::Info,
    }
}

/// Where a node event lands in the host timeline, or `None` for kinds the
/// timeline doesn't show yet (logins, file changes, the LAN, ports, jobs:
/// those come with later pairing work).
pub fn timeline_kind(e: &NodeEvent) -> Option<(EventKind, Level)> {
    let lv = level(&e.level);
    Some(match e.kind.as_str() {
        "alert" => (EventKind::Alert, lv),
        "recovered" => (EventKind::Recovered, Level::Ok),
        "boot" => (EventKind::Boot, lv),
        "outage" => match e.outage.as_ref().map(|o| o.state.as_str()) {
            Some("down") => (EventKind::Alert, Level::Critical),
            _ => (EventKind::Power, Level::Info),
        },
        "container" => (EventKind::Container, lv),
        "action" => (EventKind::Automation, lv),
        _ => return None,
    })
}

/// Splits `a.b.c` (an optional leading `v`, anything after `-` ignored)
/// into numbers for comparing versions.
pub fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// `a` is an older version than `b`.
pub fn older(a: &str, b: &str) -> bool {
    let (mut x, mut y) = (version_parts(a), version_parts(b));
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    x < y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_keys_get_the_host_after_the_first_colon() {
        assert_eq!(app_alert_key("disk:/srv", 3), "disk:3:/srv");
        assert_eq!(app_alert_key("fill:/mnt/a:b", 3), "fill:3:/mnt/a:b");
        assert_eq!(app_alert_key("docker:web", 12), "docker:12:web");
        assert_eq!(app_alert_key("odd", 1), "node:1:odd");
    }

    #[test]
    fn known_alert_kinds_map_and_unknown_ones_are_skipped() {
        let a = NodeAlert {
            id: 3,
            key: "disk:/srv".into(),
            kind: "diskFull".into(),
            severity: "critical".into(),
            title: "server01".into(),
            detail: "/srv is 97% full".into(),
            since: 100,
            resolved: None,
        };
        let m = app_alert(&a, 7).unwrap();
        assert_eq!(m.kind, AlertKind::DiskFull);
        assert_eq!(m.severity, Severity::Critical);
        assert_eq!(m.key, "disk:7:/srv");
        assert_eq!(m.host_id, Some(7));
        assert!(app_alert(&NodeAlert { kind: "loginBurst".into(), ..a }, 7).is_none());
    }

    #[test]
    fn events_map_to_timeline_kinds() {
        let ev = |kind: &str, state: Option<&str>| NodeEvent {
            kind: kind.into(),
            level: "warn".into(),
            outage: state.map(|s| OutageSpan { start: 1, end: 2, state: s.into() }),
            ..Default::default()
        };
        assert_eq!(timeline_kind(&ev("alert", None)), Some((EventKind::Alert, Level::Warn)));
        assert_eq!(timeline_kind(&ev("recovered", None)), Some((EventKind::Recovered, Level::Ok)));
        assert_eq!(timeline_kind(&ev("outage", Some("down"))), Some((EventKind::Alert, Level::Critical)));
        assert_eq!(timeline_kind(&ev("outage", Some("asleep"))), Some((EventKind::Power, Level::Info)));
        assert_eq!(timeline_kind(&ev("login", None)), None);
        assert_eq!(timeline_kind(&ev("whatever", None)), None);
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(older("0.9.0", "0.10.0"));
        assert!(older("v0.23.0", "0.23.1"));
        assert!(!older("0.23.0", "v0.23.0"));
        assert!(!older("1.0.0", "0.99.9"));
        assert!(older("0.23", "0.23.1"));
    }

    #[test]
    fn protocol_json_reads_tolerantly() {
        let w: Welcome = serde_json::from_str(
            r#"{"proto":2,"node":{"nodeId":"ab","version":"0.23.0","hostname":"server01","arch":"x86_64","new":1},
                "caps":{"metrics":1,"metrics.sync":1,"events":1}}"#,
        )
        .unwrap();
        assert_eq!(w.cap("metrics.sync"), 1);
        assert_eq!(w.cap("containers"), 0);
        assert!(w.streams());
        let p: MetricsPage = serde_json::from_str(
            r#"{"res":60,"points":[{"ts":120,"cpu":4.5,"mem":40,"n":6,"extra":true}],"next":120,"more":false,"oldest":60}"#,
        )
        .unwrap();
        assert_eq!(p.points[0].n, 6);
        assert_eq!(p.points[0].point.cpu, Some(4.5));
        assert_eq!(p.points[0].point.ts, 120.0);
        let e: EventsPage = serde_json::from_str(
            r#"{"events":[{"seq":41,"at":5,"kind":"alert","level":"warn","title":"DISK FULL · server01","detail":"x",
                "alert":{"id":3,"key":"disk:/srv","kind":"diskFull","severity":"warn","title":"server01","detail":"x","since":5,"resolved":null}}],
                "next":41,"more":false,"oldest":3}"#,
        )
        .unwrap();
        assert_eq!(e.events[0].alert.as_ref().unwrap().key, "disk:/srv");
    }
}
