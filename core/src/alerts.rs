//! Alert rules: turns what the app already watches (metrics poll states, disk
//! usage, service checks, container states, SMART) into alerts with grace
//! periods, and keeps the active set plus a short history.
//!
//! Sources report *conditions*: "this is wrong right now". The [`Tracker`]
//! decides when a condition becomes an alert (after its kind's grace period)
//! and when an alert is resolved (after the condition has been gone for a
//! while, so a disk hovering at the threshold doesn't flap). Everything here is
//! pure and clock-injected; the app crate owns the loop and the delivery.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::docker::ContainerView;
use crate::hosts::HostId;
use crate::metrics::MountUsage;
use crate::services::check::CheckResult;
use crate::system::smart::{Disk, Health};

/// Resolved alerts kept for the history list.
pub const HISTORY_MAX: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AlertKind {
    HostOffline,
    DiskFull,
    ContainerDown,
    ServiceDown,
    CertExpiring,
    SmartWarning,
    BackupFailed,
    BackupStale,
    LogBurst,
    LogNewError,
    /// A watched certificate file, domain, token or licence (M10l).
    ExpirySoon,
    /// A mount forecast to be full soon at its current growth (M10m).
    DiskFilling,
    /// A ZFS pool, RAID array or btrfs filesystem lost a device or has errors (M10m).
    PoolDegraded,
    /// A UPS runs on battery, is low, or needs a new battery (M10s).
    UpsOnBattery,
    /// A saved log search with alerts on found new lines (M10u).
    LogMatch,
    /// The network watch saw a device for the first time (D-145).
    NetNewDevice,
    /// A device in the expected network map didn't answer (D-145).
    NetMissing,
    /// Risky exposure the network watch found: a port forward to SSH or RDP,
    /// a second DHCP server (D-145).
    NetRisk,
}

impl AlertKind {
    /// Seconds a condition must hold before it fires.
    pub fn fire_after(self) -> i64 {
        match self {
            // Metrics retries back off from 5 s; two minutes rides out a reboot
            // of the network or a host that is just restarting sshd.
            AlertKind::HostOffline => 120,
            // One failed check can be a blip; the next run (60 s default) confirms.
            AlertKind::ServiceDown => 90,
            _ => 0,
        }
    }

    /// Seconds a condition must be gone before the alert resolves.
    pub fn clear_after(self) -> i64 {
        match self {
            AlertKind::DiskFull => 300,
            // A forecast wobbles around the threshold as usage moves.
            AlertKind::DiskFilling => 3 * 3600,
            _ => 0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AlertKind::HostOffline => "HOST OFFLINE",
            AlertKind::DiskFull => "DISK FULL",
            AlertKind::ContainerDown => "CONTAINER DOWN",
            AlertKind::ServiceDown => "SERVICE DOWN",
            AlertKind::CertExpiring => "CERT EXPIRING",
            AlertKind::SmartWarning => "SMART WARNING",
            AlertKind::BackupFailed => "BACKUP FAILED",
            AlertKind::BackupStale => "BACKUP STALE",
            AlertKind::LogBurst => "ERROR BURST",
            AlertKind::LogNewError => "NEW ERROR",
            AlertKind::ExpirySoon => "EXPIRING",
            AlertKind::DiskFilling => "FILLING UP",
            AlertKind::PoolDegraded => "POOL DEGRADED",
            AlertKind::UpsOnBattery => "ON BATTERY",
            AlertKind::LogMatch => "LOG MATCH",
            AlertKind::NetNewDevice => "NEW DEVICE",
            AlertKind::NetMissing => "DEVICE MISSING",
            AlertKind::NetRisk => "NETWORK RISK",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Warn,
    Critical,
}

/// Something wrong right now, as a source sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// Stable identity, e.g. `disk:3:/srv`. Prefixed with the source name.
    pub key: String,
    pub kind: AlertKind,
    pub severity: Severity,
    /// Short subject: a host, service or container name.
    pub title: String,
    pub detail: String,
    pub host_id: Option<HostId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: u32,
    pub key: String,
    pub kind: AlertKind,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub host_id: Option<HostId>,
    /// When the condition was first seen (unix seconds).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub since: i64,
    /// When it resolved (unix seconds); `None` while active.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub resolved: Option<i64>,
    /// The active alert for the outage upstream that explains this one
    /// (M10s, [`crate::depgraph::fold`]). Filled in for the snapshot only.
    #[serde(default)]
    pub caused_by: Option<u32>,
    /// The acknowledgement or mute that silences it (D-198,
    /// [`crate::silence`]). Filled in for the snapshot only.
    #[serde(default)]
    pub silenced: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Fired(Alert),
    /// Detail changed while active (e.g. disk 91 % → 93 %), or the
    /// severity went down.
    Updated(Alert),
    /// The severity went up while active (e.g. disk 95 % → 98 %, critical).
    Worse(Alert),
    Resolved(Alert),
}

/// Which conditions a report speaks for. Keys of this source (and, if given,
/// only on these hosts) that are missing from the report are cleared.
#[derive(Debug, Clone)]
pub struct Scope<'a> {
    pub source: &'a str,
    pub hosts: Option<&'a HashSet<HostId>>,
}

