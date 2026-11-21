//! Disk usage trend (M10m): hourly usage per mount, recorded while a host is
//! monitored, and a straight-line forecast of when each mount fills up.

use std::collections::{BTreeMap, HashSet};

use rusqlite::params;
use serde::Serialize;

use crate::alerts::{AlertKind, Condition, Severity};
use crate::error::Result;
use crate::hosts::HostId;
use crate::metrics::MountUsage;
use crate::mountkind::{classify, host_names, server_is, within, MountInfo, MountKind};
use crate::storagelink::StorageLink;
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

/// Who owns the disk behind a mount that isn't this host's own.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MountOwner {
    pub host_id: HostId,
    pub host: String,
    /// The owner's mount that holds it.
    pub mount: String,
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
    /// `/proc/mounts` device (`/dev/sda1`, `nas:/volume1/plex`); empty for
    /// history recorded before D-200.
    pub device: String,
    pub fstype: String,
    pub kind: MountKind,
    /// Chip words ("NFS · nas", "Windows drive H:", "ZFS tank"); empty for
    /// a plain local disk.
    pub label: String,
    /// Other mounts of the same storage on this host, forecast as one
    /// (datasets of one pool, the same share mounted twice).
    pub also: Vec<String>,
    /// The host whose disk this really is, when it's another saved host.
    pub owner: Option<MountOwner>,
    /// Other hosts that mount this storage over the network.
    pub mounted_on: Vec<String>,
    /// Hosts whose disks are stored here (storage links: VM disk images).
    pub stores: Vec<String>,
    /// Where this host's own disks are stored, when linked (a VM's image on
    /// its hypervisor's drive).
    pub stored_on: Option<MountOwner>,
    /// False when the growth is counted on another host: no FILLING UP or
    /// DISK FULL alert here, and the row is shown as a reference.
    pub counted: bool,
    /// One sentence on what this mount is and where its growth is counted.
    pub note: Option<String>,
}

