//! Metric history in SQLite: samples are averaged into 1-minute points,
//! which also feed 15-minute and hourly rollups (running averages, so no
//! batch job). Pruning drops minute points after `retention_days`, 15-minute
//! rollups after [`ROLLUP_DAYS`] and hourly ones after [`HOURLY_DAYS`].

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Sample;
use crate::hosts::HostId;
use crate::store::Store;
use crate::Result;

pub const MINUTE: i64 = 60;
pub const ROLLUP: i64 = 900;
/// How long 15-minute rollups are kept.
pub const ROLLUP_DAYS: i64 = 90;
/// Hourly rollups (M10t), for ranges longer than a month.
pub const HOURLY: i64 = 3600;
/// How long hourly rollups are kept.
pub const HOURLY_DAYS: i64 = 400;
/// Ranges up to this long are served from minute points.
pub const MINUTE_RANGE_SECS: i64 = 2 * 86400;
/// Ranges up to this long are served from 15-minute rollups; longer ones from hourly.
pub const ROLLUP_RANGE_SECS: i64 = 31 * 86400;

/// The resolution a range is served at.
pub fn resolution(from: i64, to: i64) -> i64 {
    match to - from {
        d if d <= MINUTE_RANGE_SECS => MINUTE,
        d if d <= ROLLUP_RANGE_SECS => ROLLUP,
        _ => HOURLY,
    }
}

/// One stored point (averages over its bucket).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// Bucket start, unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub ts: f64,
    /// CPU busy %.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub cpu: Option<f32>,
    /// Memory used %.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub mem: Option<f32>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub swap: Option<f32>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub load1: Option<f32>,
    /// Fullest mount, used %.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub disk: Option<f32>,
    /// Bytes/s.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub net_rx: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub net_tx: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub disk_read: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub disk_write: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub temp: Option<f32>,
    /// Busiest GPU, busy %.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub gpu: Option<f32>,
    /// Fullest GPU memory, % of its VRAM.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub gpu_mem: Option<f32>,
    /// Hottest GPU, °C.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub gpu_temp: Option<f32>,
    /// All GPUs together, watts.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub gpu_power: Option<f32>,
}

impl Point {
    pub fn from_sample(s: &Sample) -> Self {
        let pct = |used: u64, total: u64| (total > 0).then(|| (used as f64 / total as f64 * 100.0) as f32);
        Self {
            ts: (s.ts / 1000.0).floor(),
            cpu: s.cpu,
            mem: pct(s.mem_used, s.mem_total),
            swap: pct(s.swap_used, s.swap_total),
            load1: Some(s.load[0]),
            disk: s.mounts.iter().filter_map(|m| pct(m.used, m.used + m.avail)).reduce(f32::max),
            net_rx: Some(s.net_rx),
            net_tx: Some(s.net_tx),
            disk_read: Some(s.disk_read),
            disk_write: Some(s.disk_write),
            temp: s.cpu_temp,
            gpu: s.gpus.iter().filter_map(|g| g.util).reduce(f32::max),
            gpu_mem: s
                .gpus
                .iter()
                .filter_map(|g| match (g.mem_used, g.mem_total) {
                    (Some(u), Some(t)) => pct(u, t),
                    _ => None,
                })
                .reduce(f32::max),
            gpu_temp: s.gpus.iter().filter_map(|g| g.temp).reduce(f32::max),
            gpu_power: s.gpus.iter().filter_map(|g| g.power).reduce(|a, b| a + b),
        }
    }
}

/// Averages samples into minute buckets; `push` returns the finished point
/// when a sample lands in a new minute.
#[derive(Default)]
pub struct MinuteAverager {
    bucket: i64,
    n: u32,
    sum: [f64; N],
    seen: [u32; N],
}

impl MinuteAverager {
    pub fn push(&mut self, p: &Point) -> Option<Point> {
        let bucket = (p.ts as i64).div_euclid(MINUTE) * MINUTE;
        let done = (self.n > 0 && bucket != self.bucket).then(|| self.take());
        if self.n == 0 {
            self.bucket = bucket;
        }
        let vals = values(p);
        for (i, v) in vals.iter().enumerate() {
            if let Some(v) = v {
                self.sum[i] += v;
                self.seen[i] += 1;
            }
        }
        self.n += 1;
        done
    }

    /// The partial current minute, if any (e.g. when a host goes away).
    pub fn flush(&mut self) -> Option<Point> {
        (self.n > 0).then(|| self.take())
    }

    fn take(&mut self) -> Point {
        let avg: Vec<Option<f64>> =
            (0..N).map(|i| (self.seen[i] > 0).then(|| self.sum[i] / self.seen[i] as f64)).collect();
        let p = point(self.bucket, &avg);
        *self = Self::default();
        p
    }
}

/// Number of stored columns.
const N: usize = 14;

fn values(p: &Point) -> [Option<f64>; N] {
    let f = |v: Option<f32>| v.map(f64::from);
    [
        f(p.cpu),
        f(p.mem),
        f(p.swap),
        f(p.load1),
        f(p.disk),
        p.net_rx,
        p.net_tx,
        p.disk_read,
        p.disk_write,
        f(p.temp),
        f(p.gpu),
        f(p.gpu_mem),
        f(p.gpu_temp),
        f(p.gpu_power),
    ]
}

