//! Disk usage trend (M10m): hourly usage per mount, recorded while a host is
//! monitored, and a straight-line forecast of when each mount fills up.

use rusqlite::params;
use serde::Serialize;

use crate::alerts::{AlertKind, Condition, Severity};
use crate::error::Result;
use crate::hosts::HostId;
use crate::metrics::MountUsage;
use crate::store::Store;

const HOUR: i64 = 3600;
const DAY: f64 = 86_400.0;
/// History kept per mount.
pub const KEEP_DAYS: i64 = 90;
/// The trend is fitted over this many recent days.
pub const WINDOW_DAYS: i64 = 14;
/// Points shown in the sparkline (one per day).
const DAILY_MAX: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UsagePoint {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub ts: i64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub used: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MountForecast {
    pub mount: String,
    /// Usable size (used + available), bytes.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub size: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub used: u64,
    /// When the newest point was recorded.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    /// Growth in bytes per day over the window (negative when shrinking);
    /// `None` until there is enough history.
    pub per_day: Option<f64>,
    /// Days until full at that rate; `None` when not growing or not known.
    pub days_left: Option<f64>,
    /// How many days of history the trend is based on.
    pub span_days: f64,
    /// The last sample of each day, oldest first, for a sparkline.
    pub daily: Vec<UsagePoint>,
}

impl Store {
    /// Records the latest usage of each mount in the current hour's bucket
    /// and drops what is older than [`KEEP_DAYS`].
    pub fn disk_usage_record(&self, host: HostId, mounts: &[MountUsage], now: i64) -> Result<()> {
        let c = self.conn();
        let ts = now - now.rem_euclid(HOUR);
        for m in mounts {
            let size = m.used.saturating_add(m.avail);
            if size == 0 {
                continue;
            }
            c.execute(
                "INSERT OR REPLACE INTO disk_usage (host_id, mount, ts, used, size) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![host, m.mount, ts, m.used as i64, size as i64],
            )?;
        }
        c.execute("DELETE FROM disk_usage WHERE host_id = ?1 AND ts < ?2", params![host, now - KEEP_DAYS * 86_400])?;
        Ok(())
    }

    /// `(mount, ts, used, size)` for `host` since `since`, by mount then time.
    pub fn disk_usage_since(&self, host: HostId, since: i64) -> Result<Vec<(String, i64, u64, u64)>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT mount, ts, used, size FROM disk_usage WHERE host_id = ?1 AND ts >= ?2 ORDER BY mount, ts",
        )?;
        let rows = st.query_map(params![host, since], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)? as u64, r.get::<_, i64>(3)? as u64))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Forecasts for every mount of `host` with recorded history.
    pub fn disk_forecasts(&self, host: HostId, now: i64) -> Result<Vec<MountForecast>> {
        let rows = self.disk_usage_since(host, now - DAILY_MAX as i64 * 86_400)?;
        Ok(forecasts(&rows, now))
    }
}

/// Least-squares slope (bytes per day) of `points` within the window, with
/// the span it covers. Needs a day of history and six hourly points.
pub fn trend(points: &[UsagePoint], now: i64) -> (Option<f64>, f64) {
    let recent: Vec<&UsagePoint> = points.iter().filter(|p| p.ts >= now - WINDOW_DAYS * 86_400).collect();
    let span = match (recent.first(), recent.last()) {
        (Some(a), Some(b)) => (b.ts - a.ts) as f64 / DAY,
        _ => 0.0,
    };
    if recent.len() < 6 || span < 1.0 {
        return (None, span);
    }
    let n = recent.len() as f64;
    let t0 = recent[0].ts;
    let xs: Vec<f64> = recent.iter().map(|p| (p.ts - t0) as f64 / DAY).collect();
    let ys: Vec<f64> = recent.iter().map(|p| p.used as f64).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
    if sxx == 0.0 {
        return (None, span);
    }
    let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    (Some(sxy / sxx), span)
}

/// Days until `used` reaches `size` at `per_day`. Growth under 0.01 % of the
/// size per day counts as flat.
pub fn days_left(used: u64, size: u64, per_day: Option<f64>) -> Option<f64> {
    let rate = per_day?;
    if size == 0 || rate <= size as f64 * 0.0001 {
        return None;
    }
    Some((size.saturating_sub(used)) as f64 / rate)
}

/// Groups `(mount, ts, used, size)` rows (sorted by mount, then time).
pub fn forecasts(rows: &[(String, i64, u64, u64)], now: i64) -> Vec<MountForecast> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let mount = &rows[i].0;
        let j = rows[i..].iter().position(|r| &r.0 != mount).map_or(rows.len(), |k| i + k);
        let group = &rows[i..j];
        i = j;
        let points: Vec<UsagePoint> = group.iter().map(|r| UsagePoint { ts: r.1, used: r.2 }).collect();
        let last = &group[group.len() - 1];
        let (per_day, span_days) = trend(&points, now);
        let mut daily: Vec<UsagePoint> = Vec::new();
        for p in &points {
            match daily.last_mut() {
                Some(d) if d.ts.div_euclid(86_400) == p.ts.div_euclid(86_400) => *d = *p,
                _ => daily.push(*p),
            }
        }
        if daily.len() > DAILY_MAX {
            daily.drain(..daily.len() - DAILY_MAX);
        }
        out.push(MountForecast {
            mount: mount.clone(),
            size: last.3,
            used: last.2,
            at: last.1,
            per_day,
            days_left: days_left(last.2, last.3, per_day),
            span_days,
            daily,
        });
    }
    out
}