impl Scope<'_> {
    fn covers(&self, key: &str, host: Option<HostId>) -> bool {
        key.split(':').next() == Some(self.source)
            && match (self.hosts, host) {
                (Some(set), Some(h)) => set.contains(&h),
                (Some(_), None) => false,
                (None, _) => true,
            }
    }
}

#[derive(Debug, Default)]
pub struct Tracker {
    /// Seen but still inside the grace period: condition and first-seen time.
    pending: HashMap<String, (Condition, i64)>,
    active: HashMap<String, Alert>,
    /// Active alerts whose condition went away: key → since when.
    clearing: HashMap<String, i64>,
    history: VecDeque<Alert>,
    next_id: u32,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a source's current conditions and returns what changed.
    pub fn report(&mut self, scope: &Scope<'_>, conditions: Vec<Condition>, now: i64) -> Vec<Change> {
        let mut changes = Vec::new();
        let seen: HashSet<String> = conditions.iter().map(|c| c.key.clone()).collect();

        for c in conditions {
            if let Some(a) = self.active.get_mut(&c.key) {
                self.clearing.remove(&c.key);
                if a.severity != c.severity || a.detail != c.detail || a.title != c.title {
                    let worse = c.severity > a.severity;
                    a.severity = c.severity;
                    a.detail = c.detail;
                    a.title = c.title;
                    changes.push(if worse { Change::Worse(a.clone()) } else { Change::Updated(a.clone()) });
                }
                continue;
            }
            let first = self.pending.get(&c.key).map(|(_, t)| *t).unwrap_or(now);
            if now - first >= c.kind.fire_after() {
                self.pending.remove(&c.key);
                self.next_id = self.next_id.wrapping_add(1);
                let alert = Alert {
                    id: self.next_id,
                    key: c.key.clone(),
                    kind: c.kind,
                    severity: c.severity,
                    title: c.title,
                    detail: c.detail,
                    host_id: c.host_id,
                    since: first,
                    resolved: None,
                    caused_by: None,
                    silenced: None,
                };
                self.active.insert(c.key, alert.clone());
                changes.push(Change::Fired(alert));
            } else {
                self.pending.insert(c.key.clone(), (c, first));
            }
        }

        self.pending.retain(|k, (c, _)| seen.contains(k) || !scope.covers(k, c.host_id));
        let gone: Vec<String> = self
            .active
            .iter()
            .filter(|(k, a)| !seen.contains(*k) && scope.covers(k, a.host_id))
            .map(|(k, _)| k.clone())
            .collect();
        for key in gone {
            let since = *self.clearing.entry(key.clone()).or_insert(now);
            let kind = self.active[&key].kind;
            if now - since >= kind.clear_after() {
                self.clearing.remove(&key);
                if let Some(mut a) = self.active.remove(&key) {
                    a.resolved = Some(now);
                    self.push_history(a.clone());
                    changes.push(Change::Resolved(a));
                }
            }
        }
        changes
    }

    /// Resolves the active alert with this key now (the user muted it).
    pub fn resolve_key(&mut self, key: &str, now: i64) -> Option<Alert> {
        self.pending.remove(key);
        self.clearing.remove(key);
        let mut a = self.active.remove(key)?;
        a.resolved = Some(now);
        self.push_history(a.clone());
        Some(a)
    }

    /// Takes alerts another evaluator recorded (Nightgrid Node, while the app
    /// was closed or between rounds). Resolved ones go into the history once;
    /// an unresolved one becomes active unless the key already is. Returns the
    /// alerts that became active, for one summary notification.
    pub fn import(&mut self, alerts: Vec<Alert>) -> Vec<Alert> {
        let mut fired = Vec::new();
        for mut a in alerts {
            a.caused_by = None;
            if a.resolved.is_some() {
                // Active in the app too: its own next round decides.
                if self.active.contains_key(&a.key) || self.history.iter().any(|h| h.key == a.key && h.since == a.since)
                {
                    continue;
                }
                self.next_id = self.next_id.wrapping_add(1);
                a.id = self.next_id;
                self.push_history(a);
            } else if !self.active.contains_key(&a.key) {
                self.pending.remove(&a.key);
                self.next_id = self.next_id.wrapping_add(1);
                a.id = self.next_id;
                self.active.insert(a.key.clone(), a.clone());
                fired.push(a);
            }
        }
        // Keep the history newest first.
        let mut h: Vec<Alert> = self.history.drain(..).collect();
        h.sort_by_key(|x| std::cmp::Reverse(x.resolved.unwrap_or(x.since)));
        self.history = h.into();
        fired
    }

    /// Forgets active and pending alerts without recording them (alerts were
    /// turned off).
    pub fn drop_active(&mut self) {
        self.pending.clear();
        self.active.clear();
        self.clearing.clear();
    }