fn point(ts: i64, v: &[Option<f64>]) -> Point {
    let f = |x: Option<f64>| x.map(|x| x as f32);
    Point {
        ts: ts as f64,
        cpu: f(v[0]),
        mem: f(v[1]),
        swap: f(v[2]),
        load1: f(v[3]),
        disk: f(v[4]),
        net_rx: v[5],
        net_tx: v[6],
        disk_read: v[7],
        disk_write: v[8],
        temp: f(v[9]),
        gpu: f(v[10]),
        gpu_mem: f(v[11]),
        gpu_temp: f(v[12]),
        gpu_power: f(v[13]),
    }
}

const COLS: &str =
    "cpu, mem, swap, load1, disk, net_rx, net_tx, disk_read, disk_write, temp, gpu, gpu_mem, gpu_temp, gpu_power";

/// `?4, ?5, …` for the stored columns after host, res and ts.
fn placeholders() -> String {
    (4..4 + N).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ")
}

impl Store {
    /// Stores a minute point and folds it into its 15-minute and hourly rollups.
    pub fn insert_metric_point(&mut self, host_id: HostId, p: &Point) -> Result<()> {
        let ts = (p.ts as i64).div_euclid(MINUTE) * MINUTE;
        let v = values(p);
        let tx = self.conn_mut().transaction()?;
        let args = |res: i64, ts: i64| {
            let mut a: Vec<rusqlite::types::Value> = vec![host_id.into(), res.into(), ts.into()];
            a.extend(v.iter().map(|x| x.map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Real)));
            rusqlite::params_from_iter(a)
        };
        let ph = placeholders();
        let inserted = tx.execute(
            &format!(
                "INSERT INTO metric_points (host_id, res, ts, n, {COLS}) VALUES (?1, ?2, ?3, 1, {ph})
                 ON CONFLICT (host_id, res, ts) DO NOTHING"
            ),
            args(MINUTE, ts),
        )?;
        if inserted == 0 {
            // Already have this minute (e.g. a reconnect replayed it).
            return Ok(());
        }
        // Running average per column; a NULL sample leaves the column as it was.
        let upd: Vec<String> = COLS
            .split(", ")
            .map(|c| {
                format!(
                    "{c} = CASE WHEN excluded.{c} IS NULL THEN {c} WHEN {c} IS NULL THEN excluded.{c} \
                     ELSE ({c} * n + excluded.{c}) / (n + 1) END"
                )
            })
            .collect();
        let sql = format!(
            "INSERT INTO metric_points (host_id, res, ts, n, {COLS}) VALUES (?1, ?2, ?3, 1, {ph})
             ON CONFLICT (host_id, res, ts) DO UPDATE SET {}, n = n + 1",
            upd.join(", ")
        );
        for res in [ROLLUP, HOURLY] {
            tx.execute(&sql, args(res, ts.div_euclid(res) * res))?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Points in `[from, to]` (unix seconds): minute points for short
    /// ranges, 15-minute rollups up to a month, hourly ones beyond.
    pub fn metric_history(&self, host_id: HostId, from: i64, to: i64) -> Result<Vec<Point>> {
        let res = resolution(from, to);
        let mut stmt = self.conn().prepare(&format!(
            "SELECT ts, {COLS} FROM metric_points WHERE host_id = ?1 AND res = ?2 AND ts >= ?3 AND ts <= ?4 ORDER BY ts"
        ))?;
        let rows = stmt.query_map(params![host_id, res, from.div_euclid(res) * res, to], |r| {
            let v: Vec<Option<f64>> = (1..=N).map(|i| r.get(i)).collect::<rusqlite::Result<_>>()?;
            Ok(point(r.get(0)?, &v))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Drops minute points older than `retention_days`, 15-minute rollups
    /// older than [`ROLLUP_DAYS`] (or the retention, if longer) and hourly
    /// ones older than [`HOURLY_DAYS`]. Returns rows removed.
    pub fn prune_metrics(&mut self, now: i64, retention_days: u32) -> Result<usize> {
        let days = i64::from(retention_days.clamp(1, 365));
        let a = self
            .conn()
            .execute("DELETE FROM metric_points WHERE res = ?1 AND ts < ?2", params![MINUTE, now - days * 86400])?;
        let b = self.conn().execute(
            "DELETE FROM metric_points WHERE res = ?1 AND ts < ?2",
            params![ROLLUP, now - days.max(ROLLUP_DAYS) * 86400],
        )?;
        let c = self.conn().execute(
            "DELETE FROM metric_points WHERE res = ?1 AND ts < ?2",
            params![HOURLY, now - days.max(HOURLY_DAYS) * 86400],
        )?;
        Ok(a + b + c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn p(ts: f64, cpu: f32) -> Point {
        Point { ts, cpu: Some(cpu), net_rx: Some(f64::from(cpu) * 10.0), ..Default::default() }
    }

    #[test]
    fn migration_seeds_hourly_from_rollups() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "server01".into(), hostname: "server01.lan".into(), ..Default::default() })
            .unwrap();
        // Two 15-minute rollups in one hour: 10 samples at 10%, 30 at 30% (temp only in one).
        s.conn()
            .execute_batch(&format!(
                "INSERT INTO metric_points (host_id, res, ts, n, cpu, temp) VALUES ({0}, 900, 7200, 10, 10.0, 50.0), ({0}, 900, 8100, 30, 30.0, NULL);",
                h.id
            ))
            .unwrap();
        s.conn().execute_batch(include_str!("../store/migrations/0026_metrics_hourly.sql")).unwrap();
        let p = s.metric_history(h.id, 0, 100 * 86400).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].ts, p[0].cpu, p[0].temp, p[0].mem), (7200.0, Some(25.0), Some(50.0), None));
    }

