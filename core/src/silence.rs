//! Acknowledging and muting alerts (D-196).
//!
//! A silenced alert stays in the active list and on the timeline, marked,
//! but doesn't count toward the badge or the tray, and sends nothing: no
//! desktop notification, inbox entry, ntfy message or webhook.
//!
//! - **Acknowledge** one alert: "I know". It ends when the alert gets
//!   worse (a higher severity, such as a disk crossing from 90 % into the
//!   critical 98 %), and otherwise when the alert clears ([`AckFor::Clear`]),
//!   at a set time, or never (permanent).
//! - **Mute** a rule: one alert, one kind on one host, a whole host, or one
//!   kind everywhere, for a while or for good, whatever the severity.
//!
//! Silences live in the local database (`alert_silences`). They are state
//! of this computer, like the inbox, so they aren't part of a profile export.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::alerts::{Alert, AlertKind, Severity};
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type SilenceId = u32;

/// Silences kept at most (oldest expired ones go first).
pub const MAX: usize = 500;
/// Minutes after the app starts before an "until it clears" acknowledgement
/// whose alert never came back is dropped (every source has reported by then).
pub const STALE_AFTER: i64 = 20 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SilenceMode {
    Ack,
    Mute,
}

impl SilenceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SilenceMode::Ack => "ack",
            SilenceMode::Mute => "mute",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ack" => Some(SilenceMode::Ack),
            "mute" => Some(SilenceMode::Mute),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Silence {
    pub id: SilenceId,
    pub mode: SilenceMode,
    /// One alert (`disk:3:/srv`). Acknowledgements always have one.
    pub key: Option<String>,
    /// Mutes without a key: this kind of alert (on `host_id`, or everywhere).
    pub kind: Option<AlertKind>,
    /// Mutes without a key: this host (every kind, or `kind` only).
    pub host_id: Option<HostId>,
    /// What it silences, in words, for the list: `DISK FULL · nas01 · /volume1`.
    pub label: String,
    /// Acknowledgements: the severity acknowledged. Worse lifts it.
    pub severity: Option<Severity>,
    /// Ends at this time (unix seconds); `None` = no time limit.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub until: Option<i64>,
    /// Acknowledgements: also ends when the alert clears.
    pub until_clear: bool,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub created: i64,
}

impl Silence {
    pub fn expired(&self, now: i64) -> bool {
        self.until.is_some_and(|u| u <= now)
    }

    /// Whether this silences `a` right now.
    pub fn matches(&self, a: &Alert, now: i64) -> bool {
        if self.expired(now) {
            return false;
        }
        match self.mode {
            SilenceMode::Ack => {
                self.key.as_deref() == Some(a.key.as_str()) && self.severity.map_or(true, |s| a.severity <= s)
            }
            SilenceMode::Mute => match &self.key {
                Some(k) => *k == a.key,
                None => {
                    (self.kind.is_some() || self.host_id.is_some())
                        && self.kind.map_or(true, |k| k == a.kind)
                        && self.host_id.map_or(true, |h| a.host_id == Some(h))
                }
            },
        }
    }

    /// An acknowledgement the alert has outgrown (it got worse).
    pub fn lifted_by(&self, a: &Alert) -> bool {
        self.mode == SilenceMode::Ack
            && self.key.as_deref() == Some(a.key.as_str())
            && self.severity.is_some_and(|s| a.severity > s)
    }
}

/// The silence that covers `a` now: a mute before an acknowledgement.
pub fn covering<'a>(list: &'a [Silence], a: &Alert, now: i64) -> Option<&'a Silence> {
    list.iter().filter(|s| s.matches(a, now)).min_by_key(|s| (s.mode == SilenceMode::Ack, s.id))
}

/// How long an acknowledgement lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AckFor {
    /// Until the alert clears (or gets worse).
    Clear,
    /// For this many seconds (or until it gets worse), even if it clears and comes back.
    For {
        #[cfg_attr(feature = "specta", specta(type = u32))]
        secs: i64,
    },
    /// For good, unless it gets worse.
    Always,
}

/// What a mute covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum MuteScope {
    /// This one alert (one mount, container, service…).
    Alert,
    /// This kind of alert on this alert's host.
    KindOnHost,
    /// Every alert on this alert's host.
    Host,
    /// This kind of alert on every host.
    Kind,
}

