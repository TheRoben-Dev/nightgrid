//! What the app keeps about nodes: the node id per host, the "use the node"
//! switch, sync cursors, and how synced rows merge with the app's own.

use rusqlite::{params, OptionalExtension};

use super::{DiskRow, UptimeHour};
use crate::hosts::HostId;
use crate::store::Store;
use crate::timeline::{EventKind, Level};
use crate::Result;

/// Reserved cursor kinds in `node_sync` (metric tiers use their resolution).
pub const CURSOR_EVENTS: i64 = -1;
pub const CURSOR_UPTIME: i64 = -2;
pub const CURSOR_DISK: i64 = -3;

impl Store {
    /// Whether the app may use the node on this host (default yes).
    pub fn node_use(&self, host: HostId) -> Result<bool> {
        let v: Option<i64> = self
            .conn()
            .query_row("SELECT use_node FROM node_hosts WHERE host_id = ?1", [host], |r| r.get(0))
            .optional()?;
        Ok(v != Some(0))
    }

    pub fn node_use_set(&self, host: HostId, on: bool) -> Result<()> {
        self.conn().execute(
            "INSERT INTO node_hosts (host_id, use_node) VALUES (?1, ?2)
             ON CONFLICT (host_id) DO UPDATE SET use_node = excluded.use_node",
            params![host, on as i64],
        )?;
        Ok(())
    }

    /// Records the node id seen on `host`. When it differs from last time
    /// the node's history started over, so the cursors are dropped; returns
    /// whether that happened.
    pub fn node_seen(&mut self, host: HostId, node_id: &str) -> Result<bool> {
        let tx = self.conn_mut().transaction()?;
        let old: Option<String> =
            tx.query_row("SELECT node_id FROM node_hosts WHERE host_id = ?1", [host], |r| r.get(0)).optional()?;
        let reset = matches!(&old, Some(o) if !o.is_empty() && o != node_id);
        if reset || old.as_deref().map_or(true, str::is_empty) {
            tx.execute("DELETE FROM node_sync WHERE host_id = ?1", [host])?;
        }
        tx.execute(
            "INSERT INTO node_hosts (host_id, node_id) VALUES (?1, ?2)
             ON CONFLICT (host_id) DO UPDATE SET node_id = excluded.node_id",
            params![host, node_id],
        )?;
        tx.commit()?;
        Ok(reset)
    }

    /// Forgets a host's node (uninstalled): its id and cursors, keeping the switch.
    pub fn node_forget(&self, host: HostId) -> Result<()> {
        let c = self.conn();
        c.execute("DELETE FROM node_sync WHERE host_id = ?1", [host])?;
        c.execute("UPDATE node_hosts SET node_id = '' WHERE host_id = ?1", [host])?;
        Ok(())
    }