    /// Conditions still inside their grace period.
    pub fn pending(&self) -> Vec<Condition> {
        self.pending.values().map(|(c, _)| c.clone()).collect()
    }

    /// Hosts that have pending or active alerts.
    pub fn hosts(&self) -> HashSet<HostId> {
        self.pending
            .values()
            .filter_map(|(c, _)| c.host_id)
            .chain(self.active.values().filter_map(|a| a.host_id))
            .collect()
    }

    /// Drops everything a source owns for hosts that no longer exist.
    pub fn forget_host(&mut self, host: HostId) {
        self.pending.retain(|_, (c, _)| c.host_id != Some(host));
        let keys: Vec<String> =
            self.active.iter().filter(|(_, a)| a.host_id == Some(host)).map(|(k, _)| k.clone()).collect();
        for k in keys {
            self.active.remove(&k);
            self.clearing.remove(&k);
        }
    }

    fn push_history(&mut self, a: Alert) {
        self.history.push_front(a);
        self.history.truncate(HISTORY_MAX);
    }

    /// Active alerts, worst and newest first.
    pub fn active(&self) -> Vec<Alert> {
        let mut v: Vec<Alert> = self.active.values().cloned().collect();
        v.sort_by(|a, b| b.severity.cmp(&a.severity).then(b.since.cmp(&a.since)).then(b.id.cmp(&a.id)));
        v
    }