/// The longest timed silence: a year (anything longer is "forever").
pub const MAX_SECS: i64 = 366 * 86_400;

fn check_secs(secs: i64) -> Result<()> {
    if !(60..=MAX_SECS).contains(&secs) {
        return Err(Error::Invalid { field: "duration", reason: "between a minute and a year".into() });
    }
    Ok(())
}

fn mount_of(a: &Alert) -> Option<&str> {
    (a.kind == AlertKind::DiskFull || a.kind == AlertKind::DiskFilling)
        .then(|| a.key.splitn(3, ':').nth(2))
        .flatten()
        .filter(|m| m.starts_with('/') || m.contains(":\\") || m.ends_with(':'))
}

/// `DISK FULL · nas01 · /volume1`.
pub fn alert_label(a: &Alert) -> String {
    let mut s = format!("{} · {}", a.kind.label(), a.title);
    if let Some(m) = mount_of(a) {
        s.push_str(" · ");
        s.push_str(m);
    }
    s
}

/// The acknowledgement for `a`.
pub fn ack_for(a: &Alert, how: AckFor, now: i64) -> Result<Silence> {
    let (until, until_clear) = match how {
        AckFor::Clear => (None, true),
        AckFor::For { secs } => {
            check_secs(secs)?;
            (Some(now + secs), false)
        }
        AckFor::Always => (None, false),
    };
    Ok(Silence {
        id: 0,
        mode: SilenceMode::Ack,
        key: Some(a.key.clone()),
        kind: Some(a.kind),
        host_id: a.host_id,
        label: alert_label(a),
        severity: Some(a.severity),
        until,
        until_clear,
        created: now,
    })
}

/// A mute built from `a`. `secs` = `None` mutes for good. `host_name` names
/// the host for host-wide labels.
pub fn mute_for(a: &Alert, scope: &MuteScope, secs: Option<i64>, host_name: &str, now: i64) -> Result<Silence> {
    if let Some(s) = secs {
        check_secs(s)?;
    }
    let base = Silence {
        id: 0,
        mode: SilenceMode::Mute,
        key: None,
        kind: None,
        host_id: None,
        label: String::new(),
        severity: None,
        until: secs.map(|s| now + s),
        until_clear: false,
        created: now,
    };
    let need_host =
        || a.host_id.ok_or(Error::Invalid { field: "scope", reason: "this alert doesn't belong to a host".into() });
    Ok(match scope {
        MuteScope::Alert => {
            Silence { key: Some(a.key.clone()), kind: Some(a.kind), host_id: a.host_id, label: alert_label(a), ..base }
        }
        MuteScope::KindOnHost => Silence {
            kind: Some(a.kind),
            host_id: Some(need_host()?),
            label: format!("{} · every alert of this kind on {host_name}", a.kind.label()),
            ..base
        },
        MuteScope::Host => {
            Silence { host_id: Some(need_host()?), label: format!("Every alert on {host_name}"), ..base }
        }
        MuteScope::Kind => {
            Silence { kind: Some(a.kind), label: format!("{} · on every host", a.kind.label()), ..base }
        }
    })
}

/// Read-only, by-design-full and system mounts that never get a DISK FULL
/// alert while "skip system mounts" is on: Docker Desktop's and WSL's own
/// mounts, snaps, CD/DVD images and other read-only image filesystems.
pub fn system_mount(mount: &str, fstype: &str, read_only: bool) -> bool {
    const IMAGE_FS: [&str; 7] = ["squashfs", "iso9660", "udf", "erofs", "cramfs", "romfs", "overlay"];
    const PREFIXES: [&str; 6] = ["/mnt/wsl/", "/mnt/wslg/", "/usr/lib/wsl/", "/snap/", "/var/lib/snapd/", "/run/"];
    let m = mount.trim_end_matches('/');
    read_only
        || IMAGE_FS.contains(&fstype)
        || fstype.starts_with("fuse.snap")
        || m == "/mnt/wsl"
        || m == "/mnt/wslg"
        || m == "/usr/lib/wsl"
        || PREFIXES.iter().any(|p| mount.starts_with(p))
}

// ---- store -----------------------------------------------------------------

