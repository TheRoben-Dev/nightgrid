//! Filling the app's history from a node (nightgrid-node app-integration
//! specs 0001 and 0002): metric tiers, disk usage, uptime hours and the
//! node's timeline with its alerts. Cursors live in the app (`node_sync`),
//! pages are applied before their cursor is saved, and every merge is
//! idempotent, so an interrupted sync simply runs again.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::json;

use super::client::{NodeClient, NodeError};
use super::store::{CURSOR_DISK, CURSOR_EVENTS, CURSOR_UPTIME};
use super::{app_alert, timeline_kind, DiskPage, EventsPage, MetricsPage, NodeEvent, UptimePage};
use crate::alerts::Alert;
use crate::hosts::HostId;
use crate::metrics::history::{HOURLY, HOURLY_DAYS, MINUTE, ROLLUP};
use crate::store::Store;

/// What one sync brought in.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// Minute points and rollups stored.
    pub points: u32,
    pub disk_rows: u32,
    pub hours: u32,
    /// Timeline entries added (ones the app already had are skipped).
    pub events: u32,
    /// The node's history started over (new node id), so everything was synced afresh.
    pub reset: bool,
    /// Some node events were pruned before the app fetched them.
    pub pruned: bool,
    /// Alerts from the node's events, the latest per key, for the app's
    /// tracker. Not serialized: the app imports them.
    #[serde(skip)]
    pub alerts: Vec<Alert>,
}

fn store_err(e: impl std::fmt::Display) -> NodeError {
    NodeError::Bad(format!("couldn't save what the node sent: {e}"))
}

fn lock(store: &Mutex<Store>) -> std::sync::MutexGuard<'_, Store> {
    store.lock().expect("poisoned")
}

/// Everything the node offers, in the order the specs give.
pub async fn all(client: &NodeClient, store: &Mutex<Store>, host: HostId, now: i64) -> Result<SyncReport, NodeError> {
    let w = client.welcome().clone();
    let mut r =
        SyncReport { reset: lock(store).node_seen(host, &w.node.node_id).map_err(store_err)?, ..Default::default() };
    if w.cap("metrics.sync") >= 1 {
        metrics(client, store, host, now, &mut r).await?;
    }
    if w.cap("disk") >= 1 {
        disk(client, store, host, &mut r).await?;
    }
    if w.cap("uptime") >= 1 {
        uptime(client, store, host, &mut r).await?;
    }
    if w.cap("events") >= 1 {
        events(client, store, host, &mut r).await?;
    }
    Ok(r)
}

async fn page(client: &NodeClient, res: i64, after: i64) -> Result<MetricsPage, NodeError> {
    client.call("metrics.sync", json!({ "res": res, "after": after, "limit": 5000 })).await
}

/// Metric history: minute points from the cursor on; when the app was away
/// longer than the node keeps minutes, the older span comes from the
/// node's rollups (only buckets that end before its minutes begin, so no
/// minute counts twice).
pub async fn metrics(
    client: &NodeClient,
    store: &Mutex<Store>,
    host: HostId,
    now: i64,
    r: &mut SyncReport,
) -> Result<(), NodeError> {
    let start = now - HOURLY_DAYS * 86_400;
    let c60 = lock(store).node_cursor(host, MINUTE).map_err(store_err)?.unwrap_or(start);
    let mut p = page(client, MINUTE, c60).await?;
    if let Some(oldest) = p.oldest.filter(|o| c60 < o - MINUTE) {
        for res in [HOURLY, ROLLUP] {
            let bound = oldest.div_euclid(res) * res;
            let mut c = lock(store).node_cursor(host, res).map_err(store_err)?.unwrap_or(start);
            while c < bound {
                let q = page(client, res, c).await?;
                {
                    let mut s = lock(store);
                    for pt in q.points.iter().filter(|pt| pt.point.ts as i64 + res <= bound) {
                        s.merge_rollup_point(host, res, &pt.point, pt.n).map_err(store_err)?;
                        r.points += 1;
                    }
                }
                let next = q.next.max(c).min(bound);
                lock(store).node_cursor_set(host, res, next).map_err(store_err)?;
                if !q.more || next <= c {
                    break;
                }
                c = next;
            }
        }
    }
    let mut cursor = c60;
    loop {
        for pt in &p.points {
            // One point per lock, so the UI's own queries get in between.
            lock(store).insert_metric_point(host, &pt.point).map_err(store_err)?;
            r.points += 1;
        }
        let next = p.next.max(cursor);
        lock(store).node_cursor_set(host, MINUTE, next).map_err(store_err)?;
        if !p.more || next <= cursor {
            return Ok(());
        }
        cursor = next;
        p = page(client, MINUTE, cursor).await?;
    }
}

