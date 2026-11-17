//! Power & cost (M12d): what each host draws and costs, estimated from a
//! per-host power profile (watts at idle and at full CPU) and its hourly CPU
//! history, plus measured GPU watts where the metrics have them. A UPS that
//! reports its output (NUT `ups.realpower`, or load × nominal) shows how
//! close the estimates come for the hosts on it. Hours of the day a host is
//! reliably idle become hints for scheduled power.
//!
//! Nothing new is polled: the hourly metric tier and the UPS readings the
//! alert loop already takes are read as they are.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::metrics::history::Point;
use crate::store::Store;
use crate::ups::UpsReading;
use crate::{Error, Result};

/// Hours in an average month.
pub const MONTH_HOURS: f32 = 730.0;
/// A host is idle in an hour when its CPU is below this (%) …
pub const IDLE_CPU: f32 = 5.0;
/// … and its network moves less than this (bytes/s, both ways together).
pub const IDLE_NET: f64 = 100_000.0;
/// An hour of the day counts as idle when it was idle on this share of days …
pub const IDLE_SHARE: f32 = 0.85;
/// … with data on at least this many days.
pub const MIN_DAYS: u32 = 5;
/// The shortest window worth a schedule.
pub const MIN_WINDOW: u32 = 3;
/// What a powered-off host with Wake-on-LAN still draws.
pub const STANDBY_W: f32 = 1.0;
pub const MAX_WATTS: f32 = 5000.0;

// ---------------------------------------------------------------- types

// Plain f32 fields are typed as numbers for TypeScript (specta's f32 allows
// NaN as null); the values are always finite here.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PowerProfile {
    pub host_id: HostId,
    /// Watts with the CPU idle.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub idle_w: f32,
    /// Watts with the CPU fully busy.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_w: f32,
}

/// Hours of the day (local time) a host is reliably idle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IdleWindow {
    /// First idle hour, 0–23.
    pub from: u32,
    /// The hour after the last idle one, 0–23 (wraps past midnight).
    pub to: u32,
    pub hours: u32,
    /// Share of days these hours were idle, 0–1 (the weakest hour's).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub share: f32,
    /// What switching it off for these hours would save a month.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub kwh_month: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostPower {
    pub host_id: HostId,
    pub name: String,
    pub profile: Option<PowerProfile>,
    /// Hours of history the numbers come from.
    pub hours: u32,
    pub avg_cpu: Option<f32>,
    /// Average GPU draw the metrics measured (watts), on top of the profile.
    pub gpu_w: Option<f32>,
    pub avg_w: Option<f32>,
    pub peak_w: Option<f32>,
    pub kwh_month: Option<f32>,
    pub idle: Vec<IdleWindow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpsPower {
    pub id: i32,
    pub label: String,
    /// What it supplies now, in watts.
    pub watts: Option<f32>,
    /// From `ups.realpower`; otherwise load × nominal power (an estimate).
    pub measured: bool,
    /// Seconds since the reading.
    pub age: u32,
    /// Hosts in the watch's group (the ones it powers).
    pub hosts: Vec<HostId>,
    /// The estimates of those hosts added up (hosts without a profile left out).
    pub estimated_w: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PowerOverview {
    pub days: u32,
    pub hosts: Vec<HostPower>,
    pub ups: Vec<UpsPower>,
}

// --------------------------------------------------------------- maths

pub fn check_profile(idle_w: f32, max_w: f32) -> Result<()> {
    let bad = |reason: &str| Err(Error::Invalid { field: "power", reason: reason.into() });
    if !idle_w.is_finite() || !max_w.is_finite() || idle_w < 0.0 || max_w <= 0.0 {
        return bad("watts must be positive numbers");
    }
    if max_w < idle_w {
        return bad("full load can't draw less than idle");
    }
    if max_w > MAX_WATTS {
        return bad("more than 5 kW: is that one host?");
    }
    Ok(())
}

/// Watts at a CPU load (%), on the line from idle to full.
pub fn watts(p: &PowerProfile, cpu: f32) -> f32 {
    p.idle_w + (p.max_w - p.idle_w) * (cpu.clamp(0.0, 100.0) / 100.0)
}

/// What a UPS supplies now, and whether it measured it.
pub fn ups_watts(r: &UpsReading) -> Option<(f32, bool)> {
    if let Some(w) = r.realpower.filter(|w| *w > 0.0) {
        return Some((w, true));
    }
    let load = r.load? / 100.0;
    if let Some(n) = r.realpower_nominal.filter(|n| *n > 0.0) {
        return Some((load * n, false));
    }
    // VA only: a typical power factor for small UPSes.
    r.power_nominal.filter(|n| *n > 0.0).map(|va| (load * va * 0.6, false))
}

fn net(p: &Point) -> Option<f64> {
    match (p.net_rx, p.net_tx) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
    }
}