    /// Resolved alerts, newest first.
    pub fn history(&self) -> Vec<Alert> {
        self.history.iter().cloned().collect()
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

// ---- condition builders ----

/// A failed or stale backup job.
pub fn backup(
    job: &crate::backup_jobs::BackupJob,
    host_name: &str,
    st: &crate::backup_jobs::BackupStatus,
) -> Option<Condition> {
    use crate::backup_jobs::BackupResult;
    let base = |kind, severity, detail: String| Condition {
        key: format!("backup:{}:{}", job.host_id, job.id),
        kind,
        severity,
        title: format!("{} on {host_name}", job.name),
        detail,
        host_id: Some(job.host_id),
    };
    if st.result == BackupResult::Failed {
        let why = st.summary.clone().unwrap_or_else(|| "the last run failed".into());
        return Some(base(AlertKind::BackupFailed, Severity::Critical, why));
    }
    st.stale.then(|| {
        base(
            AlertKind::BackupStale,
            Severity::Warn,
            format!("no good run in the last {}", crate::backup_jobs::stale_words(job.stale_hours)),
        )
    })
}

/// Error bursts, 404 spikes and new error patterns in one log source of a
/// host. Each new pattern is its own alert, keyed by its fingerprint, so
/// "Mute this pattern" knows what to mute.
pub fn log_anomaly(host: HostId, host_name: &str, source: &str, finding: &crate::logwatch::Finding) -> Vec<Condition> {
    let mut out = Vec::new();
    let label = source.split_once('/').map_or(source, |(_, n)| n);
    if let Some((n, usual)) = finding.burst {
        out.push(Condition {
            key: format!("logs:{host}:{source}:burst"),
            kind: AlertKind::LogBurst,
            severity: Severity::Warn,
            title: format!("{label} on {host_name}"),
            detail: format!("{n} errors since the last check (usually about {:.0})", usual.max(0.0)),
            host_id: Some(host),
        });
    }
    if let Some((n, usual)) = finding.not_found_spike {
        out.push(Condition {
            key: format!("logs:{host}:{source}:404"),
            kind: AlertKind::LogBurst,
            severity: Severity::Warn,
            title: format!("{label} on {host_name}"),
            detail: format!(
                "{n} \"not found\" (404) responses since the last check (usually about {:.0}). Requests for missing pages, which may be a scan or a broken link, not a fault",
                usual.max(0.0)
            ),
            host_id: Some(host),
        });
    }
    for p in &finding.new_patterns {
        let sample = p.sample.clone().unwrap_or_else(|| p.pattern.clone());
        let first = sample.lines().next().unwrap_or_default();
        let mut detail: String = first.chars().take(160).collect();
        let why = match p.loud {
            Some(w) => w.to_owned(),
            None => format!("{} times in the last few minutes", p.recent),
        };
        detail.push_str(&format!(" ({why})"));
        out.push(Condition {
            key: format!("logs:{host}:{source}:new:{}", p.fingerprint),
            kind: AlertKind::LogNewError,
            severity: if p.fatal { Severity::Critical } else { Severity::Warn },
            title: format!("{label} on {host_name}"),
            detail,
            host_id: Some(host),
        });
    }
    out
}

/// A host the metrics poller can't reach.
pub fn host_offline(host: HostId, name: &str, message: Option<&str>) -> Condition {
    Condition {
        key: format!("host:{host}"),
        kind: AlertKind::HostOffline,
        severity: Severity::Critical,
        title: name.to_owned(),
        detail: message.map_or_else(|| "unreachable over SSH".to_owned(), |m| format!("unreachable: {m}")),
        host_id: Some(host),
    }
}

/// Mounts at or above `threshold` percent used (critical from 98 %).
/// `skip_system` leaves out read-only and by-design-full mounts
/// ([`crate::silence::system_mount`]: WSL, Docker Desktop, snaps, images).
pub fn disk_full(host: HostId, name: &str, mounts: &[MountUsage], threshold: u8, skip_system: bool) -> Vec<Condition> {
    mounts
        .iter()
        .filter(|m| !(skip_system && crate::silence::system_mount(&m.mount, &m.fstype, m.read_only)))
        .filter_map(|m| {
            let size = m.used + m.avail;
            if size == 0 {
                return None;
            }
            let pct = m.used as f64 * 100.0 / size as f64;
            (pct >= f64::from(threshold)).then(|| Condition {
                key: format!("disk:{host}:{}", m.mount),
                kind: AlertKind::DiskFull,
                severity: if pct >= 98.0 { Severity::Critical } else { Severity::Warn },
                title: name.to_owned(),
                detail: format!("{} is {:.0}% full ({} free)", m.mount, pct.floor(), human_bytes(m.avail)),
                host_id: Some(host),
            })
        })
        .collect()
}

/// A failed service check, and its certificate if it expires within `days`.
pub fn service(
    id: crate::services::ServiceId,
    name: &str,
    host: Option<HostId>,
    r: &CheckResult,
    cert_days: u16,
    now: i64,
) -> Vec<Condition> {
    let mut out = Vec::new();
    if !r.up {
        out.push(Condition {
            key: format!("svc:{id}"),
            kind: AlertKind::ServiceDown,
            severity: Severity::Critical,
            title: name.to_owned(),
            detail: r.error.clone().unwrap_or_else(|| "check failed".into()),
            host_id: host,
        });
    }
    if let Some(left) = r.tls.as_ref().and_then(|t| t.days_left(now)) {
        if left < i64::from(cert_days) {
            out.push(Condition {
                key: format!("svc:{id}:cert"),
                kind: AlertKind::CertExpiring,
                severity: if left <= 3 { Severity::Critical } else { Severity::Warn },
                title: name.to_owned(),
                detail: match left {
                    l if l < 0 => "certificate has expired".into(),
                    0 => "certificate expires today".into(),
                    1 => "certificate expires tomorrow".into(),
                    l => format!("certificate expires in {l} days"),
                },
                host_id: host,
            });
        }
    }
    out
}

/// SMART health worse than OK.
pub fn smart(host: HostId, name: &str, disks: &[Disk]) -> Vec<Condition> {
    disks
        .iter()
        .filter(|d| matches!(d.health, Health::Warn | Health::Bad))
        .map(|d| Condition {
            key: format!("smart:{host}:{}", d.name),
            kind: AlertKind::SmartWarning,
            severity: if d.health == Health::Bad { Severity::Critical } else { Severity::Warn },
            title: name.to_owned(),
            detail: format!(
                "{} {}: {}",
                d.name,
                d.model.as_deref().unwrap_or(""),
                d.issues.first().map_or("health check failed", String::as_str)
            )
            .replace("  ", " "),
            host_id: Some(host),
        })
        .collect()
}

/// Exit code from Docker's status line, e.g. `Exited (137) 2 minutes ago`.
pub fn exit_code(status: &str) -> Option<i32> {
    let rest = status.strip_prefix("Exited (")?;
    rest.split(')').next()?.trim().parse().ok()
}

/// Remembers which containers were seen running, so only containers that
/// *stop* raise an alert (not ones that were already off when the app
/// started, or that a compose file keeps stopped).
#[derive(Debug, Default)]
pub struct ContainerWatch {
    seen_running: HashSet<(HostId, String)>,
}

impl ContainerWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Conditions for one host's current container list.
    pub fn conditions(&mut self, host: HostId, host_name: &str, containers: &[ContainerView]) -> Vec<Condition> {
        let names: HashSet<&str> = containers.iter().map(|c| c.name.as_str()).collect();
        self.seen_running.retain(|(h, n)| *h != host || names.contains(n.as_str()));
        let mut out = Vec::new();
        for c in containers {
            let key = (host, c.name.clone());
            let cond = |severity, detail: String| Condition {
                key: format!("docker:{host}:{}", c.name),
                kind: AlertKind::ContainerDown,
                severity,
                title: format!("{} on {host_name}", c.name),
                detail,
                host_id: Some(host),
            };
            match c.state.as_str() {
                "running" => {
                    self.seen_running.insert(key);
                    // An ignored healthcheck (D-201) counts as none.
                    if c.effective_health() == Some("unhealthy") {
                        out.push(cond(Severity::Warn, "running but unhealthy".into()));
                    }
                }
                "restarting" => {
                    self.seen_running.insert(key);
                    out.push(cond(Severity::Critical, "restarting (crash loop?)".into()));
                }
                "exited" | "dead" if self.seen_running.contains(&key) => {
                    // 0 = clean exit, 143 = SIGTERM from `docker stop`.
                    match exit_code(&c.status) {
                        Some(0 | 143) => {
                            self.seen_running.remove(&key);
                        }
                        code => out.push(cond(
                            Severity::Critical,
                            match code {
                                Some(137) => "exited with code 137 (killed, maybe out of memory)".into(),
                                Some(n) => format!("exited with code {n}"),
                                None => c.status.clone(),
                            },
                        )),
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn forget_host(&mut self, host: HostId) {
        self.seen_running.retain(|(h, _)| *h != host);
    }
}

/// Minutes since local midnight inside a quiet-hours window that may wrap
/// past midnight (22:00–07:00). Equal start and end means "never".
pub fn in_quiet_hours(start_min: u16, end_min: u16, now_min: u16) -> bool {
    match start_min.cmp(&end_min) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => (start_min..end_min).contains(&now_min),
        std::cmp::Ordering::Greater => now_min >= start_min || now_min < end_min,
    }
}

fn human_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 || v >= 10.0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// One line for a notification or webhook: `HOST OFFLINE · server01`.
pub fn headline(a: &Alert) -> String {
    format!("{} · {}", a.kind.label(), a.title)
}

/// The webhook request for an alert. `kind` is `ntfy`, `discord` or `json`;
/// `None` for anything else. The URL is the user's (ntfy topic URL, Discord
/// webhook URL, or any endpoint taking JSON).
pub fn webhook_request(kind: &str, url: &str, a: &Alert) -> Option<crate::http::Request> {
    use crate::http::Request;
    let resolved = a.resolved.is_some();
    let state = if resolved {
        "RESOLVED"
    } else if a.severity == Severity::Critical {
        "CRITICAL"
    } else {
        "WARNING"
    };
    match kind {
        "ntfy" => Some(
            Request::post(url, a.detail.clone())
                .header("Title", format!("Nightgrid {state}: {}", headline(a)))
                .header(
                    "Priority",
                    match (resolved, a.severity) {
                        (true, _) => "low",
                        (false, Severity::Critical) => "high",
                        (false, Severity::Warn) => "default",
                    },
                )
                .header("Tags", if resolved { "white_check_mark" } else { "warning" }),
        ),
        "discord" => Some(Request::post(url, Vec::new()).json(&serde_json::json!({
            "username": "Nightgrid",
            "content": format!("**{state}** · {}\n{}", headline(a), a.detail),
            "allowed_mentions": { "parse": [] },
        }))),
        "json" => Some(Request::post(url, Vec::new()).json(&serde_json::json!({
            "source": "nightgrid",
            "state": state.to_ascii_lowercase(),
            "alert": a,
            "headline": headline(a),
        }))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(key: &str, kind: AlertKind, host: Option<HostId>) -> Condition {
        Condition {
            key: key.into(),
            kind,
            severity: Severity::Warn,
            title: "t".into(),
            detail: "d".into(),
            host_id: host,
        }
    }

    fn all<'a>(source: &'a str) -> Scope<'a> {
        Scope { source, hosts: None }
    }

    #[test]
    fn grace_period_then_fire_then_resolve() {
        let mut t = Tracker::new();
        let c = || vec![cond("host:1", AlertKind::HostOffline, Some(1))];
        assert!(t.report(&all("host"), c(), 1000).is_empty());
        assert!(t.report(&all("host"), c(), 1060).is_empty());
        let ch = t.report(&all("host"), c(), 1120);
        assert!(matches!(&ch[..], [Change::Fired(a)] if a.since == 1000 && a.resolved.is_none()));
        assert_eq!(t.active().len(), 1);
        assert!(t.report(&all("host"), c(), 1200).is_empty(), "no repeat while active");
        let ch = t.report(&all("host"), vec![], 1300);
        assert!(matches!(&ch[..], [Change::Resolved(a)] if a.resolved == Some(1300)));
        assert!(t.active().is_empty());
        assert_eq!(t.history().len(), 1);
    }

    #[test]
    fn blip_inside_grace_never_fires() {
        let mut t = Tracker::new();
        t.report(&all("host"), vec![cond("host:1", AlertKind::HostOffline, Some(1))], 0);
        t.report(&all("host"), vec![], 30);
        // Back again later: the grace period starts over.
        assert!(t.report(&all("host"), vec![cond("host:1", AlertKind::HostOffline, Some(1))], 100).is_empty());
        assert!(t.report(&all("host"), vec![cond("host:1", AlertKind::HostOffline, Some(1))], 200).is_empty());
        assert_eq!(t.report(&all("host"), vec![cond("host:1", AlertKind::HostOffline, Some(1))], 220).len(), 1);
    }

    #[test]
    fn disk_clears_only_after_hold() {
        let mut t = Tracker::new();
        let c = || vec![cond("disk:1:/", AlertKind::DiskFull, Some(1))];
        assert_eq!(t.report(&all("disk"), c(), 0).len(), 1, "fires at once");
        assert!(t.report(&all("disk"), vec![], 10).is_empty());
        assert!(t.report(&all("disk"), c(), 20).is_empty(), "came back: still the same alert");
        assert!(t.report(&all("disk"), vec![], 30).is_empty());
        assert!(t.report(&all("disk"), vec![], 300).is_empty());
        assert_eq!(t.report(&all("disk"), vec![], 330).len(), 1);
    }

    #[test]
    fn scopes_only_clear_their_own() {
        let mut t = Tracker::new();
        t.report(&all("disk"), vec![cond("disk:1:/", AlertKind::DiskFull, Some(1))], 0);
        t.report(&all("smart"), vec![cond("smart:2:sda", AlertKind::SmartWarning, Some(2))], 0);
        // A smart scan of host 1 only must not clear host 2's smart alert, nor the disk alert.
        let hosts: HashSet<HostId> = [1].into();
        assert!(t.report(&Scope { source: "smart", hosts: Some(&hosts) }, vec![], 10).is_empty());
        assert_eq!(t.active().len(), 2);
        let hosts: HashSet<HostId> = [2].into();
        assert_eq!(t.report(&Scope { source: "smart", hosts: Some(&hosts) }, vec![], 10).len(), 1);
        assert_eq!(t.active().len(), 1);
    }

    #[test]
    fn updates_and_ordering() {
        let mut t = Tracker::new();
        let mut c = cond("svc:1", AlertKind::CertExpiring, None);
        t.report(&all("svc"), vec![c.clone()], 0);
        c.severity = Severity::Critical;
        c.detail = "certificate expires tomorrow".into();
        let ch = t.report(&all("svc"), vec![c.clone(), cond("svc:2", AlertKind::CertExpiring, None)], 10);
        assert!(ch.iter().any(|x| matches!(x, Change::Worse(a) if a.severity == Severity::Critical)));
        c.detail = "certificate expires today".into();
        let ch = t.report(&all("svc"), vec![c.clone(), cond("svc:2", AlertKind::CertExpiring, None)], 20);
        assert!(matches!(&ch[..], [Change::Updated(_)]), "same severity: only updated");
        let act = t.active();
        assert_eq!(act[0].key, "svc:1", "critical first");
        t.forget_host(1);
        assert_eq!(t.active().len(), 2, "service alerts without a host stay");
    }

    #[test]
    fn history_is_capped() {
        let mut t = Tracker::new();
        for i in 0..(HISTORY_MAX as i64 + 20) {
            t.report(&all("svc"), vec![cond("svc:1", AlertKind::CertExpiring, None)], i * 2);
            t.report(&all("svc"), vec![], i * 2 + 1);
        }
        assert_eq!(t.history().len(), HISTORY_MAX);
        t.clear_history();
        assert!(t.history().is_empty());
    }

    #[test]
    fn disk_conditions() {
        let m = |mount: &str, used: u64, avail: u64| MountUsage {
            device: "/dev/x".into(),
            mount: mount.into(),
            fstype: "ext4".into(),
            total: used + avail,
            used,
            avail,
            read_only: false,
        };
        let gb = 1024 * 1024 * 1024;
        let c =
            disk_full(3, "server01", &[m("/", 95 * gb, 5 * gb), m("/srv", 50, 50), m("/big", 99 * gb, gb)], 90, true);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].key, "disk:3:/");
        assert_eq!(c[0].detail, "/ is 95% full (5.0 GB free)");
        assert_eq!(c[0].severity, Severity::Warn);
        assert_eq!(c[1].severity, Severity::Critical);
        // WSL's and Docker Desktop's own mounts are always full by design.
        let wsl =
            MountUsage { fstype: "iso9660".into(), read_only: true, ..m("/mnt/wsl/docker-desktop/cli-tools", gb, 0) };
        let cd = MountUsage { fstype: "ext4".into(), ..m("/mnt/wsl/docker-desktop-bind-mounts/x", gb, 0) };
        assert!(disk_full(3, "hs01", &[wsl.clone(), cd.clone()], 90, true).is_empty());
        assert_eq!(disk_full(3, "hs01", &[wsl, cd], 90, false).len(), 2, "unless the setting is off");
    }

    #[test]
    fn service_conditions() {
        use crate::services::check::TlsInfo;
        let now = 1_000_000;
        let tls = |days: i64| TlsInfo {
            trusted: true,
            problem: None,
            expires: Some(now + days * 86_400 + 60),
            subject: None,
            issuer: None,
            fingerprint: None,
        };
        let up = CheckResult { up: true, status: Some(200), latency_ms: Some(3), error: None, tls: Some(tls(30)) };
        assert!(service(1, "web", None, &up, 14, now).is_empty());
        let soon = CheckResult { tls: Some(tls(10)), ..up.clone() };
        let c = service(1, "web", None, &soon, 14, now);
        assert_eq!(
            (c.len(), c[0].severity, c[0].detail.as_str()),
            (1, Severity::Warn, "certificate expires in 10 days")
        );
        let down = CheckResult { up: false, error: Some("connection refused".into()), tls: Some(tls(1)), ..up };
        let c = service(1, "web", Some(2), &down, 14, now);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].kind, c[0].detail.as_str()), (AlertKind::ServiceDown, "connection refused"));
        assert_eq!((c[1].severity, c[1].detail.as_str()), (Severity::Critical, "certificate expires tomorrow"));
    }