/// "FILLING UP" when a mount is forecast to be full within `within_days`
/// (critical inside three days). Mounts whose newest point is older than a
/// day are left out: the host isn't being watched.
pub fn conditions(host: HostId, name: &str, list: &[MountForecast], within_days: u16, now: i64) -> Vec<Condition> {
    list.iter()
        .filter(|f| now - f.at <= 86_400)
        .filter_map(|f| {
            let d = f.days_left?;
            (d < f64::from(within_days)).then(|| Condition {
                key: format!("fill:{host}:{}", f.mount),
                kind: AlertKind::DiskFilling,
                severity: if d < 3.0 { Severity::Critical } else { Severity::Warn },
                title: name.to_owned(),
                detail: format!("{} full in about {} at the current rate", f.mount, days_words(d)),
                host_id: Some(host),
            })
        })
        .collect()
}

/// "4 days", "18 hours", "less than an hour".
pub fn days_words(d: f64) -> String {
    if d >= 2.0 {
        format!("{} days", d.floor() as i64)
    } else if d * 24.0 >= 1.5 {
        format!("{} hours", (d * 24.0).round() as i64)
    } else {
        "an hour".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    fn mount(m: &str, used: u64, avail: u64) -> MountUsage {
        MountUsage {
            device: "/dev/sda1".into(),
            mount: m.into(),
            fstype: "ext4".into(),
            total: used + avail,
            used,
            avail,
            read_only: false,
        }
    }

    #[test]
    fn trend_and_days() {
        let now = 1_700_000_000;
        // 2 GB a day for 5 days, sampled every 6 hours.
        let pts: Vec<UsagePoint> = (0..=20)
            .map(|k| UsagePoint { ts: now - (20 - k) * 6 * 3600, used: 100 * GB + k as u64 * GB / 2 })
            .collect();
        let (rate, span) = trend(&pts, now);
        assert!((rate.unwrap() - 2.0 * GB as f64).abs() < 1.0);
        assert!((span - 5.0).abs() < 1e-9);
        assert_eq!(days_left(110 * GB, 120 * GB, rate).map(|d| d.round()), Some(5.0));
        // Flat or shrinking: no forecast.
        assert_eq!(days_left(10, 100, Some(-5.0)), None);
        assert_eq!(days_left(10 * GB, 1000 * GB, Some(1000.0)), None);
        // Too little history.
        assert_eq!(trend(&pts[18..], now).0, None);
        assert_eq!(days_words(4.7), "4 days");
        assert_eq!(days_words(0.5), "12 hours");
        assert_eq!(days_words(0.01), "an hour");
    }

    #[test]
    fn store_round_trip_and_alert() {
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('lab', 'lab.example.com')", []).unwrap();
        let id = s.conn().last_insert_rowid() as HostId;
        let start = 1_700_000_000 - 1_700_000_000 % 3600;
        for k in 0..72 {
            // /data grows 1 GB per hour (24 GB a day) towards 1000 GB; / stays put.
            let t = start + k * 3600;
            s.disk_usage_record(
                id,
                &[mount("/data", 900 * GB + k as u64 * GB, 100 * GB - k as u64 * GB), mount("/", 20 * GB, 80 * GB)],
                t,
            )
            .unwrap();
            // A second sample in the same hour replaces the first.
            s.disk_usage_record(id, &[mount("/data", 900 * GB + k as u64 * GB, 100 * GB - k as u64 * GB)], t + 600)
                .unwrap();
        }
        let now = start + 71 * 3600 + 700;
        let f = s.disk_forecasts(id, now).unwrap();
        assert_eq!(f.iter().map(|m| m.mount.as_str()).collect::<Vec<_>>(), ["/", "/data"]);
        assert_eq!(f[0].days_left, None);
        let d = &f[1];
        assert_eq!((d.used, d.size), (971 * GB, 1000 * GB));
        assert!((d.per_day.unwrap() / GB as f64 - 24.0).abs() < 0.01);
        assert!((d.days_left.unwrap() * 24.0 - 29.0).abs() < 0.1);
        assert!(d.daily.len() >= 3 && d.daily.len() <= 4);
        let c = conditions(id, "lab", &f, 14, now);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].severity, c[0].key.clone()), (Severity::Critical, format!("fill:{}:/data", id)));
        assert_eq!(c[0].detail, "/data full in about 29 hours at the current rate");
        // Old history is pruned.
        s.disk_usage_record(id, &[mount("/", 1, 1)], now + KEEP_DAYS * 86_400 + 4 * 86_400).unwrap();
        assert_eq!(s.disk_usage_since(id, 0).unwrap().len(), 1);
        // Stale data doesn't alert.
        assert!(conditions(id, "lab", &f, 14, now + 2 * 86_400).is_empty());
    }
}