    #[test]
    fn stores_gpu_memory_temperature_and_power() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "gpu01".into(), hostname: "gpu01.lan".into(), ..Default::default() })
            .unwrap();
        let a = Point {
            ts: 60.0,
            gpu: Some(50.0),
            gpu_mem: Some(40.0),
            gpu_temp: Some(60.0),
            gpu_power: Some(100.0),
            ..Default::default()
        };
        let b = Point {
            ts: 61.0,
            gpu: Some(70.0),
            gpu_mem: None,
            gpu_temp: Some(70.0),
            gpu_power: Some(200.0),
            ..Default::default()
        };
        s.insert_metric_point(h.id, &a).unwrap();
        s.insert_metric_point(h.id, &Point { ts: 120.0, ..b.clone() }).unwrap();
        let p = s.metric_history(h.id, 0, 600).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].gpu_mem, p[0].gpu_temp, p[0].gpu_power), (Some(40.0), Some(60.0), Some(100.0)));
        // The 15-minute rollup averages; a missing VRAM reading doesn't count.
        let r = s.metric_history(h.id, 0, 7 * 86400).unwrap();
        assert_eq!(
            (r[0].gpu, r[0].gpu_mem, r[0].gpu_temp, r[0].gpu_power),
            (Some(60.0), Some(40.0), Some(65.0), Some(150.0))
        );
    }

    #[test]
    fn picks_a_resolution_by_range() {
        assert_eq!(resolution(0, 3600), MINUTE);
        assert_eq!(resolution(0, 7 * 86400), ROLLUP);
        assert_eq!(resolution(0, 90 * 86400), HOURLY);
    }

    #[test]
    fn averages_per_minute() {
        let mut a = MinuteAverager::default();
        assert_eq!(a.push(&p(120.0, 10.0)), None);
        assert_eq!(a.push(&p(150.0, 30.0)), None);
        let done = a.push(&p(181.0, 50.0)).unwrap();
        assert_eq!((done.ts, done.cpu, done.net_rx, done.temp), (120.0, Some(20.0), Some(200.0), None));
        assert_eq!(a.flush().unwrap().cpu, Some(50.0));
        assert_eq!(a.flush(), None);
    }

    #[test]
    fn stores_rolls_up_queries_and_prunes() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "server01".into(), hostname: "server01.lan".into(), ..Default::default() })
            .unwrap();
        let day = 86400;
        let base = 100 * day;
        for i in 0..30 {
            s.insert_metric_point(h.id, &p((base + i * 60) as f64, i as f32)).unwrap();
        }
        // Same minute again is ignored.
        s.insert_metric_point(h.id, &p(base as f64, 99.0)).unwrap();
        let recent = s.metric_history(h.id, base, base + 3600).unwrap();
        assert_eq!(recent.len(), 30);
        assert_eq!(recent[0].cpu, Some(0.0));
        // Long range: two 15-minute rollups averaging 0..14 and 15..29.
        let long = s.metric_history(h.id, base - 7 * day, base + 3600).unwrap();
        assert_eq!(long.len(), 2);
        assert_eq!(long[1].cpu, Some(22.0));
        assert_eq!(long[0].cpu, Some(7.0));
        // Prune: minute points older than 7 days go, rollups stay for 90.
        assert_eq!(s.prune_metrics(base + 8 * day, 7).unwrap(), 30);
        assert_eq!(s.metric_history(h.id, base - 7 * day, base + 3600).unwrap().len(), 2);
        s.prune_metrics(base + 91 * day, 7).unwrap();
        assert!(s.metric_history(h.id, base - 7 * day, base + 3600).unwrap().is_empty());
        // Hourly rollups stay for 400 days and serve ranges over a month.
        let year = s.metric_history(h.id, base - 300 * day, base + 3600).unwrap();
        assert_eq!(year.len(), 1);
        assert_eq!(year[0].cpu, Some(14.5));
        s.prune_metrics(base + 401 * day, 7).unwrap();
        assert!(s.metric_history(h.id, base - 300 * day, base + 3600).unwrap().is_empty());
        s.insert_metric_point(h.id, &p(base as f64, 1.0)).unwrap();
        s.delete_host(h.id).unwrap();
        assert!(s.metric_history(h.id, 0, i64::MAX).unwrap().is_empty());
    }
}