/// Estimate for one host from its hourly points.
pub fn estimate(
    host_id: HostId,
    name: &str,
    profile: Option<PowerProfile>,
    pts: &[Point],
    utc_offset: i32,
) -> HostPower {
    let cpu: Vec<&Point> = pts.iter().filter(|p| p.cpu.is_some()).collect();
    let n = cpu.len() as f32;
    let avg_cpu = (n > 0.0).then(|| cpu.iter().map(|p| p.cpu.unwrap_or(0.0)).sum::<f32>() / n);
    let gpus: Vec<f32> = pts.iter().filter_map(|p| p.gpu_power).filter(|w| w.is_finite() && *w >= 0.0).collect();
    // Above the GPU's own lowest reading: its idle draw is part of the profile.
    let gpu_floor = gpus.iter().copied().fold(f32::INFINITY, f32::min);
    let gpu_w = (!gpus.is_empty()).then(|| gpus.iter().map(|w| w - gpu_floor).sum::<f32>() / gpus.len() as f32);
    let (avg_w, peak_w) = match (&profile, n > 0.0) {
        (Some(pr), true) => {
            let ws: Vec<f32> = cpu.iter().map(|p| watts(pr, p.cpu.unwrap_or(0.0))).collect();
            let avg = ws.iter().sum::<f32>() / n + gpu_w.unwrap_or(0.0);
            let peak = ws.iter().copied().fold(0.0, f32::max) + gpus.iter().map(|w| w - gpu_floor).fold(0.0, f32::max);
            (Some(avg), Some(peak))
        }
        // No history yet: the idle figure is the honest guess.
        (Some(pr), false) => (Some(pr.idle_w), None),
        _ => (None, None),
    };
    let idle = match &profile {
        Some(pr) => idle_windows(pts, pr, utc_offset),
        None => Vec::new(),
    };
    HostPower {
        host_id,
        name: name.to_owned(),
        profile,
        hours: n as u32,
        avg_cpu,
        gpu_w,
        avg_w,
        peak_w,
        kwh_month: avg_w.map(|w| w * MONTH_HOURS / 1000.0),
        idle,
    }
}

/// Hours of the local day that were idle on almost every day of the history,
/// merged into windows (wrapping past midnight) of at least [`MIN_WINDOW`].
pub fn idle_windows(pts: &[Point], profile: &PowerProfile, utc_offset: i32) -> Vec<IdleWindow> {
    // hour -> day -> (idle, cpu)
    let mut by_hour: Vec<BTreeMap<i64, (bool, f32)>> = vec![BTreeMap::new(); 24];
    for p in pts {
        let (Some(c), Some(nb)) = (p.cpu, net(p)) else { continue };
        let local = p.ts as i64 + i64::from(utc_offset);
        let h = local.div_euclid(3600).rem_euclid(24) as usize;
        by_hour[h].insert(local.div_euclid(86400), (c < IDLE_CPU && nb < IDLE_NET, c));
    }
    let stat: Vec<Option<(f32, f32)>> = by_hour
        .iter()
        .map(|days| {
            let n = days.len() as u32;
            if n < MIN_DAYS {
                return None;
            }
            let idle = days.values().filter(|(i, _)| *i).count() as f32 / n as f32;
            let cpu = days.values().map(|(_, c)| *c).sum::<f32>() / n as f32;
            (idle >= IDLE_SHARE).then_some((idle, cpu))
        })
        .collect();
    if stat.iter().all(Option::is_some) {
        return vec![window(&stat, 0, 24, profile)];
    }
    // Start right after a busy hour so a window never splits at midnight.
    let start = stat.iter().position(Option::is_none).unwrap_or(0);
    let mut out = Vec::new();
    let mut run: Option<usize> = None;
    for k in 1..=24 {
        let h = (start + k) % 24;
        match (stat[h].is_some(), run) {
            (true, None) => run = Some(h),
            (false, Some(from)) => {
                let len = (h + 24 - from) % 24;
                if len as u32 >= MIN_WINDOW {
                    out.push(window(&stat, from, len, profile));
                }
                run = None;
            }
            _ => {}
        }
    }
    out.sort_by_key(|w| w.from);
    out
}

