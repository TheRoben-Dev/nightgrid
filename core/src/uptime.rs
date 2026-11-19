//! Uptime history (M10t): how long each host and service was seen up, down
//! or asleep, per hour, and each outage from its first to its last "down".
//!
//! Only what Nightgrid observed counts: the alerts loop reports every host
//! and checked service each round, and the time since the previous round
//! (at most [`MAX_GAP`]) is added to the hour it falls in. When the app
//! wasn't running, nothing is recorded, so availability is "up out of
//! up + down", never "up out of the whole month". Sleeping (suspended, in a
//! sleep window, rebooting from Nightgrid) is kept apart and isn't downtime.
//!
//! A "down" is only believed once it's confirmed (D-181), because most
//! failed probes say more about this computer than about the target:
//! - a target must be seen down for [`CONFIRM_SECS`] without a break before
//!   that time counts (a shorter blip is unknown time, not up or down);
//! - for [`WAKE_GRACE`] after the app starts or the computer wakes, downs
//!   don't count (the network, tailnet and pollers are still coming back);
//! - a round where everything watched is down, or every host is, is this
//!   computer's own outage and is unknown time too.

use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::depgraph::Node;
use crate::store::Store;
use crate::Result;

/// The longest gap between two rounds that still counts; a longer one
/// (the computer slept, the app was closed) is unknown time, and the round
/// after it starts a [`WAKE_GRACE`].
pub const MAX_GAP: i64 = 60;
/// How long a target must be seen down, without a break, before it counts
/// as an outage. One failed service check (already retried once) or a few
/// failed metrics polls stay under it.
pub const CONFIRM_SECS: i64 = 150;
/// Downs seen this soon after the app started or the computer woke don't count.
pub const WAKE_GRACE: i64 = 180;
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

/// Seconds of `[from, to)` per hour, oldest first.
fn by_hour(from: i64, to: i64) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut t = from;
    while t < to {
        let hour = t.div_euclid(HOUR) * HOUR;
        let end = (hour + HOUR).min(to);
        out.push((hour, end - t));
        t = end;
    }
    out
}

fn add(tx: &rusqlite::Transaction<'_>, kind: &str, id: i64, col: &str, hour: i64, secs: i64) -> Result<()> {
    tx.execute(
        &format!(
            "INSERT INTO uptime_hours (kind, subject, hour, {col}) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (kind, subject, hour) DO UPDATE SET {col} = {col} + excluded.{col}"
        ),
        params![kind, id, hour, secs],
    )?;
    Ok(())
}

/// True when the round says more about this computer than the targets:
/// everything watched is down (at least three), or every host is (at least two).
pub fn local_outage(obs: &[Observation]) -> bool {
    let watched: Vec<&Observation> = obs.iter().filter(|o| o.seen != Seen::Asleep).collect();
    let all_down = |v: &[&Observation]| v.iter().all(|o| o.seen == Seen::Down);
    let hosts: Vec<&Observation> = watched.iter().copied().filter(|o| matches!(o.node, Node::Host(_))).collect();
    (watched.len() >= 3 && all_down(&watched)) || (hosts.len() >= 2 && all_down(&hosts))
}

/// A target seen down, not yet for [`CONFIRM_SECS`].
#[derive(Debug)]
struct Pending {
    start: i64,
    detail: String,
    hours: Vec<(i64, i64)>,
}

/// Feeds rounds of observations into the store and keeps outages open
/// between rounds. One per app; the outages it opened stay open only while
/// it runs (a restart or wake starts a new outage if still down).
#[derive(Debug, Default)]
pub struct Recorder {
    last: Option<i64>,
    open: HashMap<Node, i64>,
    pending: HashMap<Node, Pending>,
    grace_until: i64,
}