/// One recorded hour of one mount.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub mount: String,
    pub ts: i64,
    pub used: u64,
    pub size: u64,
    pub device: String,
    pub fstype: String,
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
                "INSERT OR REPLACE INTO disk_usage (host_id, mount, ts, used, size, device, fstype)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![host, m.mount, ts, m.used as i64, size as i64, m.device, m.fstype],
            )?;
        }
        c.execute("DELETE FROM disk_usage WHERE host_id = ?1 AND ts < ?2", params![host, now - KEEP_DAYS * 86_400])?;
        Ok(())
    }

    /// Rows for `host` since `since`, by mount then time.
    pub fn disk_usage_since(&self, host: HostId, since: i64) -> Result<Vec<UsageRow>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT mount, ts, used, size, device, fstype FROM disk_usage
             WHERE host_id = ?1 AND ts >= ?2 ORDER BY mount, ts",
        )?;
        let rows = st.query_map(params![host, since], |r| {
            Ok(UsageRow {
                mount: r.get(0)?,
                ts: r.get(1)?,
                used: r.get::<_, i64>(2)? as u64,
                size: r.get::<_, i64>(3)? as u64,
                device: r.get(4)?,
                fstype: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Forecasts for every mount of one host with recorded history, seen
    /// against the rest of the fleet (shares counted on their owner).
    pub fn disk_forecasts(&self, host: HostId, now: i64) -> Result<Vec<MountForecast>> {
        Ok(self.fleet_disk_forecasts(now)?.remove(&host).unwrap_or_default())
    }

    /// Forecasts for every saved host, with network shares and passed-in
    /// drives tied to the host that owns the disk and counted once.
    pub fn fleet_disk_forecasts(&self, now: i64) -> Result<BTreeMap<HostId, Vec<MountForecast>>> {
        let mut fleet = Vec::new();
        for h in self.list_hosts()? {
            let rows = self.disk_usage_since(h.id, now - DAILY_MAX as i64 * 86_400)?;
            let names = host_names(
                [h.name.as_str(), h.hostname.as_str()]
                    .into_iter()
                    .chain(h.tailnet_address.as_deref())
                    .chain(h.ssh_alias.as_deref()),
            );
            fleet.push(FleetHost {
                id: h.id,
                name: h.name,
                names,
                windows: h.os == crate::hosts::OS_WINDOWS,
                forecasts: forecasts(&rows, now),
            });
        }
        annotate(&mut fleet, now);
        apply_links(&mut fleet, &self.storage_links()?);
        Ok(fleet.into_iter().map(|h| (h.id, h.forecasts)).collect())
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

/// One mount's history, newest device and type.
struct Series {
    mount: String,
    device: String,
    fstype: String,
    info: MountInfo,
    points: Vec<UsagePoint>,
    /// `(ts, size)` per point.
    sizes: Vec<u64>,
}

impl Series {
    fn last(&self) -> (i64, u64, u64) {
        let i = self.points.len() - 1;
        (self.points[i].ts, self.points[i].used, self.sizes[i])
    }
}

fn close(a: u64, b: u64, per_mille: u64) -> bool {
    a.abs_diff(b) <= a.max(b) / 1000 * per_mille
}

/// Groups rows (sorted by mount, then time) into forecasts. Datasets of one
/// pool are summed and forecast as the pool; mounts of the same storage
/// (equal size and usage at the same hour) are forecast once.
pub fn forecasts(rows: &[UsageRow], now: i64) -> Vec<MountForecast> {
    let mut series: Vec<Series> = Vec::new();
    for r in rows {
        match series.last_mut() {
            Some(s) if s.mount == r.mount => {
                s.points.push(UsagePoint { ts: r.ts, used: r.used });
                s.sizes.push(r.size);
                if !r.device.is_empty() {
                    s.device.clone_from(&r.device);
                    s.fstype.clone_from(&r.fstype);
                }
            }
            _ => series.push(Series {
                mount: r.mount.clone(),
                device: r.device.clone(),
                fstype: r.fstype.clone(),
                info: classify(&r.mount, "", ""),
                points: vec![UsagePoint { ts: r.ts, used: r.used }],
                sizes: vec![r.size],
            }),
        }
    }
    for s in &mut series {
        s.info = classify(&s.mount, &s.device, &s.fstype);
    }
    // Shortest path first, so a group's first member names it.
    series.sort_by(|a, b| a.mount.len().cmp(&b.mount.len()).then_with(|| a.mount.cmp(&b.mount)));

    let mut out = Vec::new();
    let mut done = vec![false; series.len()];
    for i in 0..series.len() {
        if done[i] {
            continue;
        }
        done[i] = true;
        let s = &series[i];
        let mut members = vec![i];
        if s.info.kind == MountKind::Pool {
            for (j, o) in series.iter().enumerate().skip(i + 1) {
                if !done[j] && o.info.kind == MountKind::Pool && o.info.group == s.info.group {
                    done[j] = true;
                    members.push(j);
                }
            }
        } else {
            let (ts, used, size) = s.last();
            for (j, o) in series.iter().enumerate().skip(i + 1) {
                let (ots, oused, osize) = o.last();
                if !done[j]
                    && o.info.kind != MountKind::Pool
                    && size >= 1_000_000_000
                    && ots == ts
                    && osize == size
                    && oused == used
                {
                    done[j] = true;
                    members.push(j);
                }
            }
        }
        // A plain local disk names a mirror group when there is one.
        let lead = *members.iter().find(|&&m| series[m].info.kind == MountKind::Local).unwrap_or(&members[0]);
        let (points, sizes) = if s.info.kind == MountKind::Pool && members.len() > 1 {
            pooled(&series, &members)
        } else {
            (series[lead].points.clone(), series[lead].sizes.clone())
        };
        let l = &series[lead];
        out.push(forecast_of(
            l,
            &points,
            &sizes,
            members.iter().filter(|&&m| m != lead).map(|&m| series[m].mount.clone()).collect(),
            now,
        ));
    }
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out
}

/// A pool's history: per hour, the datasets' usage summed and the largest
/// free space any of them reports (they share it).
fn pooled(series: &[Series], members: &[usize]) -> (Vec<UsagePoint>, Vec<u64>) {
    let mut by_ts: BTreeMap<i64, (u64, u64)> = BTreeMap::new();
    for &m in members {
        let s = &series[m];
        for (p, size) in s.points.iter().zip(&s.sizes) {
            let e = by_ts.entry(p.ts).or_default();
            e.0 += p.used;
            e.1 = e.1.max(size.saturating_sub(p.used));
        }
    }
    by_ts.into_iter().map(|(ts, (used, avail))| (UsagePoint { ts, used }, used + avail)).unzip()
}

fn forecast_of(s: &Series, points: &[UsagePoint], sizes: &[u64], also: Vec<String>, now: i64) -> MountForecast {
    let last = points[points.len() - 1];
    let size = sizes[sizes.len() - 1];
    let (per_day, span_days) = trend(points, now);
    let mut daily: Vec<UsagePoint> = Vec::new();
    for p in points {
        match daily.last_mut() {
            Some(d) if d.ts.div_euclid(86_400) == p.ts.div_euclid(86_400) => *d = *p,
            _ => daily.push(*p),
        }
    }
    if daily.len() > DAILY_MAX {
        daily.drain(..daily.len() - DAILY_MAX);
    }
    let note = match s.info.kind {
        MountKind::Pool if !also.is_empty() => Some(format!(
            "{} datasets share the free space of {}, so they're forecast together.",
            also.len() + 1,
            s.info.label
        )),
        MountKind::Passthrough if s.info.group.is_some() => Some(format!(
            "A Windows drive mounted into WSL. The disk belongs to Windows, so SMART isn't read here, and anything on Windows that saves to {} (VM disk images are a common one) grows it.",
            s.info.group.as_deref().unwrap_or("it")
        )),
        MountKind::Passthrough => Some(
            "A folder shared in from the machine this VM runs on. Its disk and SMART belong to that machine.".into(),
        ),
        MountKind::Network => Some(match &s.info.server {
            Some(server) => format!(
                "A share from {server}: the space is used on that machine. Add it as a host to see its disks, SMART and what's growing there."
            ),
            None => "A network share: the space is used on the machine that serves it.".into(),
        }),
        _ => None,
    };
    MountForecast {
        mount: s.mount.clone(),
        size,
        used: last.used,
        at: last.ts,
        per_day,
        days_left: days_left(last.used, size, per_day),
        span_days,
        daily,
        device: s.device.clone(),
        fstype: s.fstype.clone(),
        kind: s.info.kind,
        label: s.info.label.clone(),
        also,
        owner: None,
        mounted_on: Vec::new(),
        stores: Vec::new(),
        stored_on: None,
        counted: true,
        note,
    }
}

/// "FILLING UP" when a mount is forecast to be full within `within_days`
/// (critical inside three days). Mounts whose newest point is older than a
/// day are left out (the host isn't being watched), and so are mounts
/// counted on another host.
pub fn conditions(host: HostId, name: &str, list: &[MountForecast], within_days: u16, now: i64) -> Vec<Condition> {
    list.iter()
        .filter(|f| f.counted && now - f.at <= 86_400)
        .filter_map(|f| {
            let d = f.days_left?;
            let mut what = if f.label.is_empty() { f.mount.clone() } else { format!("{} ({})", f.mount, f.label) };
            if !f.stores.is_empty() {
                what = format!("{what}, which holds the disks of {},", f.stores.join(", "));
            }
            (d < f64::from(within_days)).then(|| Condition {
                key: format!("fill:{host}:{}", f.mount),
                kind: AlertKind::DiskFilling,
                severity: if d < 3.0 { Severity::Critical } else { Severity::Warn },
                title: name.to_owned(),
                detail: format!("{what} full in about {} at the current rate", days_words(d)),
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

/// A saved host and its forecasts, for [`annotate`].
pub struct FleetHost {
    pub id: HostId,
    pub name: String,
    /// What the network may call it ([`host_names`]).
    pub names: Vec<String>,
    pub windows: bool,
    pub forecasts: Vec<MountForecast>,
}

/// Where a share or passed-in drive lives in the fleet: `(host index,
/// forecast index)` of the owner's mount.
fn find_owner(fleet: &[FleetHost], me: usize, f: &MountForecast, now: i64) -> Option<(usize, usize)> {
    let info = classify(&f.mount, &f.device, &f.fstype);
    let fresh = |o: &MountForecast| now - o.at <= 86_400 && matches!(o.kind, MountKind::Local | MountKind::Pool);
    let mut best: Option<(usize, usize, usize)> = None;
    for (hi, h) in fleet.iter().enumerate() {
        if hi == me {
            continue;
        }
        for (fi, o) in h.forecasts.iter().enumerate() {
            if !fresh(o) {
                continue;
            }
            let same_space = close(o.size, f.size, 10) && close(o.used, f.used, 20);
            let score = match f.kind {
                MountKind::Network => {
                    let named = info.server.as_deref().is_some_and(|s| server_is(s, &h.names));
                    let export = info.export.as_deref().unwrap_or("");
                    let path = std::iter::once(&o.mount)
                        .chain(&o.also)
                        .filter(|m| within(export, m) || within(&format!("/{export}"), m))
                        .map(String::len)
                        .max();
                    match (named, path) {
                        (true, Some(len)) => 2000 + len,
                        // An SMB share name rarely matches a path: the space tells.
                        (true, None) if same_space => 1000,
                        (false, Some(len)) if same_space && len > 1 => 500 + len,
                        _ => continue,
                    }
                }
                // WSL's H: and the Windows host's own H:\, if it's saved too.
                MountKind::Passthrough => match &info.group {
                    Some(letter)
                        if h.windows && o.mount.to_ascii_uppercase().starts_with(letter.as_str()) && same_space =>
                    {
                        1000
                    }
                    _ => continue,
                },
                _ => continue,
            };
            if best.map_or(true, |b| score > b.2) {
                best = Some((hi, fi, score));
            }
        }
    }
    best.map(|(h, f, _)| (h, f))
}

/// Ties shares and passed-in drives to the host that owns the disk, so
/// growth is counted (and alerted) once: on the owner. The same share
/// mounted on several hosts with no owner saved is counted on the first.
pub fn annotate(fleet: &mut [FleetHost], now: i64) {
    let mut links: Vec<(usize, usize, usize, usize)> = Vec::new();
    for (hi, h) in fleet.iter().enumerate() {
        for (fi, f) in h.forecasts.iter().enumerate() {
            if matches!(f.kind, MountKind::Network | MountKind::Passthrough) {
                if let Some((oh, of)) = find_owner(fleet, hi, f, now) {
                    links.push((hi, fi, oh, of));
                }
            }
        }
    }
    for (hi, fi, oh, of) in links {
        let (owner_name, owner_id, owner_mount) =
            (fleet[oh].name.clone(), fleet[oh].id, fleet[oh].forecasts[of].mount.clone());
        let me = fleet[hi].name.clone();
        let f = &mut fleet[hi].forecasts[fi];
        f.counted = false;
        f.note = Some(match f.kind {
            MountKind::Passthrough => format!(
                "A Windows drive mounted into WSL: it's {owner_mount} on {owner_name}, where its growth and SMART are counted."
            ),
            _ => format!("Shared from {owner_name} ({owner_mount}): its growth is counted there, not here."),
        });
        f.owner = Some(MountOwner { host_id: owner_id, host: owner_name, mount: owner_mount });
        let o = &mut fleet[oh].forecasts[of];
        if !o.mounted_on.contains(&me) {
            o.mounted_on.push(me);
        }
    }
    for h in fleet.iter_mut() {
        for f in &mut h.forecasts {
            if !f.mounted_on.is_empty() && f.note.is_none() {
                f.note = Some(format!(
                    "Also mounted on {} over the network: growth from there lands here.",
                    f.mounted_on.join(", ")
                ));
            }
        }
    }

    // Shares with no saved owner: the same storage seen from several hosts
    // is counted on the first that reports it.
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for hi in 0..fleet.len() {
        for fi in 0..fleet[hi].forecasts.len() {
            let f = &fleet[hi].forecasts[fi];
            if f.kind != MountKind::Network || !f.counted || now - f.at > 86_400 {
                continue;
            }
            let server = classify(&f.mount, &f.device, &f.fstype).server;
            let first = seen.iter().copied().find(|&(sh, sf)| {
                let o = &fleet[sh].forecasts[sf];
                sh != hi
                    && classify(&o.mount, &o.device, &o.fstype).server == server
                    && close(o.size, f.size, 10)
                    && close(o.used, f.used, 20)
            });
            match first {
                Some((sh, sf)) => {
                    let (host, mount) = (fleet[sh].name.clone(), fleet[sh].forecasts[sf].mount.clone());
                    let f = &mut fleet[hi].forecasts[fi];
                    f.counted = false;
                    f.note = Some(format!("The same share as {mount} on {host}: counted there."));
                }
                None => seen.push((hi, fi)),
            }
        }
    }
}

/// Marks storage links: the owner's drive lists the hosts stored on it and
/// each linked host's own disks say where they live.
pub fn apply_links(fleet: &mut [FleetHost], links: &[StorageLink]) {
    for l in links {
        let Some(child) = fleet.iter().position(|h| h.id == l.host_id) else { continue };
        let Some(oh) = fleet.iter().position(|h| h.id == l.owner_host_id) else { continue };
        let child_name = fleet[child].name.clone();
        let owner = MountOwner { host_id: l.owner_host_id, host: fleet[oh].name.clone(), mount: l.owner_mount.clone() };
        if let Some(o) = fleet[oh]
            .forecasts
            .iter_mut()
            .find(|f| same_mount(&f.mount, &l.owner_mount) || f.also.iter().any(|m| same_mount(m, &l.owner_mount)))
        {
            if !o.stores.contains(&child_name) {
                o.stores.push(child_name);
            }
        }
        for f in &mut fleet[child].forecasts {
            if matches!(f.kind, MountKind::Local | MountKind::Pool) && f.stored_on.is_none() {
                f.stored_on = Some(owner.clone());
                if f.note.is_none() {
                    f.note =
                        Some(format!("Stored on {} ({}): growth here grows that drive too.", owner.host, owner.mount));
                }
            }
        }
    }
    for h in fleet.iter_mut() {
        for f in &mut h.forecasts {
            if !f.stores.is_empty() {
                let words = format!("Holds the disks of {}: growth inside them lands here.", f.stores.join(", "));
                f.note = Some(match f.note.take() {
                    Some(n) => format!("{n} {words}"),
                    None => words,
                });
            }
        }
    }
}

/// `H:\` and `H:`, `/mnt/h/` and `/mnt/h`.
fn same_mount(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let t = s.trim().trim_end_matches(['/', '\\']);
        if t.is_empty() {
            s.trim().to_owned()
        } else {
            t.to_ascii_lowercase()
        }
    };
    norm(a) == norm(b)
}

/// `(host, mount)` pairs whose space is counted elsewhere (another host,
/// or the first mount of the same storage here), for DISK FULL.
pub fn counted_elsewhere(fleet: &BTreeMap<HostId, Vec<MountForecast>>) -> HashSet<(HostId, String)> {
    let mut out = HashSet::new();
    for (id, list) in fleet {
        for f in list {
            if !f.counted {
                out.insert((*id, f.mount.clone()));
            }
            for m in &f.also {
                out.insert((*id, m.clone()));
            }
        }
    }
    out
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

    fn row(mount: &str, device: &str, fstype: &str, ts: i64, used: u64, size: u64) -> UsageRow {
        UsageRow { mount: mount.into(), ts, used, size, device: device.into(), fstype: fstype.into() }
    }

    /// 48 hourly rows of `mount` growing `per_hour` from `used`.
    fn grow(mount: &str, device: &str, fstype: &str, now: i64, used: u64, per_hour: u64, size: u64) -> Vec<UsageRow> {
        (0..48).map(|k| row(mount, device, fstype, now - (47 - k) * 3600, used + k as u64 * per_hour, size)).collect()
    }

    fn sorted(mut rows: Vec<UsageRow>) -> Vec<UsageRow> {
        rows.sort_by(|a, b| a.mount.cmp(&b.mount).then(a.ts.cmp(&b.ts)));
        rows
    }

    #[test]
    fn pools_and_mirrors_forecast_once() {
        let now = 1_700_000_000 - 1_700_000_000 % 3600;
        let mut rows = Vec::new();
        // Two ZFS datasets share 100 GB free; each grows 1 GB an hour.
        rows.extend((0..48).map(|k| {
            row("/tank/a", "tank/a", "zfs", now - (47 - k) * 3600, 10 * GB + k as u64 * GB, 200 * GB - k as u64 * GB)
        }));
        rows.extend((0..48).map(|k| {
            row("/tank", "tank", "zfs", now - (47 - k) * 3600, 50 * GB + k as u64 * GB, 240 * GB - k as u64 * GB)
        }));
        // The same Windows drive seen twice (WSL and a Docker Desktop bind).
        rows.extend(grow("/mnt/h", "H:\\134", "9p", now, 500 * GB, GB / 4, 2000 * GB));
        rows.extend(grow("/mnt/wsl/h", "drvfs", "9p", now, 500 * GB, GB / 4, 2000 * GB));
        let f = forecasts(&sorted(rows), now);
        assert_eq!(f.iter().map(|m| m.mount.as_str()).collect::<Vec<_>>(), ["/mnt/h", "/tank"]);
        let tank = &f[1];
        assert_eq!((tank.kind, tank.also.clone()), (MountKind::Pool, vec!["/tank/a".to_owned()]));
        assert_eq!(tank.used, (10 + 47 + 50 + 47) * GB);
        assert!((tank.per_day.unwrap() / GB as f64 - 48.0).abs() < 0.01, "both datasets' growth");
        let h = &f[0];
        assert_eq!(
            (h.kind, h.label.as_str(), h.also.clone()),
            (MountKind::Passthrough, "Windows drive H:", vec!["/mnt/wsl/h".to_owned()])
        );
        assert!(h.note.as_deref().unwrap().contains("SMART isn't read here"));
        let elsewhere = counted_elsewhere(&BTreeMap::from([(1, f.clone())]));
        assert!(elsewhere.contains(&(1, "/mnt/wsl/h".to_owned())) && elsewhere.contains(&(1, "/tank/a".to_owned())));
    }

    #[test]
    fn shares_count_on_their_owner() {
        let now = 1_700_000_000 - 1_700_000_000 % 3600;
        let host = |id, name: &str, names: &[&str], windows, rows: Vec<UsageRow>| FleetHost {
            id,
            name: name.into(),
            names: host_names(names.iter().copied()),
            windows,
            forecasts: forecasts(&sorted(rows), now),
        };
        let nas = grow("/volume1", "/dev/md2", "btrfs", now, 3000 * GB, GB, 4000 * GB);
        let mut client = grow("/mnt/plex", "192.168.0.129:/volume1/plex", "nfs4", now, 3000 * GB, GB, 4000 * GB);
        client.extend(grow("/", "/dev/sda2", "ext4", now, 20 * GB, 0, 100 * GB));
        // Another client mounts a share from a NAS nobody saved, as does a third.
        let other = grow("/srv/media", "files.lan:/export", "nfs", now, 700 * GB, 2 * GB, 1000 * GB);
        let third = grow("/mnt/media", "files.lan:/export/media", "nfs", now, 700 * GB, 2 * GB, 1000 * GB);
        // WSL's H: and the same drive on the saved Windows machine.
        let wsl = grow("/mnt/h", "H:\\134", "9p", now, 900 * GB, GB / 2, 2000 * GB);
        let win = grow("H:\\", "H:", "NTFS", now, 900 * GB, GB / 2, 2000 * GB);
        let mut fleet = vec![
            host(1, "Oracle", &["oracle.lan", "192.168.0.129"], false, nas),
            host(2, "swizzin", &["swizzin"], false, client),
            host(3, "box", &["box"], false, other),
            host(4, "box2", &["box2"], false, third),
            host(5, "wsl", &["wsl"], false, wsl),
            host(6, "desk", &["desk"], true, win),
        ];
        annotate(&mut fleet, now);
        let plex = &fleet[1].forecasts[1];
        assert_eq!(plex.mount, "/mnt/plex");
        assert!(!plex.counted);
        assert_eq!(plex.owner.as_ref().map(|o| (o.host.as_str(), o.mount.as_str())), Some(("Oracle", "/volume1")));
        assert_eq!(fleet[0].forecasts[0].mounted_on, ["swizzin"]);
        assert!(fleet[0].forecasts[0].counted);
        assert!(fleet[1].forecasts[0].counted, "the client's own disk");
        assert!(fleet[2].forecasts[0].counted && fleet[2].forecasts[0].owner.is_none());
        assert!(!fleet[3].forecasts[0].counted, "same unsaved share as on box");
        assert!(!fleet[4].forecasts[0].counted);
        assert_eq!(fleet[4].forecasts[0].owner.as_ref().map(|o| o.host.as_str()), Some("desk"));

        let list = fleet[1].forecasts.clone();
        assert!(conditions(2, "swizzin", &list, 9999, now).is_empty(), "counted on the NAS");
        let c = conditions(3, "box", &fleet[2].forecasts, 9999, now);
        assert_eq!(c.len(), 1);
        assert!(c[0].detail.starts_with("/srv/media (NFS · files.lan) full in about"), "{}", c[0].detail);
    }

    #[test]
    fn links_mark_both_ends() {
        let now = 1_700_000_000 - 1_700_000_000 % 3600;
        let h = |id, name: &str, rows: Vec<UsageRow>| FleetHost {
            id,
            name: name.into(),
            names: vec![],
            windows: false,
            forecasts: forecasts(&sorted(rows), now),
        };
        let mut fleet = vec![
            h(1, "hyper", grow("/mnt/h", "/dev/sdb1", "ext4", now, 900 * GB, GB, 2000 * GB)),
            h(2, "web01", grow("/", "/dev/vda1", "ext4", now, 20 * GB, GB / 10, 60 * GB)),
        ];
        apply_links(&mut fleet, &[StorageLink { id: 1, host_id: 2, owner_host_id: 1, owner_mount: "/mnt/h/".into() }]);
        assert_eq!(fleet[0].forecasts[0].stores, ["web01"]);
        assert!(fleet[0].forecasts[0].note.as_deref().unwrap().contains("Holds the disks of web01"));
        let own = &fleet[1].forecasts[0];
        assert_eq!(own.stored_on.as_ref().map(|o| o.host.as_str()), Some("hyper"));
        assert!(own.counted, "growth inside the VM still fills the VM's own disk");
        let c = conditions(1, "hyper", &fleet[0].forecasts, 9999, now);
        assert!(c[0].detail.starts_with("/mnt/h, which holds the disks of web01, full in about"), "{}", c[0].detail);
    }
}
