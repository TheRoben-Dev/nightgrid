//! UPS watch (M10s): reads a UPS through NUT's `upsc` on the host it is
//! plugged into, raises alerts when it runs on battery or gets low, and can
//! shut down a group of hosts (the UPS host last) after a countdown the user
//! can cancel.
//!
//! Only `upsc` is used: it reads `upsd` without a password and changes
//! nothing. The shutdown itself is the same power action as System → POWER,
//! run with sudo that needs no prompt (root, NOPASSWD or a remembered
//! password); hosts where that isn't possible are reported, not skipped
//! silently.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::alerts::{AlertKind, Condition, Severity};
use crate::files::path::shell_quote;
use crate::hosts::{GroupId, HostId};
use crate::store::Store;
use crate::{Error, Result};

pub type UpsId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpsWatch {
    pub id: UpsId,
    /// A label for the UI, e.g. "Rack UPS".
    pub label: String,
    /// The `upsc` target: `ups`, `ups@localhost` or `ups@nas.lan:3493`.
    pub target: String,
    /// Where `upsc` runs (usually the host the UPS is plugged into).
    pub host_id: HostId,
    /// Hosts to shut down. `None`: alerts only.
    pub group_id: Option<GroupId>,
    /// Shut the group down when the trigger below is met.
    pub shutdown: bool,
    /// Trigger after this long on battery (seconds); 0 = only on low battery.
    pub on_battery_secs: u32,
    /// Trigger when the UPS reports less runtime than this (seconds), or
    /// flags low battery itself.
    pub low_runtime_secs: u32,
    /// Countdown before the shutdown starts, in seconds.
    pub countdown_secs: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct UpsInput {
    pub id: Option<UpsId>,
    pub label: String,
    pub target: String,
    pub host_id: HostId,
    pub group_id: Option<GroupId>,
    pub shutdown: bool,
    pub on_battery_secs: u32,
    pub low_runtime_secs: u32,
    pub countdown_secs: u32,
    pub enabled: bool,
}

pub const MAX_WATCHES: usize = 16;

fn invalid(field: &'static str, reason: &str) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// `upsname[@hostname[:port]]`, as `upsc` takes it.
pub fn valid_target(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= 120
        && !t.starts_with('-')
        && t.chars().all(|c| c.is_ascii_alphanumeric() || "-_.@:[]".contains(c))
}

fn clean(i: &UpsInput) -> Result<UpsInput> {
    let label = i.label.trim().to_owned();
    if label.is_empty() || label.chars().count() > 60 {
        return Err(invalid("label", "must be 1-60 characters"));
    }
    let target = i.target.trim().to_owned();
    if !valid_target(&target) {
        return Err(invalid("target", "a NUT name like ups or ups@localhost"));
    }
    if i.shutdown && i.group_id.is_none() {
        return Err(invalid("groupId", "pick the group to shut down"));
    }
    if i.on_battery_secs > 24 * 3600 {
        return Err(invalid("onBatterySecs", "at most a day"));
    }
    if i.low_runtime_secs > 4 * 3600 {
        return Err(invalid("lowRuntimeSecs", "at most 4 hours"));
    }
    if !(10..=1800).contains(&i.countdown_secs) {
        return Err(invalid("countdownSecs", "between 10 seconds and 30 minutes"));
    }
    Ok(UpsInput { label, target, ..i.clone() })
}

const COLS: &str =
    "id, label, target, host_id, group_id, shutdown, on_battery_secs, low_runtime_secs, countdown_secs, enabled";

fn row(r: &Row<'_>) -> rusqlite::Result<UpsWatch> {
    Ok(UpsWatch {
        id: r.get(0)?,
        label: r.get(1)?,
        target: r.get(2)?,
        host_id: r.get(3)?,
        group_id: r.get(4)?,
        shutdown: r.get(5)?,
        on_battery_secs: r.get(6)?,
        low_runtime_secs: r.get(7)?,
        countdown_secs: r.get(8)?,
        enabled: r.get(9)?,
    })
}

impl Store {
    pub fn ups_list(&self) -> Result<Vec<UpsWatch>> {
        let mut st = self.conn().prepare(&format!("SELECT {COLS} FROM ups_watches ORDER BY label, id"))?;
        let rows = st.query_map([], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn ups_get(&self, id: UpsId) -> Result<Option<UpsWatch>> {
        Ok(self.conn().query_row(&format!("SELECT {COLS} FROM ups_watches WHERE id = ?1"), [id], row).optional()?)
    }

    pub fn ups_save(&self, input: &UpsInput) -> Result<UpsWatch> {
        let i = clean(input)?;
        let c = self.conn();
        let id = match i.id {
            None => {
                if self.ups_list()?.len() >= MAX_WATCHES {
                    return Err(invalid("ups", "too many UPS watches"));
                }
                c.execute(
                    "INSERT INTO ups_watches (label, target, host_id, group_id, shutdown, on_battery_secs,
                       low_runtime_secs, countdown_secs, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        i.label,
                        i.target,
                        i.host_id,
                        i.group_id,
                        i.shutdown,
                        i.on_battery_secs,
                        i.low_runtime_secs,
                        i.countdown_secs,
                        i.enabled
                    ],
                )?;
                c.last_insert_rowid() as UpsId
            }
            Some(id) => {
                let n = c.execute(
                    "UPDATE ups_watches SET label = ?2, target = ?3, host_id = ?4, group_id = ?5, shutdown = ?6,
                       on_battery_secs = ?7, low_runtime_secs = ?8, countdown_secs = ?9, enabled = ?10 WHERE id = ?1",
                    params![
                        id,
                        i.label,
                        i.target,
                        i.host_id,
                        i.group_id,
                        i.shutdown,
                        i.on_battery_secs,
                        i.low_runtime_secs,
                        i.countdown_secs,
                        i.enabled
                    ],
                )?;
                if n == 0 {
                    return Err(Error::NotFound("UPS watch"));
                }
                id
            }
        };
        self.ups_get(id)?.ok_or(Error::NotFound("UPS watch"))
    }

    pub fn ups_delete(&self, id: UpsId) -> Result<()> {
        self.conn().execute("DELETE FROM ups_watches WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// Reads every variable of one UPS. stderr is kept: it carries the reason
/// when the driver or `upsd` isn't there.
pub fn command(target: &str) -> String {
    format!(
        "command -v upsc >/dev/null 2>&1 || {{ echo 'NIGHTGRID: no upsc'; exit 0; }}; upsc {} 2>&1",
        shell_quote(target)
    )
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpsReading {
    /// `ups.status` flags: OL (on line), OB (on battery), LB (low battery),
    /// CHRG, RB (replace battery) …
    pub status: Vec<String>,
    pub on_battery: bool,
    pub low_battery: bool,
    pub replace_battery: bool,
    /// Percent.
    pub charge: Option<f32>,
    /// Seconds of runtime left.
    pub runtime: Option<u32>,
    /// Percent.
    pub load: Option<f32>,
    pub input_voltage: Option<f32>,
    pub model: Option<String>,
    /// Why there is no reading (no upsc, driver not connected, unknown UPS).
    pub error: Option<String>,
}

/// Parses `upsc` output (`key: value` lines).
pub fn parse(text: &str) -> UpsReading {
    let mut r = UpsReading::default();
    let mut errors = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line == "NIGHTGRID: no upsc" {
            r.error = Some("upsc isn't installed on this host (install nut-client, or nut)".into());
            return r;
        }
        if line.starts_with("Init SSL") || line.is_empty() {
            continue;
        }
        if let Some(e) = line.strip_prefix("Error: ") {
            errors.push(e.to_owned());
            continue;
        }
        let Some((k, v)) = line.split_once(": ") else { continue };
        let num = || v.trim().parse::<f32>().ok();
        match k.trim() {
            "ups.status" => r.status = v.split_whitespace().map(str::to_owned).collect(),
            "battery.charge" => r.charge = num(),
            "battery.runtime" => r.runtime = num().map(|s| s.max(0.0) as u32),
            "ups.load" => r.load = num(),
            "input.voltage" => r.input_voltage = num(),
            "device.model" | "ups.model" if r.model.is_none() => r.model = Some(v.trim().to_owned()),
            _ => {}
        }
    }
    r.on_battery = r.status.iter().any(|s| s == "OB");
    r.low_battery = r.status.iter().any(|s| s == "LB");
    r.replace_battery = r.status.iter().any(|s| s == "RB");
    if r.status.is_empty() {
        r.error = Some(errors.first().cloned().unwrap_or_else(|| "no ups.status in upsc's answer".into()));
    }
    r
}

/// Why the shutdown should start, or `None`.
pub fn trigger(w: &UpsWatch, r: &UpsReading, on_battery_since: Option<i64>, now: i64) -> Option<String> {
    if !w.enabled || !r.on_battery {
        return None;
    }
    if r.low_battery {
        return Some("the UPS reports low battery".into());
    }
    if let Some(rt) = r.runtime {
        if w.low_runtime_secs > 0 && rt < w.low_runtime_secs {
            return Some(format!("{} of runtime left", words(rt)));
        }
    }
    let since = on_battery_since.unwrap_or(now);
    if w.on_battery_secs > 0 && now - since >= i64::from(w.on_battery_secs) {
        return Some(format!("on battery for {}", words((now - since) as u32)));
    }
    None
}

/// Alerts for one UPS: on battery (warn), low (critical), battery worn out.
pub fn conditions(w: &UpsWatch, r: &UpsReading) -> Vec<Condition> {
    let mut out = Vec::new();
    if !w.enabled || r.error.is_some() {
        return out;
    }
    let base = |suffix: &str, kind, severity, detail: String| Condition {
        key: format!("ups:{}:{}:{suffix}", w.host_id, w.id),
        kind,
        severity,
        title: w.label.clone(),
        detail,
        host_id: Some(w.host_id),
    };
    let left = match (r.charge, r.runtime) {
        (Some(c), Some(t)) => format!(" ({c:.0}%, about {} left)", words(t)),
        (Some(c), None) => format!(" ({c:.0}%)"),
        (None, Some(t)) => format!(" (about {} left)", words(t)),
        _ => String::new(),
    };
    let low = r.low_battery || (w.low_runtime_secs > 0 && r.runtime.is_some_and(|t| t < w.low_runtime_secs));
    if r.on_battery && low {
        out.push(base("power", AlertKind::UpsOnBattery, Severity::Critical, format!("battery low{left}")));
    } else if r.on_battery {
        out.push(base("power", AlertKind::UpsOnBattery, Severity::Warn, format!("running on battery{left}")));
    }
    if r.replace_battery {
        out.push(base(
            "rb",
            AlertKind::UpsOnBattery,
            Severity::Warn,
            "the UPS says its battery needs replacing".into(),
        ));
    }
    out
}

/// "4 min", "1 h 5 min", "45 s".
pub fn words(secs: u32) -> String {
    match secs {
        s if s < 90 => format!("{s} s"),
        s if s < 3600 => format!("{} min", (s + 30) / 60),
        s => {
            let m = (s % 3600 + 30) / 60;
            if m == 0 {
                format!("{} h", s / 3600)
            } else {
                format!("{} h {m} min", s / 3600)
            }
        }
    }
}

/// The order to shut hosts down in: everyone else first, the host that reads
/// the UPS last (it often powers the network or storage the others use).
pub fn shutdown_order(members: &[HostId], ups_host: HostId) -> Vec<HostId> {
    let mut v: Vec<HostId> = members.iter().copied().filter(|h| *h != ups_host).collect();
    if members.contains(&ups_host) {
        v.push(ups_host);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONLINE: &str = "Init SSL without certificate database\nbattery.charge: 100\nbattery.runtime: 1830\n\
        device.model: Back-UPS ES 700G\ninput.voltage: 232.0\nups.load: 21\nups.status: OL CHRG\n";

    fn watch() -> UpsWatch {
        UpsWatch {
            id: 1,
            label: "Rack".into(),
            target: "ups@localhost".into(),
            host_id: 3,
            group_id: Some(2),
            shutdown: true,
            on_battery_secs: 300,
            low_runtime_secs: 600,
            countdown_secs: 60,
            enabled: true,
        }
    }

    #[test]
    fn parses_upsc() {
        let r = parse(ONLINE);
        assert_eq!(r.status, vec!["OL", "CHRG"]);
        assert!(!r.on_battery && !r.low_battery);
        assert_eq!((r.charge, r.runtime, r.load), (Some(100.0), Some(1830), Some(21.0)));
        assert_eq!(r.model.as_deref(), Some("Back-UPS ES 700G"));
        assert_eq!(r.error, None);
        let ob = parse("ups.status: OB DISCHRG LB\nbattery.runtime: 240\n");
        assert!(ob.on_battery && ob.low_battery);
        let bad = parse("Init SSL without certificate database\nError: Driver not connected\n");
        assert_eq!(bad.error.as_deref(), Some("Driver not connected"));
        assert!(parse("NIGHTGRID: no upsc\n").error.unwrap().contains("isn't installed"));
    }

    #[test]
    fn triggers_on_low_battery_runtime_or_time() {
        let w = watch();
        let online = parse(ONLINE);
        assert_eq!(trigger(&w, &online, None, 1000), None);
        let ob = parse("ups.status: OB DISCHRG\nbattery.runtime: 1500\n");
        assert_eq!(trigger(&w, &ob, Some(900), 1000), None);
        assert_eq!(trigger(&w, &ob, Some(600), 1000).as_deref(), Some("on battery for 7 min"));
        let short = parse("ups.status: OB\nbattery.runtime: 420\n");
        assert_eq!(trigger(&w, &short, Some(990), 1000).as_deref(), Some("7 min of runtime left"));
        let lb = parse("ups.status: OB LB\n");
        assert!(trigger(&w, &lb, Some(999), 1000).unwrap().contains("low battery"));
        let off = UpsWatch { on_battery_secs: 0, low_runtime_secs: 0, ..w.clone() };
        assert_eq!(trigger(&off, &ob, Some(0), 100_000), None);
        assert!(trigger(&off, &lb, Some(0), 1).is_some());
        assert_eq!(trigger(&UpsWatch { enabled: false, ..w }, &lb, None, 1), None);
    }

    #[test]
    fn alerts_and_order() {
        let w = watch();
        assert!(conditions(&w, &parse(ONLINE)).is_empty());
        let c = conditions(&w, &parse("ups.status: OB\nbattery.charge: 80\nbattery.runtime: 1500\n"));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].severity, Severity::Warn);
        assert_eq!(c[0].detail, "running on battery (80%, about 25 min left)");
        let c = conditions(&w, &parse("ups.status: OB LB RB\n"));
        assert_eq!(c[0].severity, Severity::Critical);
        assert_eq!(c.len(), 2);
        assert_eq!(shutdown_order(&[3, 1, 2], 3), vec![1, 2, 3]);
        assert_eq!(shutdown_order(&[1, 2], 3), vec![1, 2]);
        assert_eq!(words(3900), "1 h 5 min");
    }

    #[test]
    fn validates_and_stores() {
        assert!(valid_target("ups@nas.lan:3493"));
        assert!(!valid_target("ups; rm -rf /"));
        assert!(!valid_target("-h"));
        let mut st = Store::open_in_memory().unwrap();
        let h = st
            .create_host(&crate::hosts::HostInput {
                name: "nas".into(),
                hostname: "nas.lan".into(),
                ..Default::default()
            })
            .unwrap();
        let input = UpsInput {
            label: "Rack".into(),
            target: "ups@localhost".into(),
            host_id: h.id,
            countdown_secs: 60,
            enabled: true,
            ..Default::default()
        };
        let w = st.ups_save(&input).unwrap();
        assert_eq!(st.ups_list().unwrap(), vec![w.clone()]);
        assert!(st.ups_save(&UpsInput { shutdown: true, ..input.clone() }).is_err(), "needs a group");
        assert!(st.ups_save(&UpsInput { countdown_secs: 1, ..input.clone() }).is_err());
        st.delete_host(h.id).unwrap();
        assert!(st.ups_list().unwrap().is_empty());
    }
}