impl Recorder {
    pub fn tick(&mut self, store: &mut Store, now: i64, obs: &[Observation]) -> Result<()> {
        let dt = match self.last {
            Some(t) if now > t && now - t <= MAX_GAP => now - t,
            Some(t) if now <= t => 0,
            // First round, or after sleep or a stall: the time in between is
            // unknown, and what's seen next can't be trusted for a moment.
            _ => {
                self.grace_until = now + WAKE_GRACE;
                self.open.clear();
                self.pending.clear();
                0
            }
        };
        self.last = Some(now.max(self.last.unwrap_or(now)));
        let from = now - dt;
        let doubtful = now < self.grace_until || local_outage(obs);
        let tx = store.conn_mut().transaction()?;
        let mut still_open = HashMap::new();
        let mut still_pending = HashMap::new();
        for o in obs {
            let (kind, id) = parts(o.node);
            if o.seen != Seen::Down {
                // Up or asleep: a blip that never got confirmed is unknown time.
                let col = if o.seen == Seen::Up { "up" } else { "asleep" };
                for (hour, secs) in by_hour(from, now) {
                    add(&tx, kind, id, col, hour, secs)?;
                }
                continue;
            }
            if doubtful {
                // Neither up nor down: keep what's open or pending as it is.
                if let Some(&row) = self.open.get(&o.node) {
                    still_open.insert(o.node, row);
                }
                if let Some(p) = self.pending.remove(&o.node) {
                    still_pending.insert(o.node, p);
                }
                continue;
            }
            if let Some(&row) = self.open.get(&o.node) {
                for (hour, secs) in by_hour(from, now) {
                    add(&tx, kind, id, "down", hour, secs)?;
                }
                tx.execute("UPDATE outages SET end = ?2 WHERE id = ?1", params![row, now])?;
                still_open.insert(o.node, row);
                continue;
            }
            let mut p = self.pending.remove(&o.node).unwrap_or_else(|| Pending {
                start: from,
                detail: o.detail.chars().take(200).collect(),
                hours: Vec::new(),
            });
            p.hours.extend(by_hour(from, now));
            if now - p.start >= CONFIRM_SECS {
                for &(hour, secs) in &p.hours {
                    add(&tx, kind, id, "down", hour, secs)?;
                }
                tx.execute(
                    "INSERT INTO outages (kind, subject, start, end, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![kind, id, p.start, now, p.detail],
                )?;
                still_open.insert(o.node, tx.last_insert_rowid());
            } else {
                still_pending.insert(o.node, p);
            }
        }
        tx.commit()?;
        // Up, asleep or no longer reported: the outage ended.
        self.open = still_open;
        self.pending = still_pending;
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
    const S: Node = Node::Service(5);

    /// Ticks every 15 s through `[from, to)` with the same observations.
    fn run(r: &mut Recorder, s: &mut Store, from: i64, to: i64, obs: &[Observation]) {
        let mut t = from;
        while t < to {
            r.tick(s, t, obs).unwrap();
            t += 15;
        }
    }

    fn day(s: &Store, n: Node) -> UptimeDay {
        s.uptime_days(0, 10 * 86_400, 0)
            .unwrap()
            .into_iter()
            .find(|x| x.node == n)
            .map(|x| x.days[0].clone())
            .unwrap_or_default()
    }

    #[test]
    fn adds_time_between_rounds_split_by_hour() {
        let mut s = store();
        let mut r = Recorder::default();
        let t0 = 10 * 3600 - 20;
        r.tick(&mut s, t0, &[o(H, Seen::Up)]).unwrap(); // first round: nothing to add
        r.tick(&mut s, t0 + 30, &[o(H, Seen::Up)]).unwrap(); // 20 s in hour 9, 10 s in hour 10
        r.tick(&mut s, t0 + 30 + 600, &[o(H, Seen::Up)]).unwrap(); // a gap: unknown time
        r.tick(&mut s, t0 + 30 + 660, &[o(H, Seen::Up)]).unwrap();
        assert_eq!(day(&s, H).up, 90);
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
    fn confirmed_outages_open_extend_and_close() {
        let mut s = store();
        let mut r = Recorder::default();
        let t0 = 1000;
        run(&mut r, &mut s, t0, t0 + 300, &[o(H, Seen::Up)]); // past the start-up grace
        run(&mut r, &mut s, t0 + 300, t0 + 600, &[o(H, Seen::Down)]);
        assert_eq!(r.open().len(), 1);
        r.tick(&mut s, t0 + 600, &[o(H, Seen::Up)]).unwrap();
        assert!(r.open().is_empty());
        let out = s.outages(Some(H), 0, 5000).unwrap();
        assert_eq!(out.len(), 1);
        // Down from the round before the first "down" to the last one.
        assert_eq!((out[0].start, out[0].end), (t0 + 285, t0 + 585));
        assert_eq!(out[0].detail, "timed out");
        let d = day(&s, H);
        assert_eq!(d.down, 300, "the unconfirmed first 150 s are counted once confirmed");
        assert_eq!(d.up, 285 + 15);
        assert_eq!(s.outages(Some(Node::Service(9)), 0, 5000).unwrap().len(), 0);
        assert_eq!(s.outages(None, 2000, 5000).unwrap().len(), 0);
    }

    #[test]
    fn short_blips_are_unknown_time() {
        let mut s = store();
        let mut r = Recorder::default();
        run(&mut r, &mut s, 0, 300, &[o(S, Seen::Up)]);
        // One failed check, seen down for its 60 s interval, then up again.
        run(&mut r, &mut s, 300, 360, &[o(S, Seen::Down)]);
        run(&mut r, &mut s, 360, 600, &[o(S, Seen::Up)]);
        // Down, then no longer reported: dropped too.
        run(&mut r, &mut s, 600, 700, &[o(S, Seen::Down)]);
        r.tick(&mut s, 700, &[]).unwrap();
        assert!(s.outages(None, 0, 5000).unwrap().is_empty());
        let d = day(&s, S);
        assert_eq!(d.down, 0);
        assert_eq!(d.up, 285 + 240, "the blip is neither up nor down");
    }

    #[test]
    fn downs_after_start_or_wake_dont_count() {
        let mut s = store();
        let mut r = Recorder::default();
        // Just started: three minutes of failures while the network comes up.
        run(&mut r, &mut s, 0, 180, &[o(S, Seen::Down)]);
        run(&mut r, &mut s, 180, 600, &[o(S, Seen::Up)]);
        // The computer slept for an hour, then the same.
        run(&mut r, &mut s, 4200, 4380, &[o(S, Seen::Down)]);
        run(&mut r, &mut s, 4380, 4500, &[o(S, Seen::Up)]);
        assert!(s.outages(None, 0, 9000).unwrap().is_empty());
        assert_eq!(day(&s, S).down, 0);
        // Still down once the grace is over: an outage from then on.
        run(&mut r, &mut s, 4500, 4800, &[o(S, Seen::Down)]);
        let out = s.outages(None, 0, 9000).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, 4485);
    }

    #[test]
    fn everything_down_at_once_is_a_local_outage() {
        let h2 = Node::Host(2);
        let mut s = store();
        let mut r = Recorder::default();
        let up = [o(H, Seen::Up), o(h2, Seen::Up), o(S, Seen::Up)];
        run(&mut r, &mut s, 0, 300, &up);
        // Every host unreachable (Wi-Fi dropped, tailnet reconnecting).
        let lost = [o(H, Seen::Down), o(h2, Seen::Down), o(S, Seen::Up)];
        run(&mut r, &mut s, 300, 900, &lost);
        assert!(s.outages(None, 0, 9000).unwrap().is_empty());
        // One host down while the other answers: real.
        let one = [o(H, Seen::Down), o(h2, Seen::Up), o(S, Seen::Up)];
        run(&mut r, &mut s, 900, 1200, &one);
        assert_eq!(s.outages(None, 0, 9000).unwrap().len(), 1);
        assert!(local_outage(&[o(H, Seen::Down), o(S, Seen::Down), o(Node::Service(6), Seen::Down)]));
        assert!(!local_outage(&[o(H, Seen::Down), o(S, Seen::Down)]));
        assert!(!local_outage(&[o(H, Seen::Down), o(h2, Seen::Asleep), o(S, Seen::Up)]));
    }

    #[test]
    fn asleep_is_not_down_and_days_follow_the_offset() {
        let mut s = store();
        let mut r = Recorder::default();
        // 23:59:30 to 00:00:30 UTC; with +1 h both halves fall on the same local day.
        r.tick(&mut s, 86_400 - 30, &[o(H, Seen::Asleep)]).unwrap();
        r.tick(&mut s, 86_400, &[o(H, Seen::Asleep)]).unwrap();
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
        run(&mut r, &mut s, 0, 200, &[o(H, Seen::Up)]);
        run(&mut r, &mut s, 200, 500, &[o(H, Seen::Down)]);
        run(&mut r, &mut s, now, now + 200, &[o(H, Seen::Up), o(Node::Service(7), Seen::Up)]);
        run(&mut r, &mut s, now + 200, now + 500, &[o(H, Seen::Up), o(Node::Service(7), Seen::Down)]);
        assert_eq!(s.outages(None, 0, now * 2).unwrap().len(), 2);
        let n = s.prune_uptime(now).unwrap();
        assert!(n >= 3, "{n}");
        assert!(s.outages(None, 0, now * 2).unwrap().is_empty());
        let left = s.uptime_days(0, now * 2, 0).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].node, H);
    }

    #[test]
    fn cleanup_migration_drops_blips_and_client_downs() {
        let s = store();
        let c = s.conn();
        c.execute("INSERT INTO hosts (name, hostname, role) VALUES ('deck', 'deck', 'client')", []).unwrap();
        let deck: i64 = c.last_insert_rowid();
        c.execute_batch(&format!(
            "INSERT INTO uptime_hours (kind, subject, hour, up, down) VALUES
               ('service', 5, 0, 3000, 600), ('service', 5, 3600, 3500, 100), ('host', {deck}, 0, 100, 900);
             INSERT INTO outages (kind, subject, start, end, detail) VALUES
               ('service', 5, 3550, 3650, 'blip'), ('service', 5, 100, 600, 'real'), ('host', {deck}, 0, 900, 'away');"
        ))
        .unwrap();
        c.execute_batch(include_str!("store/migrations/0051_uptime_cleanup.sql")).unwrap();
        let rows: Vec<(String, i64, i64, i64, i64)> = c
            .prepare("SELECT kind, hour, up, down, asleep FROM uptime_hours ORDER BY kind, hour")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            rows,
            vec![
                ("host".into(), 0, 100, 0, 900),
                ("service".into(), 0, 3000, 550, 0),
                ("service".into(), 3600, 3500, 50, 0),
            ]
        );
        let left: Vec<String> = c
            .prepare("SELECT detail FROM outages")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(left, vec!["real".to_owned()]);
    }
}
