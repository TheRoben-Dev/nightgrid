//! Scheduled power actions (M10k): a systemd timer on the host itself, so the
//! schedule runs whether or not Nightgrid is open. Wake-ups are timers on the
//! sleeping host's relay host (it sends the magic packet).
//!
//! Units are named `nightgrid-power-<action>` / `nightgrid-wake-<mac>` and
//! carry their own spec in a `# nightgrid {json}` first line, so Nightgrid
//! can list what it installed without keeping anything locally.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::power::{self, PowerAction};
use crate::{Error, Result};

/// What a schedule does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ScheduledAction {
    Reboot,
    Shutdown,
    Suspend,
    /// Send Wake-on-LAN to `mac` (runs on a relay host).
    Wake,
}

impl ScheduledAction {
    fn slug(self) -> &'static str {
        match self {
            Self::Reboot => "reboot",
            Self::Shutdown => "shutdown",
            Self::Suspend => "suspend",
            Self::Wake => "wake",
        }
    }

    fn power(self) -> Option<PowerAction> {
        match self {
            Self::Reboot => Some(PowerAction::Reboot),
            Self::Shutdown => Some(PowerAction::Shutdown),
            Self::Suspend => Some(PowerAction::Suspend),
            Self::Wake => None,
        }
    }
}

pub const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// When and what. `time` is `HH:MM` in the host's local time; `days` are
/// `mon`…`sun` (empty = every day).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSpec {
    pub action: ScheduledAction,
    pub time: String,
    #[serde(default)]
    pub days: Vec<String>,
    /// The machine to wake (`aa:bb:…`), for `Wake` only.
    #[serde(default)]
    pub mac: Option<String>,
}

impl ScheduleSpec {
    /// Checks and normalizes: `7:5` → `07:05`, days lower-case, sorted,
    /// deduplicated (all seven → empty), MAC formatted.
    pub fn normalized(&self) -> Result<Self> {
        let bad = |field, reason: &str| Error::Invalid { field, reason: reason.into() };
        let (h, m) = self.time.trim().split_once(':').ok_or_else(|| bad("time", "use HH:MM"))?;
        let (h, m): (u8, u8) =
            (h.parse().map_err(|_| bad("time", "use HH:MM"))?, m.parse().map_err(|_| bad("time", "use HH:MM"))?);
        if h > 23 || m > 59 {
            return Err(bad("time", "use HH:MM (00:00 to 23:59)"));
        }
        let mut days = Vec::new();
        for d in &self.days {
            let d = d.trim().to_ascii_lowercase();
            let i = DAYS.iter().position(|x| **x == d).ok_or_else(|| bad("days", "use mon…sun"))?;
            if !days.contains(&i) {
                days.push(i);
            }
        }
        days.sort_unstable();
        let days = if days.len() == 7 { Vec::new() } else { days.into_iter().map(|i| DAYS[i].to_owned()).collect() };
        let mac = match (self.action, &self.mac) {
            (ScheduledAction::Wake, Some(m)) => Some(power::format_mac(&power::parse_mac(m)?)),
            (ScheduledAction::Wake, None) => return Err(bad("mac", "a wake-up needs the machine's MAC address")),
            _ => None,
        };
        Ok(Self { action: self.action, time: format!("{h:02}:{m:02}"), days, mac })
    }