type Row = (i64, String, Option<String>, Option<String>, Option<i64>, String, Option<String>, Option<i64>, bool, i64);

fn kind_str(k: AlertKind) -> String {
    serde_json::to_value(k).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

fn kind_parse(s: &str) -> Option<AlertKind> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).ok()
}

fn sev_str(s: Severity) -> &'static str {
    match s {
        Severity::Warn => "warn",
        Severity::Critical => "critical",
    }
}

impl Store {
    /// Every silence, newest first.
    pub fn silences(&self) -> Result<Vec<Silence>> {
        let mut st = self.conn().prepare(
            "SELECT id, mode, key, kind, host_id, label, severity, until, until_clear, created
             FROM alert_silences ORDER BY id DESC",
        )?;
        let rows = st.query_map([], |r| -> rusqlite::Result<Row> {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get::<_, i64>(8)? != 0,
                r.get(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, mode, key, kind, host, label, sev, until, until_clear, created) = row?;
            let Some(mode) = SilenceMode::parse(&mode) else { continue };
            out.push(Silence {
                id: id as SilenceId,
                mode,
                key,
                kind: kind.as_deref().and_then(kind_parse),
                host_id: host.map(|h| h as HostId),
                label,
                severity: sev.map(|s| if s == "critical" { Severity::Critical } else { Severity::Warn }),
                until,
                until_clear,
                created,
            });
        }
        Ok(out)
    }

    /// Saves a silence. An acknowledgement or mute of the same alert (or the
    /// same kind and host) replaces the older one. Returns it with its id.
    pub fn silence_add(&self, s: &Silence) -> Result<Silence> {
        if s.key.is_none() && s.kind.is_none() && s.host_id.is_none() {
            return Err(Error::Invalid { field: "scope", reason: "a mute needs an alert, a kind or a host".into() });
        }
        if s.mode == SilenceMode::Ack && s.key.is_none() {
            return Err(Error::Invalid { field: "key", reason: "an acknowledgement is for one alert".into() });
        }
        let c = self.conn();
        let kind = s.kind.map(kind_str);
        if s.key.is_some() {
            c.execute("DELETE FROM alert_silences WHERE mode = ?1 AND key = ?2", params![s.mode.as_str(), s.key])?;
        } else {
            c.execute(
                "DELETE FROM alert_silences WHERE mode = ?1 AND key IS NULL AND kind IS ?2 AND host_id IS ?3",
                params![s.mode.as_str(), kind, s.host_id],
            )?;
        }
        c.execute(
            "INSERT INTO alert_silences (mode, key, kind, host_id, label, severity, until, until_clear, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                s.mode.as_str(),
                s.key,
                kind,
                s.host_id,
                s.label.chars().take(300).collect::<String>(),
                s.severity.map(sev_str),
                s.until,
                i64::from(s.until_clear),
                s.created
            ],
        )?;
        let id = c.last_insert_rowid();
        c.execute(
            "DELETE FROM alert_silences WHERE id NOT IN (SELECT id FROM alert_silences ORDER BY id DESC LIMIT ?1)",
            [MAX as i64],
        )?;
        Ok(Silence { id: id as SilenceId, ..s.clone() })
    }

    pub fn silence_get(&self, id: SilenceId) -> Result<Option<Silence>> {
        let exists =
            self.conn().query_row("SELECT 1 FROM alert_silences WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some();
        Ok(if exists { self.silences()?.into_iter().find(|s| s.id == id) } else { None })
    }

    pub fn silences_delete(&self, ids: &[SilenceId]) -> Result<()> {
        let c = self.conn();
        for id in ids {
            c.execute("DELETE FROM alert_silences WHERE id = ?1", [id])?;
        }
        Ok(())
    }
}

/// What a round of alert changes does to the silences.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cleanup {
    /// Silences to delete: expired, outgrown, or acknowledgements of an
    /// alert that cleared.
    pub delete: Vec<SilenceId>,
    /// Silences that ended by time this round: their still-active alerts
    /// are announced again.
    pub ended: Vec<SilenceId>,
}