    fn ctr(name: &str, state: &str, status: &str, health: Option<&str>) -> ContainerView {
        ContainerView {
            id: name.into(),
            name: name.into(),
            image: "img".into(),
            state: state.into(),
            status: status.into(),
            health: health.map(Into::into),
            health_ignored: false,
            created: 0,
            ports: vec![],
            project: None,
            service: None,
            compose_dir: None,
            compose_files: vec![],
            web: vec![],
        }
    }

    #[test]
    fn containers() {
        assert_eq!(exit_code("Exited (137) 2 minutes ago"), Some(137));
        assert_eq!(exit_code("Up 3 hours"), None);
        let mut w = ContainerWatch::new();
        // Already stopped at start: no alert.
        let c = w.conditions(
            1,
            "s1",
            &[ctr("old", "exited", "Exited (1) 3 days ago", None), ctr("web", "running", "Up", None)],
        );
        assert!(c.is_empty());
        // web crashes.
        let c = w.conditions(1, "s1", &[ctr("web", "exited", "Exited (137) 5 seconds ago", None)]);
        assert_eq!(c.len(), 1);
        assert!(c[0].detail.contains("137"));
        assert_eq!(c[0].title, "web on s1");
        // Still down on the next poll: same condition.
        assert_eq!(w.conditions(1, "s1", &[ctr("web", "exited", "Exited (137) 1 minute ago", None)]).len(), 1);
        // Back up and unhealthy, then stopped cleanly.
        let c = w.conditions(1, "s1", &[ctr("web", "running", "Up (unhealthy)", Some("unhealthy"))]);
        assert_eq!(c[0].severity, Severity::Warn);
        assert!(w.conditions(1, "s1", &[ctr("web", "exited", "Exited (0) now", None)]).is_empty());
        // A clean stop forgets it: a later failed start isn't "seen running".
        assert!(w.conditions(1, "s1", &[ctr("web", "exited", "Exited (1) now", None)]).is_empty());
        let c = w.conditions(1, "s1", &[ctr("loop", "restarting", "Restarting (1) 2 seconds ago", None)]);
        assert_eq!(c[0].severity, Severity::Critical);
    }