async fn cursor_of(store: &Mutex<Store>, host: HostId, kind: i64) -> Result<i64, NodeError> {
    Ok(lock(store).node_cursor(host, kind).map_err(store_err)?.unwrap_or(0))
}

pub async fn disk(
    client: &NodeClient,
    store: &Mutex<Store>,
    host: HostId,
    r: &mut SyncReport,
) -> Result<(), NodeError> {
    let mut after = cursor_of(store, host, CURSOR_DISK).await?;
    loop {
        let p: DiskPage = client.call("disk.sync", json!({ "after": after })).await?;
        let next = p.next.max(after);
        {
            let s = lock(store);
            r.disk_rows += s.disk_usage_merge(host, &p.rows).map_err(store_err)? as u32;
            s.node_cursor_set(host, CURSOR_DISK, next).map_err(store_err)?;
        }
        if !p.more || next <= after {
            return Ok(());
        }
        after = next;
    }
}

pub async fn uptime(
    client: &NodeClient,
    store: &Mutex<Store>,
    host: HostId,
    r: &mut SyncReport,
) -> Result<(), NodeError> {
    let mut after = cursor_of(store, host, CURSOR_UPTIME).await?;
    loop {
        let p: UptimePage = client.call("uptime.sync", json!({ "after": after })).await?;
        let next = p.next.max(after);
        {
            let mut s = lock(store);
            s.uptime_merge(host, &p.hours).map_err(store_err)?;
            s.node_cursor_set(host, CURSOR_UPTIME, next).map_err(store_err)?;
        }
        r.hours += p.hours.len() as u32;
        if !p.more || next <= after {
            return Ok(());
        }
        after = next;
    }
}

/// The node's timeline from the cursor on.
pub async fn events(
    client: &NodeClient,
    store: &Mutex<Store>,
    host: HostId,
    r: &mut SyncReport,
) -> Result<(), NodeError> {
    let mut after = cursor_of(store, host, CURSOR_EVENTS).await?;
    let mut latest: HashMap<String, Alert> = HashMap::new();
    loop {
        let p: EventsPage = client.call("events.sync", json!({ "after": after })).await?;
        if p.oldest.is_some_and(|o| o > after + 1) && after > 0 {
            r.pruned = true;
        }
        let next = p.next.max(after);
        {
            let s = lock(store);
            for e in &p.events {
                r.events += apply_event(&s, host, e, &mut latest).map_err(store_err)? as u32;
            }
            s.node_cursor_set(host, CURSOR_EVENTS, next).map_err(store_err)?;
        }
        if !p.more || next <= after {
            break;
        }
        after = next;
    }
    r.alerts.extend(latest.into_values());
    Ok(())
}

/// Stores one node event in the host timeline (and an outage it reports),
/// and keeps its alert as the latest for its key. Returns whether a
/// timeline entry was added. Used for synced pages and for pushed events.
pub fn apply_event(s: &Store, host: HostId, e: &NodeEvent, alerts: &mut HashMap<String, Alert>) -> crate::Result<bool> {
    if let Some(a) = e.alert.as_ref().and_then(|a| app_alert(a, host)) {
        alerts.insert(a.key.clone(), a);
    }
    if let Some(o) = e.outage.as_ref().filter(|o| o.state == "down") {
        s.outage_merge(host, o.start, o.end, &e.title)?;
    }
    let Some((kind, level)) = timeline_kind(e) else {
        return Ok(false);
    };
    let (window, any_title) = match kind {
        crate::timeline::EventKind::Boot => (120, true),
        _ => (90, false),
    };
    s.event_merge(host, e.at, kind, level, &e.title, &e.detail, window, any_title)
}