/// Decides which silences end. `active` is every active alert, `resolved`
/// the alerts that cleared this round, `known` the keys active or pending
/// right now, `settled` whether the app has run long enough that every
/// source has reported once.
pub fn cleanup(
    list: &[Silence],
    active: &[Alert],
    resolved: &[Alert],
    known: &dyn Fn(&str) -> bool,
    settled: bool,
    now: i64,
) -> Cleanup {
    let mut out = Cleanup::default();
    for s in list {
        if s.expired(now) {
            out.delete.push(s.id);
            out.ended.push(s.id);
            continue;
        }
        if s.mode != SilenceMode::Ack {
            continue;
        }
        let worse = active.iter().any(|a| s.lifted_by(a));
        let key = s.key.as_deref().unwrap_or_default();
        let cleared = s.until_clear && (resolved.iter().any(|a| a.key == key) || (settled && !known(key)));
        if worse || cleared {
            out.delete.push(s.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alert(key: &str, kind: AlertKind, sev: Severity, host: Option<HostId>) -> Alert {
        Alert {
            id: 1,
            key: key.into(),
            kind,
            severity: sev,
            title: "nas01".into(),
            detail: "/volume1 is 93% full (40 GB free)".into(),
            host_id: host,
            since: 0,
            resolved: None,
            caused_by: None,
            silenced: None,
        }
    }

    #[test]
    fn ack_holds_until_worse() {
        let a = alert("disk:3:/volume1", AlertKind::DiskFull, Severity::Warn, Some(3));
        let s = Silence { id: 1, ..ack_for(&a, AckFor::Clear, 100).unwrap() };
        assert!(s.matches(&a, 200));
        let worse = Alert { severity: Severity::Critical, ..a.clone() };
        assert!(!s.matches(&worse, 200));
        assert!(s.lifted_by(&worse));
        let other = alert("disk:3:/srv", AlertKind::DiskFull, Severity::Warn, Some(3));
        assert!(!s.matches(&other, 200));
        assert_eq!(s.label, "DISK FULL · nas01 · /volume1");
    }

    #[test]
    fn timed_and_permanent_acks() {
        let a = alert("disk:3:/volume1", AlertKind::DiskFull, Severity::Warn, Some(3));
        let t = Silence { id: 1, ..ack_for(&a, AckFor::For { secs: 3600 }, 0).unwrap() };
        assert!(t.matches(&a, 3599));
        assert!(!t.matches(&a, 3600));
        assert!(!t.until_clear);
        let p = Silence { id: 2, ..ack_for(&a, AckFor::Always, 0).unwrap() };
        assert!(p.matches(&a, i64::MAX / 2));
        assert!(ack_for(&a, AckFor::For { secs: 5 }, 0).is_err());
    }

    #[test]
    fn mute_scopes() {
        let a = alert("disk:3:/mnt/wsl/x", AlertKind::DiskFull, Severity::Critical, Some(3));
        let b = alert("disk:3:/", AlertKind::DiskFull, Severity::Critical, Some(3));
        let c = alert("host:3", AlertKind::HostOffline, Severity::Critical, Some(3));
        let d = alert("disk:4:/", AlertKind::DiskFull, Severity::Warn, Some(4));
        let svc = alert("svc:9", AlertKind::ServiceDown, Severity::Critical, None);
        let m = |scope| Silence { id: 1, ..mute_for(&a, &scope, None, "hs01", 0).unwrap() };
        let one = m(MuteScope::Alert);
        assert_eq!([&a, &b, &c, &d].map(|x| one.matches(x, 0)), [true, false, false, false]);
        let kh = m(MuteScope::KindOnHost);
        assert_eq!([&a, &b, &c, &d].map(|x| kh.matches(x, 0)), [true, true, false, false]);
        let host = m(MuteScope::Host);
        assert_eq!([&a, &b, &c, &d, &svc].map(|x| host.matches(x, 0)), [true, true, true, false, false]);
        let kind = m(MuteScope::Kind);
        assert_eq!([&a, &b, &c, &d].map(|x| kind.matches(x, 0)), [true, true, false, true]);
        assert!(mute_for(&svc, &MuteScope::Host, None, "x", 0).is_err());
        let timed = Silence { id: 1, ..mute_for(&a, &MuteScope::Alert, Some(3600), "hs01", 0).unwrap() };
        assert!(timed.matches(&a, 10) && !timed.matches(&a, 3600));
    }

    #[test]
    fn mute_wins_over_ack() {
        let a = alert("disk:3:/", AlertKind::DiskFull, Severity::Warn, Some(3));
        let list = vec![
            Silence { id: 5, ..ack_for(&a, AckFor::Clear, 0).unwrap() },
            Silence { id: 6, ..mute_for(&a, &MuteScope::Host, None, "h", 0).unwrap() },
        ];
        assert_eq!(covering(&list, &a, 1).map(|s| s.id), Some(6));
    }

    #[test]
    fn cleanup_rules() {
        let a = alert("disk:3:/", AlertKind::DiskFull, Severity::Warn, Some(3));
        let b = alert("disk:3:/srv", AlertKind::DiskFull, Severity::Warn, Some(3));
        let list = vec![
            Silence { id: 1, ..ack_for(&a, AckFor::Clear, 0).unwrap() },
            Silence { id: 2, ..ack_for(&b, AckFor::Always, 0).unwrap() },
            Silence { id: 3, ..mute_for(&a, &MuteScope::Kind, Some(60), "h", 0).unwrap() },
        ];
        let none = |_: &str| true;
        // Nothing changed, before the mute ends.
        assert_eq!(cleanup(&list, &[a.clone(), b.clone()], &[], &none, true, 30), Cleanup::default());
        // The mute ended; a cleared: its ack goes, b's permanent ack stays.
        let r = cleanup(&list, std::slice::from_ref(&b), &[Alert { resolved: Some(70), ..a.clone() }], &none, true, 70);
        assert_eq!((r.delete, r.ended), (vec![1, 3], vec![3]));
        // b got worse: its permanent ack is lifted.
        let worse = Alert { severity: Severity::Critical, ..b.clone() };
        assert_eq!(cleanup(&list, &[worse], &[], &none, true, 30).delete, vec![2]);
        // After a restart the alert never came back: dropped once settled.
        let gone = |_: &str| false;
        assert!(cleanup(&list, &[], &[], &gone, false, 30).delete.is_empty());
        assert_eq!(cleanup(&list, &[], &[], &gone, true, 30).delete, vec![1]);
    }

    #[test]
    fn system_mounts() {
        assert!(system_mount("/mnt/wsl/docker-desktop/cli-tools", "ext4", false));
        assert!(system_mount("/mnt/wsl/docker-desktop/cli-tools", "iso9660", true));
        assert!(system_mount("/usr/lib/wsl/drivers", "9p", false));
        assert!(system_mount("/media/cdrom", "iso9660", false));
        assert!(system_mount("/snap/core22/1380", "squashfs", true));
        assert!(system_mount("/srv/backup", "nfs4", true), "read-only: nothing can fill it");
        assert!(!system_mount("/", "ext4", false));
        assert!(!system_mount("/mnt/data", "xfs", false));
        assert!(!system_mount("/mnt/wslstuff", "ext4", false));
        assert!(!system_mount("/volume1", "btrfs", false));
    }

    #[test]
    fn store_round_trip() {
        let s = Store::open_in_memory().unwrap();
        let a = alert("disk:1:/", AlertKind::DiskFull, Severity::Warn, None);
        let x = s.silence_add(&ack_for(&a, AckFor::Clear, 5).unwrap()).unwrap();
        // Acknowledging again replaces the first.
        let y = s.silence_add(&ack_for(&a, AckFor::Always, 6).unwrap()).unwrap();
        let m = s.silence_add(&mute_for(&a, &MuteScope::Kind, Some(3600), "h", 7).unwrap()).unwrap();
        let list = s.silences().unwrap();
        assert_eq!(list.iter().map(|s| s.id).collect::<Vec<_>>(), vec![m.id, y.id]);
        assert_ne!(x.id, y.id);
        assert_eq!(list[1].severity, Some(Severity::Warn));
        assert_eq!(list[0].kind, Some(AlertKind::DiskFull));
        assert_eq!(list[0].until, Some(3607));
        assert!(s.silence_get(y.id).unwrap().is_some());
        s.silences_delete(&[y.id]).unwrap();
        assert_eq!(s.silences().unwrap().len(), 1);
        assert!(s.silence_add(&Silence { kind: None, ..m }).is_err());
    }
}