    #[test]
    fn real_failures_stay_loud_with_log_guardrails() {
        use crate::logwatch::{Detector, RawLine, Rules, SourceCtx};
        let raw = |l: &[&str]| l.iter().map(|t| RawLine::new(None, *t)).collect::<Vec<_>>();
        let convertx = raw(&[
            "error: NOT_FOUND",
            " status: 404,",
            "   code: \"NOT_FOUND\"",
            "      at NotFoundError (/app/node_modules/elysia/src/error.ts:132:3)",
        ]);
        let mut d = Detector::new();
        let rules = Rules::default();
        d.observe("container/convertx", &raw(&["INFO up"]), &SourceCtx::default(), &rules, 0);
        let f = d.observe("container/convertx", &convertx, &SourceCtx::default(), &rules, 600);
        assert!(log_anomaly(1, "lumen", "container/convertx", &f).is_empty(), "a lone 404 doesn't alert");
        // The container exits or crash-loops: that alerts at once, whatever the logs said.
        let mut w = ContainerWatch::new();
        assert!(w
            .conditions(1, "lumen", &[ctr("convertx", "running", "Up 3 hours (healthy)", Some("healthy"))])
            .is_empty());
        let c = w.conditions(1, "lumen", &[ctr("convertx", "exited", "Exited (1) 5 seconds ago", None)]);
        assert_eq!((c.len(), c[0].severity), (1, Severity::Critical));
        let c = w.conditions(1, "lumen", &[ctr("convertx", "restarting", "Restarting (1) 2 seconds ago", None)]);
        assert_eq!(c[0].kind, AlertKind::ContainerDown);
        let c = w.conditions(1, "lumen", &[ctr("convertx", "running", "Up 1 minute (unhealthy)", Some("unhealthy"))]);
        assert_eq!(c[0].detail, "running but unhealthy");
        // With its healthcheck ignored (D-201), unhealthy is quiet, but a crash still alerts.
        let ignored = |state: &str, status: &str, health| ContainerView {
            health_ignored: true,
            ..ctr("convertx", state, status, health)
        };
        assert!(w
            .conditions(1, "lumen", &[ignored("running", "Up 1 minute (unhealthy)", Some("unhealthy"))])
            .is_empty());
        let c = w.conditions(1, "lumen", &[ignored("exited", "Exited (1) 5 seconds ago", None)]);
        assert_eq!((c.len(), c[0].severity), (1, Severity::Critical));
        let c = w.conditions(1, "lumen", &[ignored("restarting", "Restarting (1) 2 seconds ago", None)]);
        assert_eq!(c[0].detail, "restarting (crash loop?)");
        // And repeated errors from a crash-looping container alert immediately too.
        let looping = SourceCtx { restarting: true, ..Default::default() };
        let f = d.observe("container/convertx", &raw(&["ERROR cannot open /data/db.sqlite"]), &looping, &rules, 1200);
        let conds = log_anomaly(1, "lumen", "container/convertx", &f);
        assert_eq!(conds.len(), 1);
        assert!(conds[0].key.starts_with("logs:1:container/convertx:new:"));
        assert!(conds[0].detail.contains("restarting"));
        let f = d.observe("container/convertx", &raw(&["FATAL disk I/O error"]), &SourceCtx::default(), &rules, 1300);
        assert_eq!(log_anomaly(1, "lumen", "container/convertx", &f)[0].severity, Severity::Critical);
    }

