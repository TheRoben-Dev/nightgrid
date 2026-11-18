//! Capacity planner (1.1, D-172): how much room each host has, from what
//! Nightgrid already records: its size (cores and memory from the latest
//! metrics sample, kept in `host_specs`), the hourly CPU and memory history
//! (p95 over 7 to 90 days, so a nightly backup doesn't vanish into an
//! average) and the disk forecasts. What-if workloads are fitted in the UI.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::diskforecast::MountForecast;
use crate::hosts::HostId;
use crate::metrics::history::Point;
use crate::store::Store;
use crate::Result;

/// Memory p95 at or above this sends a notice.
pub const MEM_TIGHT: f32 = 90.0;
/// CPU p95 at or above this sends a notice.
pub const CPU_TIGHT: f32 = 85.0;
const GB: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostSpec {
    pub host_id: HostId,
    pub cores: u32,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub mem_total: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CapDisk {
    pub mount: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub size: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub used: u64,
    pub days_left: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostCapacity {
    pub host_id: HostId,
    pub spec: Option<HostSpec>,
    pub cpu_avg: Option<f32>,
    pub cpu_p95: Option<f32>,
    pub mem_avg: Option<f32>,
    pub mem_p95: Option<f32>,
    pub gpu_p95: Option<f32>,
    /// Hours of history behind the numbers.
    pub hours: u32,
    pub disks: Vec<CapDisk>,
}

/// The `p`th percentile (0–100) by nearest rank; `None` for no values.
pub fn percentile(values: &[f32], p: f32) -> Option<f32> {
    let mut v: Vec<f32> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let rank = ((p / 100.0) * v.len() as f32).ceil().max(1.0) as usize;
    Some(v[rank.min(v.len()) - 1])
}

fn avg(values: &[f32]) -> Option<f32> {
    (!values.is_empty()).then(|| values.iter().sum::<f32>() / values.len() as f32)
}

fn round1(x: Option<f32>) -> Option<f32> {
    x.map(|v| (v * 10.0).round() / 10.0)
}

/// Mounts worth planning around: real filesystems of 1 GB or more, not boot
/// partitions.
pub fn worth(f: &MountForecast) -> bool {
    f.size >= GB && !f.mount.starts_with("/boot") && !f.mount.starts_with("/snap/")
}

pub fn summarize(host_id: HostId, spec: Option<HostSpec>, points: &[Point], disks: &[MountForecast]) -> HostCapacity {
    let cpu: Vec<f32> = points.iter().filter_map(|p| p.cpu).collect();
    let mem: Vec<f32> = points.iter().filter_map(|p| p.mem).collect();
    let gpu: Vec<f32> = points.iter().filter_map(|p| p.gpu).collect();
    HostCapacity {
        host_id,
        spec,
        cpu_avg: round1(avg(&cpu)),
        cpu_p95: round1(percentile(&cpu, 95.0)),
        mem_avg: round1(avg(&mem)),
        mem_p95: round1(percentile(&mem, 95.0)),
        gpu_p95: round1(percentile(&gpu, 95.0)),
        hours: cpu.len().max(mem.len()) as u32,
        disks: disks
            .iter()
            .filter(|f| worth(f))
            .map(|f| CapDisk { mount: f.mount.clone(), size: f.size, used: f.used, days_left: f.days_left })
            .collect(),
    }
}

/// What is running short, in words, for a notice. Needs three days of
/// history so a fresh host isn't flagged on a busy first hour.
pub fn pressure(c: &HostCapacity) -> Vec<String> {
    if c.hours < 72 {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(m) = c.mem_p95.filter(|m| *m >= MEM_TIGHT) {
        out.push(format!("memory {m:.0}% used at its busiest (p95)"));
    }
    if let Some(p) = c.cpu_p95.filter(|p| *p >= CPU_TIGHT) {
        out.push(format!("CPU {p:.0}% busy at its busiest (p95)"));
    }
    out
}

impl Store {
    pub fn host_spec_save(&self, s: &HostSpec) -> Result<()> {
        self.conn().execute(
            "INSERT INTO host_specs (host_id, cores, mem_total, at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(host_id) DO UPDATE SET cores = excluded.cores, mem_total = excluded.mem_total, at = excluded.at",
            params![s.host_id, s.cores, s.mem_total as i64, s.at],
        )?;
        Ok(())
    }

    pub fn host_specs(&self) -> Result<Vec<HostSpec>> {
        let mut st = self.conn().prepare("SELECT host_id, cores, mem_total, at FROM host_specs")?;
        let rows = st.query_map([], |r| {
            Ok(HostSpec {
                host_id: r.get(0)?,
                cores: r.get(1)?,
                mem_total: r.get::<_, i64>(2)?.max(0) as u64,
                at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn capacity_notified_at(&self, host: HostId) -> Result<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT notified_at FROM host_specs WHERE host_id = ?1", [host], |r| r.get(0))
            .optional()?
            .flatten())
    }

    pub fn capacity_notified(&self, host: HostId, at: i64) -> Result<()> {
        self.conn().execute("UPDATE host_specs SET notified_at = ?2 WHERE host_id = ?1", params![host, at])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(cpu: f32, mem: f32) -> Point {
        Point { cpu: Some(cpu), mem: Some(mem), ..Point::default() }
    }

    #[test]
    fn percentiles_and_pressure() {
        let v: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        assert_eq!(percentile(&v, 95.0), Some(95.0));
        assert_eq!(percentile(&[], 95.0), None);
        assert_eq!(percentile(&[7.0], 95.0), Some(7.0));
        let mut pts: Vec<Point> = (0..100).map(|_| pt(20.0, 60.0)).collect();
        let c = summarize(1, None, &pts, &[]);
        assert_eq!((c.cpu_p95, c.mem_p95, c.hours), (Some(20.0), Some(60.0), 100));
        assert!(pressure(&c).is_empty());
        for p in pts.iter_mut().take(10) {
            p.mem = Some(95.0);
        }
        let c = summarize(1, None, &pts, &[]);
        assert_eq!(c.mem_p95, Some(95.0));
        assert_eq!(pressure(&c), vec!["memory 95% used at its busiest (p95)"]);
        // Too little history.
        assert!(pressure(&summarize(1, None, &pts[..10], &[])).is_empty());
    }

    #[test]
    fn keeps_specs() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&crate::hosts::HostInput { name: "a".into(), hostname: "a".into(), ..Default::default() })
            .unwrap();
        s.host_spec_save(&HostSpec { host_id: h.id, cores: 8, mem_total: 32 * GB, at: 1 }).unwrap();
        s.host_spec_save(&HostSpec { host_id: h.id, cores: 16, mem_total: 64 * GB, at: 2 }).unwrap();
        assert_eq!(s.host_specs().unwrap()[0].cores, 16);
        assert_eq!(s.capacity_notified_at(h.id).unwrap(), None);
        s.capacity_notified(h.id, 5).unwrap();
        assert_eq!(s.capacity_notified_at(h.id).unwrap(), Some(5));
    }
}
