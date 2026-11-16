//! Uptime history (M10t): how long each host and service was seen up, down
//! or asleep, per hour, and each outage from its first to its last "down".
//!
//! Only what Nightgrid observed counts: the alerts loop reports every host
//! and checked service each round, and the time since the previous round
//! (at most [`MAX_GAP`]) is added to the hour it falls in. When the app
//! wasn't running, nothing is recorded, so availability is "up out of
//! up + down", never "up out of the whole month". Sleeping (suspended, in a
//! sleep window, rebooting from Nightgrid) is kept apart and isn't downtime.

use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::depgraph::Node;
use crate::store::Store;
use crate::Result;

/// The longest gap between two rounds that still counts; a longer one
/// (the computer slept, the app was closed) is unknown time.
pub const MAX_GAP: i64 = 60;
/// Hours and outages older than this are dropped.
pub const KEEP_DAYS: i64 = 400;
const HOUR: i64 = 3600;
/// At most this many outages come back from one read.
pub const OUTAGES_MAX: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Up,
    Down,
    Asleep,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub node: Node,
    pub seen: Seen,
    /// Why it's down (kept on the outage, first round only).
    pub detail: String,
}

/// One day (local midnight to midnight) of one host or service.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UptimeDay {
    /// Local midnight, unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub day: i64,
    pub up: u32,
    pub down: u32,
    pub asleep: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UptimeSeries {
    pub node: Node,
    /// Days with anything recorded, oldest first (missing days: unknown).
    pub days: Vec<UptimeDay>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Outage {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub id: i64,
    pub node: Node,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub start: i64,
    /// The last round it was seen down (still down if this is recent).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub end: i64,
    pub detail: String,
}

/// Percent up out of the time seen up or down; `None` with nothing seen.
pub fn availability(up: u64, down: u64) -> Option<f64> {
    let t = up + down;
    (t > 0).then(|| up as f64 * 100.0 / t as f64)
}

fn parts(n: Node) -> (&'static str, i64) {
    match n {
        Node::Host(id) => ("host", i64::from(id)),
        Node::Service(id) => ("service", i64::from(id)),
    }
}

fn node(kind: &str, id: i64) -> Option<Node> {
    let id = i32::try_from(id).ok()?;
    match kind {
        "host" => Some(Node::Host(id)),
        "service" => Some(Node::Service(id)),
        _ => None,
    }
}

/// Feeds rounds of observations into the store and keeps outages open
/// between rounds. One per app; the outages it opened stay open only while
/// it runs (a restart starts a new outage if still down).
#[derive(Debug, Default)]
pub struct Recorder {
    last: Option<i64>,
    open: HashMap<Node, i64>,
}

impl Recorder {
    pub fn tick(&mut self, store: &mut Store, now: i64, obs: &[Observation]) -> Result<()> {
        let dt = match self.last {
            Some(t) if now > t => (now - t).min(MAX_GAP),
            _ => 0,
        };
        self.last = Some(now);
        let from = now - dt;
        let tx = store.conn_mut().transaction()?;
        let mut still_open = HashMap::new();
        for o in obs {
            let (kind, id) = parts(o.node);
            if dt > 0 {
                let col = match o.seen {
                    Seen::Up => "up",
                    Seen::Down => "down",
                    Seen::Asleep => "asleep",
                };
                let mut t = from;
                while t < now {
                    let hour = t.div_euclid(HOUR) * HOUR;
                    let end = (hour + HOUR).min(now);
                    tx.execute(
                        &format!(
                            "INSERT INTO uptime_hours (kind, subject, hour, {col}) VALUES (?1, ?2, ?3, ?4)
                             ON CONFLICT (kind, subject, hour) DO UPDATE SET {col} = {col} + excluded.{col}"
                        ),
                        params![kind, id, hour, end - t],
                    )?;
                    t = end;
                }
            }
            if o.seen == Seen::Down {
                if let Some(&row) = self.open.get(&o.node) {
                    tx.execute("UPDATE outages SET end = ?2 WHERE id = ?1", params![row, now])?;
                    still_open.insert(o.node, row);
                } else {
                    let detail: String = o.detail.chars().take(200).collect();
                    tx.execute(
                        "INSERT INTO outages (kind, subject, start, end, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![kind, id, from, now, detail],
                    )?;
                    still_open.insert(o.node, tx.last_insert_rowid());
                }
            }
        }
        tx.commit()?;
        // Up, asleep or no longer reported: the outage ended.
        self.open = still_open;
        Ok(())
    }

    /// Hosts and services that are down right now, by outage row.
    pub fn open(&self) -> &HashMap<Node, i64> {
        &self.open
    }
}

impl Store {
    /// Per-day totals for every host and service in `[from, to)`, days
    /// starting at local midnight (`offset_secs` east of UTC).
    pub fn uptime_days(&self, from: i64, to: i64, offset_secs: i64) -> Result<Vec<UptimeSeries>> {
        let mut st = self.conn().prepare(
            "SELECT kind, subject, ((hour + ?3) / 86400) * 86400 - ?3 AS day, SUM(up), SUM(down), SUM(asleep)
             FROM uptime_hours WHERE hour >= ?1 AND hour < ?2
             GROUP BY kind, subject, day ORDER BY kind, subject, day",
        )?;
        let rows = st.query_map(params![from, to, offset_secs], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                UptimeDay { day: r.get(2)?, up: r.get(3)?, down: r.get(4)?, asleep: r.get(5)? },
            ))
        })?;
        let mut out: Vec<UptimeSeries> = Vec::new();
        for row in rows {
            let (kind, id, day) = row?;
            let Some(n) = node(&kind, id) else { continue };
            match out.last_mut() {
                Some(s) if s.node == n => s.days.push(day),
                _ => out.push(UptimeSeries { node: n, days: vec![day] }),
            }
        }
        Ok(out)
    }

    /// Outages overlapping `[from, to]`, newest first; one host or service, or all.
    pub fn outages(&self, which: Option<Node>, from: i64, to: i64) -> Result<Vec<Outage>> {
        let (kind, id) = which.map_or(("", 0), parts);
        let mut st = self.conn().prepare(
            "SELECT id, kind, subject, start, end, detail FROM outages
             WHERE end >= ?1 AND start <= ?2 AND (?3 = '' OR (kind = ?3 AND subject = ?4))
             ORDER BY start DESC LIMIT ?5",
        )?;
        let rows = st.query_map(params![from, to, kind, id, OUTAGES_MAX as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (oid, kind, subject, start, end, detail) = row?;
            if let Some(n) = node(&kind, subject) {
                out.push(Outage { id: oid, node: n, start, end, detail });
            }
        }
        Ok(out)
    }

    /// Drops history older than [`KEEP_DAYS`] and rows of removed hosts and
    /// services. Returns rows removed.
    pub fn prune_uptime(&self, now: i64) -> Result<usize> {
        let cut = now - KEEP_DAYS * 86_400;
        let c = self.conn();
        let mut n = c.execute("DELETE FROM uptime_hours WHERE hour < ?1", [cut])?;
        n += c.execute("DELETE FROM outages WHERE end < ?1", [cut])?;
        for (table, kind, parent) in [
            ("uptime_hours", "host", "hosts"),
            ("uptime_hours", "service", "services"),
            ("outages", "host", "hosts"),
            ("outages", "service", "services"),
        ] {
            n += c.execute(
                &format!("DELETE FROM {table} WHERE kind = '{kind}' AND subject NOT IN (SELECT id FROM {parent})"),
                [],
            )?;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('a', 'a.example.com')", []).unwrap();
        s
    }
    fn o(node: Node, seen: Seen) -> Observation {
        Observation { node, seen, detail: "timed out".into() }
    }
    const H: Node = Node::Host(1);

    #[test]
    fn adds_time_between_rounds_split_by_hour() {
        let mut s = store();
        let mut r = Recorder::default();
        let t0 = 10 * 3600 - 20;
        r.tick(&mut s, t0, &[o(H, Seen::Up)]).unwrap(); // first round: nothing to add
        r.tick(&mut s, t0 + 30, &[o(H, Seen::Up)]).unwrap(); // 20 s in hour 9, 10 s in hour 10
        r.tick(&mut s, t0 + 30 + 600, &[o(H, Seen::Up)]).unwrap(); // gap capped at 60 s
        let days = s.uptime_days(0, 86_400, 0).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].node, H);
        assert_eq!(days[0].days[0].up, 90);
        let hours: Vec<(i64, i64)> = s
            .conn()
            .prepare("SELECT hour, up FROM uptime_hours ORDER BY hour")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(hours, vec![(9 * 3600, 20), (10 * 3600, 70)]);
    }

    #[test]
    fn outages_open_extend_and_close() {
        let mut s = store();
        let mut r = Recorder::default();
        r.tick(&mut s, 1000, &[o(H, Seen::Up)]).unwrap();
        r.tick(&mut s, 1015, &[o(H, Seen::Down)]).unwrap();
        r.tick(&mut s, 1030, &[o(H, Seen::Down)]).unwrap();
        assert_eq!(r.open().len(), 1);
        r.tick(&mut s, 1045, &[o(H, Seen::Up)]).unwrap();
        assert!(r.open().is_empty());
        r.tick(&mut s, 1060, &[o(H, Seen::Down)]).unwrap();
        r.tick(&mut s, 1075, &[]).unwrap(); // no longer reported: closed
        let out = s.outages(Some(H), 0, 5000).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!((out[1].start, out[1].end), (1000, 1030));
        assert_eq!(out[1].detail, "timed out");
        assert_eq!((out[0].start, out[0].end), (1045, 1060));
        let d = &s.uptime_days(0, 86_400, 0).unwrap()[0].days[0];
        assert_eq!((d.up, d.down), (15, 45));
        assert_eq!(s.outages(Some(Node::Service(9)), 0, 5000).unwrap().len(), 0);
        assert_eq!(s.outages(None, 2000, 5000).unwrap().len(), 0);
    }

    #[test]
    fn asleep_is_not_down_and_days_follow_the_offset() {
        let mut s = store();
        let mut r = Recorder::default();
        // 23:59:30 to 00:00:30 UTC; with +1 h both halves fall on the same local day.
        r.tick(&mut s, 86_400 - 30, &[o(H, Seen::Asleep)]).unwrap();
        r.tick(&mut s, 86_400 + 30, &[o(H, Seen::Asleep)]).unwrap();
        assert!(r.open().is_empty());
        let utc = &s.uptime_days(0, 3 * 86_400, 0).unwrap()[0].days;
        assert_eq!(utc.len(), 2);
        let local = &s.uptime_days(0, 3 * 86_400, 3600).unwrap()[0].days;
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].day, 86_400 - 3600);
        assert_eq!(local[0].asleep, 60);
        assert_eq!(availability(local[0].up.into(), local[0].down.into()), None);
        assert_eq!(availability(999, 1), Some(99.9));
    }

    #[test]
    fn prunes_old_and_orphaned_rows() {
        let mut s = store();
        let mut r = Recorder::default();
        let now = 500 * 86_400;
        r.tick(&mut s, 1000, &[o(H, Seen::Down)]).unwrap();
        r.tick(&mut s, 1010, &[o(H, Seen::Down)]).unwrap();
        r.tick(&mut s, now, &[o(H, Seen::Up), o(Node::Service(7), Seen::Down)]).unwrap();
        r.tick(&mut s, now + 10, &[o(H, Seen::Up), o(Node::Service(7), Seen::Down)]).unwrap();
        let n = s.prune_uptime(now).unwrap();
        assert!(n >= 3, "{n}");
        assert!(s.outages(None, 0, now * 2).unwrap().is_empty());
        let left = s.uptime_days(0, now * 2, 0).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].node, H);
    }
}