    #[test]
    fn muting_resolves_now() {
        let mut t = Tracker::new();
        let c = Condition {
            key: "logs:1:container/x:new:00112233aabbccdd".into(),
            kind: AlertKind::LogNewError,
            severity: Severity::Warn,
            title: "x".into(),
            detail: "d".into(),
            host_id: Some(1),
        };
        t.report(&Scope { source: "logs", hosts: None }, vec![c], 0);
        assert_eq!(t.active().len(), 1);
        assert!(t.resolve_key("logs:1:container/x:new:00112233aabbccdd", 5).is_some());
        assert!(t.active().is_empty());
        assert_eq!(t.history()[0].resolved, Some(5));
    }

    #[test]
    fn quiet_hours() {
        let h = |x: u16| x * 60;
        assert!(in_quiet_hours(h(22), h(7), h(23)));
        assert!(in_quiet_hours(h(22), h(7), h(3)));
        assert!(!in_quiet_hours(h(22), h(7), h(7)));
        assert!(!in_quiet_hours(h(22), h(7), h(12)));
        assert!(in_quiet_hours(h(1), h(5), h(2)));
        assert!(!in_quiet_hours(h(1), h(5), h(6)));
        assert!(!in_quiet_hours(h(3), h(3), h(3)));
    }

    #[test]
    fn misc() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(20 * 1024 * 1024), "20 MB");
        let c = host_offline(4, "server01", Some("connection refused"));
        assert_eq!(c.detail, "unreachable: connection refused");
        let a = Alert {
            id: 1,
            key: c.key,
            kind: c.kind,
            severity: c.severity,
            title: c.title,
            detail: c.detail,
            host_id: c.host_id,
            since: 0,
            resolved: None,
            caused_by: None,
            silenced: None,
        };
        assert_eq!(headline(&a), "HOST OFFLINE · server01");

        let r = webhook_request("ntfy", "https://ntfy.example/topic", &a).unwrap();
        assert_eq!(r.body, b"unreachable: connection refused");
        assert!(r.headers.contains(&("Priority".into(), "high".into())));
        assert!(r.headers.iter().any(|(k, v)| k == "Title" && v == "Nightgrid CRITICAL: HOST OFFLINE · server01"));
        let done = Alert { resolved: Some(5), ..a.clone() };
        let r = webhook_request("discord", "https://discord.example/api/webhooks/x", &done).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert!(v["content"].as_str().unwrap().starts_with("**RESOLVED** · HOST OFFLINE"));
        let r = webhook_request("json", "http://hooks.lan/x", &a).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!((v["state"].as_str(), v["alert"]["kind"].as_str()), (Some("critical"), Some("hostOffline")));
        assert!(webhook_request("none", "x", &a).is_none());
    }
    #[test]
    fn imported_alerts_go_to_history_or_active_once() {
        let mut t = Tracker::new();
        let a = |key: &str, since: i64, resolved: Option<i64>| Alert {
            id: 0,
            key: key.into(),
            kind: AlertKind::DiskFull,
            severity: Severity::Warn,
            title: "server01".into(),
            detail: "91%".into(),
            host_id: Some(1),
            since,
            resolved,
            caused_by: None,
            silenced: None,
        };
        let fired = t.import(vec![a("disk:1:/srv", 10, Some(20)), a("disk:1:/data", 30, None)]);
        assert_eq!(fired.len(), 1);
        assert_eq!(t.active().len(), 1);
        assert_eq!(t.history().len(), 1);
        // A second sync brings the same alerts: nothing changes.
        assert!(t.import(vec![a("disk:1:/srv", 10, Some(20)), a("disk:1:/data", 30, None)]).is_empty());
        assert_eq!((t.active().len(), t.history().len()), (1, 1));
        assert_ne!(t.active()[0].id, t.history()[0].id);
    }
}