/// The latest event `seq` stored, for advancing the cursor from a pushed event.
pub fn note_seq(s: &Store, host: HostId, seq: i64) -> crate::Result<()> {
    let cur = s.node_cursor(host, CURSOR_EVENTS)?.unwrap_or(0);
    if seq > cur {
        s.node_cursor_set(host, CURSOR_EVENTS, seq)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};

    use super::*;
    use crate::node::client::fake::{serve, welcome};

    /// A node holding minutes from `m0` (older ones aged out) and rollups.
    fn node(now: i64, m0: i64) -> impl Fn(&str, &Value) -> Result<Value, (String, String)> {
        move |m, p| {
            let after = p["after"].as_i64().unwrap_or(0);
            match m {
                "hello" => Ok(welcome(2, &["metrics.sync", "events", "uptime", "disk"])),
                "metrics.sync" => {
                    let res = p["res"].as_i64().unwrap();
                    let first = if res == 60 { m0 } else { m0 - 3 * 86_400 };
                    let mut pts = Vec::new();
                    let mut ts = (first.div_euclid(res)) * res;
                    while ts + res <= now && pts.len() < 3000 {
                        if ts > after {
                            pts.push(json!({ "ts": ts, "cpu": 10.0, "n": if res == 60 { 6 } else { res / 60 } }));
                        }
                        ts += res;
                    }
                    let next = pts.last().map_or(after, |p| p["ts"].as_i64().unwrap());
                    let more = pts.len() == 3000;
                    Ok(
                        json!({ "res": res, "points": pts, "next": next, "more": more, "oldest": first.div_euclid(res) * res }),
                    )
                }
                "disk.sync" if after == 0 => Ok(
                    json!({ "rows": [{ "mount": "/srv", "ts": now - 7200, "used": 10, "size": 100 }], "next": now - 7200, "more": false }),
                ),
                "disk.sync" => Ok(json!({ "rows": [], "next": after, "more": false })),
                "uptime.sync" if after == 0 => Ok(
                    json!({ "hours": [{ "hour": now - 7200, "up": 3600, "down": 0, "asleep": 0 }], "next": now - 7200, "more": false }),
                ),
                "uptime.sync" => Ok(json!({ "hours": [], "next": after, "more": false })),
                "events.sync" if after == 0 => Ok(json!({ "events": [
                    { "seq": 1, "at": now - 500, "kind": "alert", "level": "warn", "title": "DISK FULL · server01", "detail": "91%",
                      "alert": { "id": 1, "key": "disk:/srv", "kind": "diskFull", "severity": "warn", "title": "server01", "detail": "91%", "since": now - 500, "resolved": null } },
                    { "seq": 2, "at": now - 400, "kind": "outage", "level": "critical", "title": "Down", "detail": "",
                      "outage": { "start": now - 900, "end": now - 400, "state": "down" } },
                    { "seq": 3, "at": now - 300, "kind": "login", "level": "info", "title": "alice logged in", "detail": "" },
                    { "seq": 4, "at": now - 200, "kind": "recovered", "level": "ok", "title": "DISK FULL · server01", "detail": "",
                      "alert": { "id": 1, "key": "disk:/srv", "kind": "diskFull", "severity": "warn", "title": "server01", "detail": "91%", "since": now - 500, "resolved": now - 200 } }
                ], "next": 4, "more": false, "oldest": 1 })),
                "events.sync" => Ok(json!({ "events": [], "next": after, "more": false, "oldest": 1 })),
                _ => Err(("unknown_method".into(), m.into())),
            }
        }
    }

    #[tokio::test]
    async fn first_sync_fills_minutes_rollups_and_the_rest_then_resumes() {
        let now = 100 * 86_400;
        let m0 = now - 2 * 86_400; // the node keeps two days of minutes
        let (s, h) = crate::node::store::tests::store();
        let store = Arc::new(Mutex::new(s));
        let (c, _) = NodeClient::connect(serve(node(now, m0), vec![]), "t").await.unwrap();
        let r = all(&c, &store, h, now).await.unwrap();
        assert!(!r.reset);
        assert!(r.points > 2 * 1440, "{}", r.points);
        assert_eq!((r.disk_rows, r.hours, r.events), (1, 1, 3), "login events aren't on the timeline yet");
        assert_eq!(r.alerts.len(), 1);
        assert_eq!(r.alerts[0].key, format!("disk:{h}:/srv"));
        assert_eq!(r.alerts[0].resolved, Some(now - 200));
        {
            let s = lock(&store);
            // Hourly rollups before the minutes began came from the node, whole.
            let older = s.metric_history_at(h, HOURLY, m0 - 3 * 86_400, m0 - 3600).unwrap();
            assert_eq!(older.len(), 3 * 24);
            let n: i64 = s
                .conn()
                .query_row(
                    "SELECT n FROM metric_points WHERE host_id = ?1 AND res = 3600 AND ts = ?2",
                    [h as i64, m0 - 7200],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 60);
            assert_eq!(s.metric_history_at(h, MINUTE, m0, now).unwrap().len(), 2 * 1440);
            assert_eq!(s.outages(None, 0, now).unwrap().len(), 1);
        }
        // Again: nothing new.
        let r = all(&c, &store, h, now).await.unwrap();
        assert_eq!((r.points, r.disk_rows, r.hours, r.events), (0, 0, 0, 0));
    }

    #[tokio::test]
    async fn a_node_without_capabilities_is_left_alone() {
        let (s, h) = crate::node::store::tests::store();
        let store = Mutex::new(s);
        let io = serve(
            |m, _| match m {
                "hello" => Ok(welcome(1, &["node"])),
                _ => Err(("unknown_method".into(), m.into())),
            },
            vec![],
        );
        let (c, _) = NodeClient::connect(io, "t").await.unwrap();
        assert_eq!(all(&c, &store, h, 1_000_000).await.unwrap(), SyncReport::default());
    }
}