    /// systemd `OnCalendar=` value.
    pub fn on_calendar(&self) -> String {
        let days = self
            .days
            .iter()
            .map(|d| {
                let mut c = d.chars();
                c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(",");
        let when = format!("*-*-* {}:00", self.time);
        if days.is_empty() {
            when
        } else {
            format!("{days} {when}")
        }
    }

    /// The unit name (without `.timer`): one per action, or per MAC for wake-ups.
    pub fn unit(&self) -> String {
        match (&self.action, &self.mac) {
            (ScheduledAction::Wake, Some(m)) => format!("nightgrid-wake-{}", m.replace(':', "")),
            (a, _) => format!("nightgrid-power-{}", a.slug()),
        }
    }

    fn describe(&self) -> String {
        let what = match self.action {
            ScheduledAction::Wake => format!("wake {}", self.mac.as_deref().unwrap_or("?")),
            a => a.power().map(|p| p.verb().to_owned()).unwrap_or_default(),
        };
        let days = if self.days.is_empty() { "every day".to_owned() } else { self.days.join(",") };
        format!("Nightgrid scheduled {what} at {} ({days})", self.time)
    }

    fn exec(&self) -> Result<String> {
        let cmd = match self.action {
            ScheduledAction::Wake => {
                let mac = power::parse_mac(self.mac.as_deref().unwrap_or_default())?;
                power::wake_relay_command(&mac)
            }
            a => a.power().map(|p| p.command().to_owned()).unwrap_or_default(),
        };
        // systemd expands `%` specifiers and `$VAR` in ExecStart; none may appear.
        if cmd.contains('%') || cmd.contains('$') || cmd.contains('\\') {
            return Err(Error::Invalid { field: "action", reason: "command can't go in a unit file".into() });
        }
        Ok(format!("/bin/sh -c \"{}\"", cmd.replace('"', "\\\"")))
    }
}

/// The `.service` and `.timer` files for a (normalized) spec.
pub fn unit_files(spec: &ScheduleSpec) -> Result<(String, String)> {
    let header = format!("# nightgrid {}", serde_json::to_string(spec)?);
    let desc = spec.describe();
    let service =
        format!("{header}\n[Unit]\nDescription={desc}\n\n[Service]\nType=oneshot\nExecStart={}\n", spec.exec()?);
    let timer = format!(
        "{header}\n[Unit]\nDescription={desc}\n\n[Timer]\nOnCalendar={}\nPersistent=false\n\n[Install]\nWantedBy=timers.target\n",
        spec.on_calendar()
    );
    Ok((service, timer))
}

fn valid_unit(unit: &str) -> bool {
    (unit.starts_with("nightgrid-power-") || unit.starts_with("nightgrid-wake-"))
        && unit.len() <= 64
        && unit.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

const DIR: &str = "/etc/systemd/system";

/// Root script that writes the unit files and enables the timer (replacing
/// one with the same name). Prints `installed`.
pub fn install_command(spec: &ScheduleSpec) -> Result<String> {
    let spec = spec.normalized()?;
    let (service, timer) = unit_files(&spec)?;
    let unit = spec.unit();
    let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
    Ok(format!(
        "set -e; command -v systemctl >/dev/null 2>&1 || {{ echo 'scheduling needs systemd on this host' >&2; exit 3; }}; \
printf '%s' '{}' | base64 -d > {DIR}/{unit}.service; \
printf '%s' '{}' | base64 -d > {DIR}/{unit}.timer; \
systemctl daemon-reload; systemctl enable --now {unit}.timer >/dev/null 2>&1; systemctl restart {unit}.timer; echo installed",
        b64(&service),
        b64(&timer)
    ))
}

/// Root script that stops and deletes a Nightgrid timer. Prints `removed`.
pub fn remove_command(unit: &str) -> Result<String> {
    if !valid_unit(unit) {
        return Err(Error::Invalid { field: "unit", reason: "not a Nightgrid schedule".into() });
    }
    Ok(format!(
        "systemctl disable --now {unit}.timer >/dev/null 2>&1; rm -f {DIR}/{unit}.timer {DIR}/{unit}.service; systemctl daemon-reload; echo removed"
    ))
}

/// Lists Nightgrid timers (no root needed): `@@ unit`, the spec line, and the
/// next run as systemd prints it.
pub const LIST_COMMAND: &str =
    "for f in /etc/systemd/system/nightgrid-power-*.timer /etc/systemd/system/nightgrid-wake-*.timer; do \
[ -f \"$f\" ] || continue; u=$(basename \"$f\" .timer); echo \"@@ $u\"; head -n 1 \"$f\"; \
echo \"next: $(systemctl show -p NextElapseUSecRealtime --value \"$u.timer\" 2>/dev/null)\"; done; true";

/// One installed schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct InstalledSchedule {
    pub unit: String,
    pub spec: ScheduleSpec,
    /// Next run, as systemd prints it (host's time zone), when known.
    pub next: Option<String>,
}

/// Parses [`LIST_COMMAND`] output. Units whose first line isn't ours are skipped.
pub fn parse_list(out: &str) -> Vec<InstalledSchedule> {
    let mut res: Vec<InstalledSchedule> = Vec::new();
    let mut unit: Option<String> = None;
    // Index of the entry the current `@@` block produced, if it was ours.
    let mut current: Option<usize> = None;
    for line in out.lines() {
        if let Some(u) = line.strip_prefix("@@ ") {
            unit = Some(u.trim().to_owned());
            current = None;
        } else if let Some(json) = line.strip_prefix("# nightgrid ") {
            if let (Some(u), Ok(spec)) = (unit.take(), serde_json::from_str::<ScheduleSpec>(json)) {
                res.push(InstalledSchedule { unit: u, spec, next: None });
                current = Some(res.len() - 1);
            }
        } else if let (Some(n), Some(i)) = (line.strip_prefix("next:"), current) {
            let n = n.trim();
            if !n.is_empty() && n != "n/a" {
                res[i].next = Some(n.to_owned());
            }
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(action: ScheduledAction, time: &str, days: &[&str], mac: Option<&str>) -> ScheduleSpec {
        ScheduleSpec {
            action,
            time: time.into(),
            days: days.iter().map(|d| d.to_string()).collect(),
            mac: mac.map(Into::into),
        }
    }

    #[test]
    fn normalizes_and_builds_calendars() {
        let s = spec(ScheduledAction::Shutdown, "7:5", &["Fri", "mon", "fri"], None).normalized().unwrap();
        assert_eq!(s.time, "07:05");
        assert_eq!(s.days, ["mon", "fri"]);
        assert_eq!(s.on_calendar(), "Mon,Fri *-*-* 07:05:00");
        assert_eq!(s.unit(), "nightgrid-power-shutdown");
        let every = spec(ScheduledAction::Reboot, "23:30", &DAYS, None).normalized().unwrap();
        assert!(every.days.is_empty());
        assert_eq!(every.on_calendar(), "*-*-* 23:30:00");
        for bad in ["24:00", "12:60", "noon", "12"] {
            assert!(spec(ScheduledAction::Reboot, bad, &[], None).normalized().is_err(), "{bad}");
        }
        assert!(spec(ScheduledAction::Reboot, "1:00", &["someday"], None).normalized().is_err());
        assert!(spec(ScheduledAction::Wake, "7:00", &[], None).normalized().is_err());
        let w = spec(ScheduledAction::Wake, "6:45", &[], Some("AA-BB-CC-00-11-22")).normalized().unwrap();
        assert_eq!(w.mac.as_deref(), Some("aa:bb:cc:00:11:22"));
        assert_eq!(w.unit(), "nightgrid-wake-aabbcc001122");
    }

    #[test]
    fn writes_unit_files_systemd_can_read() {
        let s = spec(ScheduledAction::Suspend, "01:00", &["sat"], None).normalized().unwrap();
        let (service, timer) = unit_files(&s).unwrap();
        assert!(service.starts_with("# nightgrid {"));
        assert!(service.contains("ExecStart=/bin/sh -c \"systemctl suspend"));
        assert!(timer.contains("OnCalendar=Sat *-*-* 01:00:00\n"));
        assert!(timer.contains("Persistent=false"));
        let w = spec(ScheduledAction::Wake, "06:45", &[], Some("aa:bb:cc:00:11:22")).normalized().unwrap();
        let (service, _) = unit_files(&w).unwrap();
        let exec = service.lines().find(|l| l.starts_with("ExecStart=")).unwrap();
        assert!(exec.contains("wakeonlan aa:bb:cc:00:11:22"));
        assert!(exec.contains("\\\"import socket"), "inner quotes escaped: {exec}");
        assert!(!exec.contains('%') && !exec.contains('$'));
    }

    #[test]
    fn install_and_remove_scripts() {
        let s = spec(ScheduledAction::Reboot, "04:00", &["sun"], None);
        let cmd = install_command(&s).unwrap();
        assert!(cmd.contains("> /etc/systemd/system/nightgrid-power-reboot.service"));
        assert!(cmd.contains("enable --now nightgrid-power-reboot.timer"));
        // The unit text only travels base64-encoded inside single quotes.
        assert!(!cmd.contains("OnCalendar"));
        assert!(remove_command("nightgrid-power-reboot")
            .unwrap()
            .contains("rm -f /etc/systemd/system/nightgrid-power-reboot.timer"));
        for bad in ["sshd", "nightgrid-power-x; rm -rf /", "nightgrid-power-../../x", "NIGHTGRID-power-x"] {
            assert!(remove_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_the_list() {
        let s = spec(ScheduledAction::Shutdown, "23:00", &["mon", "tue"], None).normalized().unwrap();
        let (_, timer) = unit_files(&s).unwrap();
        let first = timer.lines().next().unwrap();
        let out = format!(
            "@@ nightgrid-power-shutdown\n{first}\nnext: Mon 2026-11-16 23:00:00 CET\n@@ nightgrid-power-odd\n[Unit]\nnext: \n"
        );
        let l = parse_list(&out);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].unit, "nightgrid-power-shutdown");
        assert_eq!(l[0].spec, s);
        assert_eq!(l[0].next.as_deref(), Some("Mon 2026-11-16 23:00:00 CET"));
        assert!(parse_list("").is_empty());
    }
}