    pub fn node_cursor(&self, host: HostId, kind: i64) -> Result<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT cursor FROM node_sync WHERE host_id = ?1 AND res = ?2", params![host, kind], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn node_cursor_set(&self, host: HostId, kind: i64, cursor: i64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO node_sync (host_id, res, cursor) VALUES (?1, ?2, ?3)
             ON CONFLICT (host_id, res) DO UPDATE SET cursor = excluded.cursor",
            params![host, kind, cursor],
        )?;
        Ok(())
    }

    /// Disk usage the node recorded; an hour the app recorded itself wins.
    pub fn disk_usage_merge(&self, host: HostId, rows: &[DiskRow]) -> Result<usize> {
        let c = self.conn();
        let mut n = 0;
        for r in rows.iter().filter(|r| r.size > 0) {
            n += c.execute(
                "INSERT OR IGNORE INTO disk_usage (host_id, mount, ts, used, size) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![host, r.mount, r.ts, r.used as i64, r.size as i64],
            )?;
        }
        Ok(n)
    }

    /// Hours the node witnessed, merged with the app's own: the larger value
    /// of each column (each side only counts what it saw), then clamped to an
    /// hour, taking the excess from `up` first.
    pub fn uptime_merge(&mut self, host: HostId, hours: &[UptimeHour]) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        for h in hours {
            tx.execute(
                "INSERT INTO uptime_hours (kind, subject, hour, up, down, asleep) VALUES ('host', ?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (kind, subject, hour) DO UPDATE SET
                   up = max(up, excluded.up), down = max(down, excluded.down), asleep = max(asleep, excluded.asleep)",
                params![host, h.hour, h.up.max(0), h.down.max(0), h.asleep.max(0)],
            )?;
            tx.execute(
                "UPDATE uptime_hours SET up = max(0, 3600 - down - asleep)
                 WHERE kind = 'host' AND subject = ?1 AND hour = ?2 AND up + down + asleep > 3600",
                params![host, h.hour],
            )?;
            tx.execute(
                "UPDATE uptime_hours SET asleep = max(0, 3600 - down)
                 WHERE kind = 'host' AND subject = ?1 AND hour = ?2 AND up + down + asleep > 3600",
                params![host, h.hour],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// An outage the node saw while the app was away, unless one already
    /// starts within a minute of it.
    pub fn outage_merge(&self, host: HostId, start: i64, end: i64, detail: &str) -> Result<bool> {
        let c = self.conn();
        let dup: Option<i64> = c
            .query_row(
                "SELECT id FROM outages WHERE kind = 'host' AND subject = ?1 AND abs(start - ?2) <= 60",
                params![host, start],
                |r| r.get(0),
            )
            .optional()?;
        if dup.is_some() {
            return Ok(false);
        }
        c.execute(
            "INSERT INTO outages (kind, subject, start, end, detail) VALUES ('host', ?1, ?2, ?3, ?4)",
            params![host, start, end.max(start), detail.chars().take(200).collect::<String>()],
        )?;
        Ok(true)
    }

    /// A timeline entry from the node, skipped when the timeline already has
    /// the same kind and title within `window` seconds (the app saw it too).
    #[allow(clippy::too_many_arguments)]
    pub fn event_merge(
        &self,
        host: HostId,
        at: i64,
        kind: EventKind,
        level: Level,
        title: &str,
        detail: &str,
        window: i64,
        any_title: bool,
    ) -> Result<bool> {
        let k = kind.as_str();
        let dup: Option<i64> = self
            .conn()
            .query_row(
                "SELECT 1 FROM host_events WHERE host_id = ?1 AND kind = ?2 AND abs(at - ?3) <= ?4
                   AND (?5 OR title = ?6) LIMIT 1",
                params![host, k, at, window, any_title, title],
                |r| r.get(0),
            )
            .optional()?;
        if dup.is_some() {
            return Ok(false);
        }
        self.event_add(host, at, kind, level, title, detail)?;
        Ok(true)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn store() -> (Store, HostId) {
        let mut s = Store::open_in_memory().unwrap();
        let id = s
            .create_host(&crate::hosts::HostInput {
                name: "server01".into(),
                hostname: "10.0.0.2".into(),
                ..Default::default()
            })
            .unwrap()
            .id;
        (s, id)
    }

    #[test]
    fn a_new_node_id_drops_the_cursors() {
        let (mut s, h) = store();
        assert!(!s.node_seen(h, "a").unwrap());
        s.node_cursor_set(h, 60, 1000).unwrap();
        assert!(!s.node_seen(h, "a").unwrap());
        assert_eq!(s.node_cursor(h, 60).unwrap(), Some(1000));
        assert!(s.node_seen(h, "b").unwrap());
        assert_eq!(s.node_cursor(h, 60).unwrap(), None);
        assert!(s.node_use(h).unwrap());
        s.node_use_set(h, false).unwrap();
        assert!(!s.node_use(h).unwrap());
        assert!(!s.node_seen(h, "b").unwrap());
        assert!(!s.node_use(h).unwrap(), "seeing the node keeps the switch");
    }

    #[test]
    fn uptime_takes_the_larger_side_and_clamps_to_an_hour() {
        let (mut s, h) = store();
        s.uptime_merge(h, &[UptimeHour { hour: 3600, up: 1200, down: 0, asleep: 0 }]).unwrap();
        s.uptime_merge(h, &[UptimeHour { hour: 3600, up: 3000, down: 600, asleep: 0 }]).unwrap();
        let row: (i64, i64, i64) = s
            .conn()
            .query_row("SELECT up, down, asleep FROM uptime_hours WHERE subject = ?1", [h], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(row, (3000, 600, 0));
        s.uptime_merge(h, &[UptimeHour { hour: 3600, up: 3500, down: 600, asleep: 200 }]).unwrap();
        let row: (i64, i64, i64) = s
            .conn()
            .query_row("SELECT up, down, asleep FROM uptime_hours WHERE subject = ?1", [h], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(row, (2800, 600, 200));
    }

    #[test]
    fn disk_rows_and_outages_do_not_overwrite_the_app() {
        let (s, h) = store();
        let row = |used| DiskRow { mount: "/srv".into(), ts: 7200, used, size: 100 };
        assert_eq!(s.disk_usage_merge(h, &[row(40)]).unwrap(), 1);
        assert_eq!(s.disk_usage_merge(h, &[row(50)]).unwrap(), 0);
        assert!(s.outage_merge(h, 1000, 1300, "Down").unwrap());
        assert!(!s.outage_merge(h, 1030, 1300, "Down").unwrap());
        assert!(s.event_merge(h, 500, EventKind::Boot, Level::Info, "booted", "", 120, true).unwrap());
        assert!(!s.event_merge(h, 560, EventKind::Boot, Level::Info, "Booted (node)", "", 120, true).unwrap());
        assert!(s.event_merge(h, 560, EventKind::Alert, Level::Warn, "A", "", 90, false).unwrap());
        assert!(s.event_merge(h, 560, EventKind::Alert, Level::Warn, "B", "", 90, false).unwrap());
    }
}