fn window(stat: &[Option<(f32, f32)>], from: usize, len: usize, p: &PowerProfile) -> IdleWindow {
    let hs: Vec<(f32, f32)> = (0..len).filter_map(|i| stat[(from + i) % 24]).collect();
    let share = hs.iter().map(|(s, _)| *s).fold(1.0, f32::min);
    let saved_w: f32 = hs.iter().map(|(_, c)| (watts(p, *c) - STANDBY_W).max(0.0)).sum();
    IdleWindow {
        from: from as u32,
        to: ((from + len) % 24) as u32,
        hours: len as u32,
        share,
        kwh_month: saved_w * (MONTH_HOURS / 24.0) / 1000.0,
    }
}

// --------------------------------------------------------------- store

impl Store {
    pub fn power_profiles(&self) -> Result<Vec<PowerProfile>> {
        let mut st = self.conn().prepare("SELECT host_id, idle_w, max_w FROM host_power ORDER BY host_id")?;
        let rows =
            st.query_map([], |r| Ok(PowerProfile { host_id: r.get(0)?, idle_w: r.get(1)?, max_w: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn power_profile(&self, host_id: HostId) -> Result<Option<PowerProfile>> {
        Ok(self
            .conn()
            .query_row("SELECT host_id, idle_w, max_w FROM host_power WHERE host_id = ?1", [host_id], |r| {
                Ok(PowerProfile { host_id: r.get(0)?, idle_w: r.get(1)?, max_w: r.get(2)? })
            })
            .optional()?)
    }

    pub fn power_profile_set(&mut self, p: &PowerProfile) -> Result<()> {
        check_profile(p.idle_w, p.max_w)?;
        self.conn().execute(
            "INSERT INTO host_power (host_id, idle_w, max_w) VALUES (?1, ?2, ?3)
             ON CONFLICT(host_id) DO UPDATE SET idle_w = excluded.idle_w, max_w = excluded.max_w",
            params![p.host_id, p.idle_w, p.max_w],
        )?;
        Ok(())
    }

    pub fn power_profile_clear(&mut self, host_id: HostId) -> Result<()> {
        self.conn().execute("DELETE FROM host_power WHERE host_id = ?1", [host_id])?;
        Ok(())
    }

    /// Hourly points only, whatever the range.
    pub fn metric_hourly(&self, host_id: HostId, from: i64, to: i64) -> Result<Vec<Point>> {
        self.metric_history_at(host_id, crate::metrics::history::HOURLY, from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(ts: i64, cpu: f32, net: f64) -> Point {
        Point { ts: ts as f64, cpu: Some(cpu), net_rx: Some(net), net_tx: Some(0.0), ..Default::default() }
    }

    const P: PowerProfile = PowerProfile { host_id: 1, idle_w: 20.0, max_w: 120.0 };

    /// `days` of hourly points; `busy(h)` says which UTC hours are busy.
    fn history(days: i64, busy: impl Fn(i64) -> bool) -> Vec<Point> {
        (0..days * 24)
            .map(|i| {
                let h = i % 24;
                if busy(h) {
                    pt(i * 3600, 40.0, 2e6)
                } else {
                    pt(i * 3600, 1.0, 1e3)
                }
            })
            .collect()
    }

    #[test]
    fn line_from_idle_to_full() {
        assert_eq!(watts(&P, 0.0), 20.0);
        assert_eq!(watts(&P, 50.0), 70.0);
        assert_eq!(watts(&P, 150.0), 120.0);
        assert!(check_profile(20.0, 10.0).is_err());
        assert!(check_profile(-1.0, 10.0).is_err());
        assert!(check_profile(f32::NAN, 10.0).is_err());
        assert!(check_profile(0.0, 6000.0).is_err());
        assert!(check_profile(4.0, 15.0).is_ok());
    }

    #[test]
    fn estimates_a_month() {
        let pts = vec![pt(0, 0.0, 0.0), pt(3600, 100.0, 0.0)];
        let h = estimate(1, "server01", Some(P), &pts, 0);
        assert_eq!(h.avg_w, Some(70.0));
        assert_eq!(h.peak_w, Some(120.0));
        assert_eq!(h.kwh_month, Some(70.0 * 730.0 / 1000.0));
        assert_eq!(h.hours, 2);
        let none = estimate(2, "nas", None, &pts, 0);
        assert_eq!(none.avg_w, None);
        assert_eq!(none.avg_cpu, Some(50.0));
        // No history yet: idle watts.
        assert_eq!(estimate(1, "x", Some(P), &[], 0).avg_w, Some(20.0));
    }

    #[test]
    fn gpu_adds_what_it_draws_above_idle() {
        let mut a = pt(0, 0.0, 0.0);
        a.gpu_power = Some(15.0);
        let mut b = pt(3600, 0.0, 0.0);
        b.gpu_power = Some(215.0);
        let h = estimate(1, "gpu", Some(P), &[a, b], 0);
        assert_eq!(h.gpu_w, Some(100.0));
        assert_eq!(h.avg_w, Some(120.0));
        assert_eq!(h.peak_w, Some(220.0));
    }

    #[test]
    fn finds_a_night_window_across_midnight() {
        // Busy 07:00–23:00 UTC, idle 23:00–07:00.
        let pts = history(10, |h| (7..23).contains(&h));
        let w = idle_windows(&pts, &P, 0);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].from, w[0].to, w[0].hours), (23, 7, 8));
        assert_eq!(w[0].share, 1.0);
        // 8 hours × (watts at 1 % − standby) × 30.4 days.
        let per_hour = watts(&P, 1.0) - STANDBY_W;
        assert!((w[0].kwh_month - per_hour * 8.0 * 730.0 / 24.0 / 1000.0).abs() < 1e-3);
        // The same in UTC+2 local time.
        let w = idle_windows(&pts, &P, 7200);
        assert_eq!((w[0].from, w[0].to), (1, 9));
    }

    #[test]
    fn needs_enough_days_and_consistency() {
        assert!(idle_windows(&history(3, |h| (7..23).contains(&h)), &P, 0).is_empty());
        // Idle at night but busy every third night: not reliable.
        let mut pts = history(9, |h| (7..23).contains(&h));
        for p in pts.iter_mut() {
            let day = p.ts as i64 / 86400;
            if day % 3 == 0 && (p.ts as i64 / 3600) % 24 == 2 {
                p.cpu = Some(60.0);
            }
        }
        let w = idle_windows(&pts, &P, 0);
        // 02:00 drops out, leaving 23–02 (3 h) and 03–07 (4 h).
        assert_eq!(w.iter().map(|w| (w.from, w.hours)).collect::<Vec<_>>(), vec![(3, 4), (23, 3)]);
        // Two-hour gaps don't make a window.
        assert!(idle_windows(&history(9, |h| h % 3 != 0), &P, 0).is_empty());
    }

    #[test]
    fn idle_all_day_is_one_window() {
        let w = idle_windows(&history(7, |_| false), &P, 0);
        assert_eq!((w[0].from, w[0].hours), (0, 24));
    }

    #[test]
    fn ups_output() {
        let mut r = UpsReading { load: Some(25.0), ..Default::default() };
        assert_eq!(ups_watts(&r), None);
        r.power_nominal = Some(1000.0);
        assert_eq!(ups_watts(&r), Some((150.0, false)));
        r.realpower_nominal = Some(900.0);
        assert_eq!(ups_watts(&r), Some((225.0, false)));
        r.realpower = Some(210.0);
        assert_eq!(ups_watts(&r), Some((210.0, true)));
    }

    #[test]
    fn profiles_round_trip() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&crate::hosts::HostInput {
                name: "server01".into(),
                hostname: "10.0.0.2".into(),
                ..Default::default()
            })
            .unwrap();
        s.power_profile_set(&PowerProfile { host_id: h.id, idle_w: 9.0, max_w: 35.0 }).unwrap();
        s.power_profile_set(&PowerProfile { host_id: h.id, idle_w: 10.0, max_w: 40.0 }).unwrap();
        assert_eq!(s.power_profile(h.id).unwrap().unwrap().max_w, 40.0);
        assert!(s.power_profile_set(&PowerProfile { host_id: h.id, idle_w: 50.0, max_w: 40.0 }).is_err());
        s.delete_host(h.id).unwrap();
        assert!(s.power_profiles().unwrap().is_empty());
    }
}
