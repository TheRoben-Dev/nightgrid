//! Per-host incident timeline (M10l): what happened on a host, newest first.
//!
//! Two kinds of entries are merged:
//! - **Recorded** by Nightgrid in the local database as they happen: alerts
//!   firing and clearing, power actions, compose runs, deploys, rollbacks,
//!   unit actions, upgrades and automation runs.
//! - **Read from the host** when the timeline is opened (agentless, nothing
//!   installed): boots from the journal, failed units, kernel OOM kills,
//!   package transactions (apt, pacman, dnf) and Docker container events.
//!
//! Only titles and short details are stored; any line that looks like it
//! carries a secret is replaced before it reaches the database (rule 1).

use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::Result;

/// Recorded events older than this are dropped.
pub const KEEP_SECS: i64 = 90 * 86_400;
/// At most this many recorded events per host.
pub const KEEP_ROWS: i64 = 5_000;
/// At most this many entries come back from one read.
pub const LIST_MAX: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    /// An alert fired.
    Alert,
    /// An alert cleared.
    Recovered,
    /// The host booted.
    Boot,
    /// Reboot, shutdown, suspend or wake sent from Nightgrid.
    Power,
    /// A container died, restarted, ran out of memory or turned unhealthy.
    Container,
    /// A systemd unit failed, or was started/stopped/restarted from Nightgrid.
    Unit,
    /// The kernel killed a process for memory.
    Oom,
    /// A package transaction (install, upgrade, removal).
    Packages,
    /// A compose run, deploy, snapshot or rollback.
    Stack,
    /// An automation (snippet or playbook) run.
    Automation,
}

impl EventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            EventKind::Alert => "alert",
            EventKind::Recovered => "recovered",
            EventKind::Boot => "boot",
            EventKind::Power => "power",
            EventKind::Container => "container",
            EventKind::Unit => "unit",
            EventKind::Oom => "oom",
            EventKind::Packages => "packages",
            EventKind::Stack => "stack",
            EventKind::Automation => "automation",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "alert" => EventKind::Alert,
            "recovered" => EventKind::Recovered,
            "boot" => EventKind::Boot,
            "power" => EventKind::Power,
            "container" => EventKind::Container,
            "unit" => EventKind::Unit,
            "oom" => EventKind::Oom,
            "packages" => EventKind::Packages,
            "stack" => EventKind::Stack,
            "automation" => EventKind::Automation,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Info,
    /// Something went back to normal.
    Ok,
    Warn,
    Critical,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Ok => "ok",
            Level::Warn => "warn",
            Level::Critical => "critical",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "ok" => Level::Ok,
            "warn" => Level::Warn,
            "critical" => Level::Critical,
            _ => Level::Info,
        }
    }
}

/// Where an entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// Recorded by Nightgrid.
    Nightgrid,
    /// Read from the host's own logs.
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostEvent {
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    pub kind: EventKind,
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub origin: Origin,
    /// Recorded while the alert was acknowledged or muted (D-198).
    pub silenced: Option<crate::silence::SilenceMode>,
}

impl HostEvent {
    pub fn new(at: i64, kind: EventKind, level: Level, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { at, kind, level, title: title.into(), detail: detail.into(), origin: Origin::Host, silenced: None }
    }
}

/// Trims, caps and drops secret-looking text before anything is stored.
fn tidy(s: &str, max: usize) -> String {
    let s = s.trim();
    if crate::redact::line_has_secret(s) {
        return "(hidden: looked like it contained a secret)".into();
    }
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

impl Store {
    /// Records one event for `host` and trims the host's history.
    pub fn event_add(
        &self,
        host: HostId,
        at: i64,
        kind: EventKind,
        level: Level,
        title: &str,
        detail: &str,
    ) -> Result<()> {
        self.event_add_marked(host, at, kind, level, title, detail, None)
    }

    /// [`Store::event_add`] for an alert that was acknowledged or muted.
    #[allow(clippy::too_many_arguments)]
    pub fn event_add_marked(
        &self,
        host: HostId,
        at: i64,
        kind: EventKind,
        level: Level,
        title: &str,
        detail: &str,
        silenced: Option<crate::silence::SilenceMode>,
    ) -> Result<()> {
        let c = self.conn();
        c.execute(
            "INSERT INTO host_events (host_id, at, kind, level, title, detail, silenced) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![host, at, kind.as_str(), level.as_str(), tidy(title, 200), tidy(detail, 600), silenced.map(|m| m.as_str())],
        )?;
        c.execute(
            "DELETE FROM host_events WHERE host_id = ?1 AND (at < ?2 OR id NOT IN
               (SELECT id FROM host_events WHERE host_id = ?1 ORDER BY at DESC, id DESC LIMIT ?3))",
            params![host, at - KEEP_SECS, KEEP_ROWS],
        )?;
        Ok(())
    }

    /// Recorded events for `host` at or after `since`, newest first.
    pub fn events_for(&self, host: HostId, since: i64) -> Result<Vec<HostEvent>> {
        let mut st = self.conn().prepare(
            "SELECT at, kind, level, title, detail, silenced FROM host_events
             WHERE host_id = ?1 AND at >= ?2 ORDER BY at DESC, id DESC LIMIT ?3",
        )?;
        let rows = st.query_map(params![host, since, LIST_MAX as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (at, kind, level, title, detail, silenced) = row?;
            let Some(kind) = EventKind::parse(&kind) else { continue };
            out.push(HostEvent {
                at,
                kind,
                level: Level::parse(&level),
                title,
                detail,
                origin: Origin::Nightgrid,
                silenced: silenced.as_deref().and_then(crate::silence::SilenceMode::parse),
            });
        }
        Ok(out)
    }

    /// Forgets every recorded event for `host`.
    pub fn events_clear(&self, host: HostId) -> Result<()> {
        self.conn().execute("DELETE FROM host_events WHERE host_id = ?1", [host])?;
        Ok(())
    }
}

// ---- reading the host ------------------------------------------------------

/// The shell script that reads the host's own history since `since` (unix
/// seconds). Read-only, no sudo; sections the user can't read come back empty.
pub fn host_script(since: i64) -> String {
    let since = since.max(0);
    format!(
        r#"S={since}
echo "@@now"; date +%s
echo "@@btime"; awk '/^btime/{{print $2}}' /proc/stat 2>/dev/null
echo "@@journal"; journalctl -n 0 -q --no-pager >/dev/null 2>&1 && echo ok
echo "@@boots"; journalctl --list-boots -o json --no-pager 2>/dev/null | head -c 400000; echo
echo "@@units"; journalctl _PID=1 --since "@$S" -o short-unix --no-pager -q 2>/dev/null | grep -F 'Failed with result' | tail -n 400
echo "@@oom"; journalctl -k --since "@$S" -o short-unix --no-pager -q 2>/dev/null | grep -E 'Out of memory: Kill|oom-kill:|Memory cgroup out of memory' | tail -n 200
echo "@@apt"
if [ -r /var/log/apt/history.log ]; then
  awk '/^Start-Date:/{{if(d!="")print d"\t"c"\t"n; d=$2" "$3; c=""; n=""; next}}
       /^Commandline:/{{sub(/^Commandline: /,""); c=$0; next}}
       /^(Install|Upgrade|Remove|Purge|Downgrade):/{{k=$1; sub(/:$/,"",k); s=$0; m=gsub(/\), /,"",s)+1; n=n" "tolower(k)"="m}}
       END{{if(d!="")print d"\t"c"\t"n}}' /var/log/apt/history.log | tail -n 150 |
  while IFS="$(printf '\t')" read -r d c n; do
    t=$(date -d "$d" +%s 2>/dev/null) || continue
    [ "$t" -ge "$S" ] && printf '%s\t%s\t%s\n' "$t" "$c" "$n"
  done
fi
echo "@@pacman"
[ -r /var/log/pacman.log ] && grep -E '\[ALPM\] (installed|upgraded|removed|downgraded) ' /var/log/pacman.log | tail -n 3000
echo "@@dnf"
if command -v dnf >/dev/null 2>&1 && [ ! -e /run/ostree-booted ]; then
  timeout 20 dnf history list 2>/dev/null | awk -F'|' 'NF>=5 && $3 ~ /[0-9][0-9][0-9][0-9]-/ {{print $3"\t"$2"\t"$4"\t"$5}}' | head -n 80 |
  while IFS="$(printf '\t')" read -r d c a n; do
    t=$(date -d "$d" +%s 2>/dev/null) || continue
    [ "$t" -ge "$S" ] && printf '%s\t%s\t%s\t%s\n' "$t" "$c" "$a" "$n"
  done
fi
echo "@@end"
"#
    )
}

/// What the host's own logs said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostHistory {
    pub events: Vec<HostEvent>,
    /// What couldn't be read, in words.
    pub notes: Vec<String>,
}

fn sections(out: &str) -> HashMap<&str, Vec<&str>> {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut cur: Option<&str> = None;
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("@@") {
            cur = Some(name.trim());
            map.entry(name.trim()).or_default();
        } else if let Some(c) = cur {
            map.entry(c).or_default().push(line);
        }
    }
    map
}

/// `1h 5m`, `3d 2h`, `40s`.
pub fn span(secs: i64) -> String {
    let s = secs.max(0);
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

#[derive(Deserialize)]
struct Boot {
    first_entry: Option<i64>,
    last_entry: Option<i64>,
}

/// Boots from `journalctl --list-boots -o json` (microsecond times); falls
/// back to the current boot from `/proc/stat` when the journal says nothing.
fn boots(json: &str, btime: Option<i64>, since: i64) -> Vec<HostEvent> {
    let json = json.trim();
    let list: Vec<Boot> =
        if json.starts_with('[') { serde_json::from_str(json).unwrap_or_default() } else { Vec::new() };
    let mut times: Vec<(i64, Option<i64>)> =
        list.iter().filter_map(|b| Some((b.first_entry? / 1_000_000, b.last_entry.map(|l| l / 1_000_000)))).collect();
    times.sort_by_key(|t| t.0);
    if times.is_empty() {
        if let Some(b) = btime {
            times.push((b, None));
        }
    }
    let mut out = Vec::new();
    for (i, (first, _)) in times.iter().enumerate() {
        if *first < since {
            continue;
        }
        let detail = match i.checked_sub(1).and_then(|p| times[p].1) {
            Some(prev_last) if *first > prev_last => {
                format!("down for about {} (last log before it at the previous boot's end)", span(first - prev_last))
            }
            _ => String::new(),
        };
        out.push(HostEvent::new(*first, EventKind::Boot, Level::Info, "Booted", detail));
    }
    out
}

/// `1731657600.123456 host systemd[1]: <message>` → (time, message).
fn journal_line(line: &str) -> Option<(i64, &str)> {
    let (ts, rest) = line.trim().split_once(' ')?;
    let at = ts.split('.').next()?.parse::<i64>().ok()?;
    let msg = rest.split_once("]: ").map(|(_, m)| m).or_else(|| rest.split_once(": ").map(|(_, m)| m))?;
    Some((at, msg.trim()))
}

fn failed_units(lines: &[&str]) -> Vec<HostEvent> {
    lines
        .iter()
        .filter_map(|l| {
            let (at, msg) = journal_line(l)?;
            let (unit, why) = msg.split_once(": Failed with result")?;
            let why = why.trim().trim_matches(|c| c == '\'' || c == '.' || c == ' ');
            Some(HostEvent::new(at, EventKind::Unit, Level::Warn, format!("{unit} failed"), format!("result: {why}")))
        })
        .collect()
}

fn oom_kills(lines: &[&str]) -> Vec<HostEvent> {
    let mut out: Vec<HostEvent> = Vec::new();
    for l in lines {
        let Some((at, msg)) = journal_line(l) else { continue };
        // "Out of memory: Killed process 1234 (java) total-vm:…"
        let victim = msg
            .split_once("process ")
            .and_then(|(_, r)| r.split_once('(').and_then(|(_, n)| n.split_once(')')).map(|(n, _)| n.to_owned()));
        let title = match &victim {
            Some(v) => format!("Out of memory: killed {v}"),
            None => "Out of memory".into(),
        };
        // The kernel logs several lines per kill; keep one.
        if out.last().is_some_and(|p| p.title == title && (at - p.at).abs() <= 5) {
            continue;
        }
        if victim.is_none() && out.last().is_some_and(|p| (at - p.at).abs() <= 5) {
            continue;
        }
        out.push(HostEvent::new(at, EventKind::Oom, Level::Critical, title, String::new()));
    }
    out
}

fn counts_text(n: &str) -> String {
    // " upgrade=5 install=2" → "upgraded 5, installed 2"
    n.split_whitespace()
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            let verb = match k {
                "install" => "installed",
                "upgrade" => "upgraded",
                "remove" | "purge" => "removed",
                "downgrade" => "downgraded",
                _ => return None,
            };
            Some(format!("{verb} {v}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn apt(lines: &[&str]) -> Vec<HostEvent> {
    lines
        .iter()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let at = f.next()?.trim().parse::<i64>().ok()?;
            let cmd = f.next().unwrap_or("").trim();
            let counts = counts_text(f.next().unwrap_or(""));
            let title = if counts.is_empty() { "Packages changed (apt)".to_owned() } else { format!("apt: {counts}") };
            Some(HostEvent::new(at, EventKind::Packages, Level::Info, title, tidy(cmd, 300)))
        })
        .collect()
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-11-10T08:00:01+0100` (pacman) → unix seconds.
pub(crate) fn iso_offset(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    let off = match s.get(19..) {
        Some(z) if z.len() >= 5 && (z.starts_with('+') || z.starts_with('-')) => {
            let digits: String = z[1..].chars().filter(char::is_ascii_digit).collect();
            let hh = digits.get(0..2)?.parse::<i64>().ok()?;
            let mm = digits.get(2..4).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
            let v = hh * 3600 + mm * 60;
            if z.starts_with('-') {
                -v
            } else {
                v
            }
        }
        _ => 0,
    };
    Some(days_from_civil(y, mo as u32, d as u32) * 86_400 + h * 3600 + mi * 60 + se - off)
}

/// A pacman transaction: first and last time, counts by verb, some names.
type Txn = (i64, i64, HashMap<&'static str, u32>, Vec<String>);

/// pacman.log lines, grouped into transactions (lines less than two minutes
/// apart).
fn pacman(lines: &[&str], since: i64) -> Vec<HostEvent> {
    let mut out: Vec<Txn> = Vec::new();
    for l in lines {
        let Some(rest) = l.strip_prefix('[') else { continue };
        let Some((ts, rest)) = rest.split_once(']') else { continue };
        let Some(at) = iso_offset(ts) else { continue };
        if at < since {
            continue;
        }
        let Some(rest) = rest.trim().strip_prefix("[ALPM] ") else { continue };
        let Some((verb, pkg)) = rest.split_once(' ') else { continue };
        let verb: &'static str = match verb {
            "installed" => "installed",
            "upgraded" => "upgraded",
            "removed" => "removed",
            "downgraded" => "downgraded",
            _ => continue,
        };
        let name = pkg.split_whitespace().next().unwrap_or("").to_owned();
        match out.last_mut() {
            Some(t) if at - t.1 <= 120 => {
                t.1 = at;
                *t.2.entry(verb).or_default() += 1;
                if t.3.len() < 8 {
                    t.3.push(name);
                }
            }
            _ => out.push((at, at, HashMap::from([(verb, 1)]), vec![name])),
        }
    }
    out.into_iter()
        .map(|(at, _, counts, names)| {
            let mut c: Vec<_> = counts.into_iter().collect();
            c.sort();
            let text = c.iter().map(|(v, n)| format!("{v} {n}")).collect::<Vec<_>>().join(", ");
            let total: u32 = c.iter().map(|x| x.1).sum();
            let mut detail = names.join(", ");
            if total as usize > names.len() {
                detail.push_str(", …");
            }
            HostEvent::new(at, EventKind::Packages, Level::Info, format!("pacman: {text}"), detail)
        })
        .collect()
}

fn dnf(lines: &[&str]) -> Vec<HostEvent> {
    lines
        .iter()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let at = f.next()?.trim().parse::<i64>().ok()?;
            let cmd = f.next().unwrap_or("").trim();
            let action = f.next().unwrap_or("").trim();
            let n = f.next().unwrap_or("").trim();
            let title = format!("dnf: {} {}", action.to_lowercase(), n).trim().to_owned();
            Some(HostEvent::new(at, EventKind::Packages, Level::Info, title, tidy(cmd, 300)))
        })
        .collect()
}

/// Parses the output of [`host_script`].
pub fn parse_host(out: &str, since: i64) -> HostHistory {
    let s = sections(out);
    let get = |k: &str| s.get(k).map(Vec::as_slice).unwrap_or(&[]);
    let btime = get("btime").first().and_then(|l| l.trim().parse::<i64>().ok());
    let journal_ok = get("journal").iter().any(|l| l.trim() == "ok");
    let mut events = boots(&get("boots").join("\n"), btime, since);
    events.extend(failed_units(get("units")).into_iter().filter(|e| e.at >= since));
    events.extend(oom_kills(get("oom")).into_iter().filter(|e| e.at >= since));
    events.extend(apt(get("apt")));
    events.extend(pacman(get("pacman"), since));
    events.extend(dnf(get("dnf")));
    let mut notes = Vec::new();
    if !journal_ok {
        notes.push(
            "The system journal isn't readable for this user (add it to the systemd-journal or adm group): \
             failed units, OOM kills and older boots are missing."
                .into(),
        );
    }
    if !s.contains_key("end") {
        notes.push("The host's history was cut short; some entries may be missing.".into());
    }
    HostHistory { events, notes }
}

// ---- Docker ------------------------------------------------------------------

/// One container event as Docker reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerEvent {
    pub at: i64,
    /// `die`, `start`, `restart`, `oom`, `health_status: unhealthy`, …
    pub action: String,
    pub name: String,
    pub exit_code: Option<i32>,
}

fn exit_text(code: i32) -> String {
    match code {
        137 => "exit 137 (killed, maybe out of memory)".into(),
        143 => "exit 143 (stopped)".into(),
        n => format!("exit {n}"),
    }
}

/// Turns raw container events into timeline entries: a `die` followed by a
/// `start` of the same container within two minutes is one "restarted"
/// entry; clean stops are info, crashes warn, OOM is critical.
pub fn container_events(mut raw: Vec<DockerEvent>) -> Vec<HostEvent> {
    raw.sort_by_key(|e| e.at);
    let mut out: Vec<HostEvent> = Vec::new();
    // name → index in `out` of a die that a start may still merge into.
    let mut open_die: HashMap<String, usize> = HashMap::new();
    let mut unhealthy: HashMap<String, bool> = HashMap::new();
    for e in raw {
        let action = e.action.as_str();
        match action {
            "die" => {
                let code = e.exit_code.unwrap_or(0);
                let (level, word) = match code {
                    0 | 143 => (Level::Info, "stopped"),
                    _ => (Level::Warn, "exited"),
                };
                let title = format!("{} {word}", e.name);
                let detail = if code == 0 { String::new() } else { exit_text(code) };
                open_die.insert(e.name.clone(), out.len());
                out.push(HostEvent::new(e.at, EventKind::Container, level, title, detail));
            }
            "oom" => out.push(HostEvent::new(
                e.at,
                EventKind::Container,
                Level::Critical,
                format!("{} ran out of memory", e.name),
                String::new(),
            )),
            "start" => {
                if let Some(i) = open_die.remove(&e.name) {
                    if e.at - out[i].at <= 120 {
                        let was = out[i].detail.clone();
                        let crashed = out[i].level != Level::Info;
                        out[i] = HostEvent::new(
                            out[i].at,
                            EventKind::Container,
                            if crashed { Level::Warn } else { Level::Info },
                            format!("{} restarted", e.name),
                            if was.is_empty() { String::new() } else { format!("after {was}") },
                        );
                        continue;
                    }
                }
                out.push(HostEvent::new(e.at, EventKind::Container, Level::Info, format!("{} started", e.name), ""));
            }
            "restart" => {
                // `docker restart` sends die + start + restart; the merged
                // entry already says it.
                if out.last().is_some_and(|p| p.title == format!("{} restarted", e.name) && e.at - p.at <= 120) {
                    continue;
                }
                out.push(HostEvent::new(e.at, EventKind::Container, Level::Info, format!("{} restarted", e.name), ""));
            }
            a if a.starts_with("health_status") => {
                let bad = a.ends_with("unhealthy");
                let was_bad = unhealthy.insert(e.name.clone(), bad).unwrap_or(false);
                if bad && !was_bad {
                    out.push(HostEvent::new(
                        e.at,
                        EventKind::Container,
                        Level::Warn,
                        format!("{} unhealthy", e.name),
                        "",
                    ));
                } else if !bad && was_bad {
                    out.push(HostEvent::new(
                        e.at,
                        EventKind::Container,
                        Level::Ok,
                        format!("{} healthy again", e.name),
                        "",
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

/// Recorded and host-read entries together, newest first, capped.
pub fn merge(mut a: Vec<HostEvent>, b: Vec<HostEvent>) -> Vec<HostEvent> {
    a.extend(b);
    a.sort_by(|x, y| y.at.cmp(&x.at).then_with(|| x.title.cmp(&y.title)));
    a.dedup();
    a.truncate(LIST_MAX);
    a
}

/// Plain-text lines for DRIFT or a copy, oldest first. `fmt` renders a time.
pub fn as_text(events: &[HostEvent], fmt: impl Fn(i64) -> String) -> String {
    let mut lines: Vec<String> = events
        .iter()
        .rev()
        .map(|e| {
            let lvl = match e.level {
                Level::Critical => " [critical]",
                Level::Warn => " [warn]",
                Level::Ok => " [ok]",
                Level::Info => "",
            };
            if e.detail.is_empty() {
                format!("{} {}{lvl}", fmt(e.at), e.title)
            } else {
                format!("{} {}{lvl}: {}", fmt(e.at), e.title, e.detail)
            }
        })
        .collect();
    lines.dedup();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_host() -> (Store, HostId) {
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('a', 'a.example.com')", []).unwrap();
        let id = s.conn().last_insert_rowid() as HostId;
        (s, id)
    }

    #[test]
    fn records_lists_and_trims() {
        let (s, h) = store_with_host();
        let now = 1_800_000_000;
        s.event_add(h, now - KEEP_SECS - 10, EventKind::Alert, Level::Warn, "old", "").unwrap();
        s.event_add(h, now - 60, EventKind::Alert, Level::Critical, "HOST OFFLINE", "no answer").unwrap();
        s.event_add(h, now, EventKind::Recovered, Level::Ok, "HOST OFFLINE", "").unwrap();
        let list = s.events_for(h, 0).unwrap();
        assert_eq!(list.len(), 2, "the 90-day-old entry is gone");
        assert_eq!(list[0].kind, EventKind::Recovered);
        assert_eq!(list[1].level, Level::Critical);
        assert_eq!(list[1].origin, Origin::Nightgrid);
        assert_eq!(s.events_for(h, now - 30).unwrap().len(), 1);
        s.event_add(h, now, EventKind::Automation, Level::Info, "run", "curl -H 'Authorization: Bearer abcdef123456'")
            .unwrap();
        assert!(s.events_for(h, now).unwrap()[0].detail.starts_with("(hidden"));
        s.events_clear(h).unwrap();
        assert!(s.events_for(h, 0).unwrap().is_empty());
    }

    #[test]
    fn deleting_a_host_drops_its_events() {
        let (s, h) = store_with_host();
        s.event_add(h, 1, EventKind::Power, Level::Info, "Rebooted from Nightgrid", "").unwrap();
        s.conn().execute("DELETE FROM hosts WHERE id = ?1", [h]).unwrap();
        let n: i64 = s.conn().query_row("SELECT COUNT(*) FROM host_events", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    const OUT: &str = "@@now\n1800000000\n@@btime\n1799990000\n@@journal\nok\n@@boots\n\
[{\"index\":-1,\"boot_id\":\"a\",\"first_entry\":1799000000000000,\"last_entry\":1799980000000000},\
{\"index\":0,\"boot_id\":\"b\",\"first_entry\":1799990000000000,\"last_entry\":1800000000000000}]\n\
@@units\n1799995000.123 host systemd[1]: backup.service: Failed with result 'exit-code'.\n\
@@oom\n1799996000.5 host kernel: Out of memory: Killed process 4242 (java) total-vm:100kB\n\
1799996000.6 host kernel: oom-kill:constraint=CONSTRAINT_NONE,task=java\n\
@@apt\n1799997000\tapt-get upgrade -y\t upgrade=5 install=1\n\
@@pacman\n[2026-01-01T10:00:00+0100] [ALPM] upgraded linux (6.1 -> 6.2)\n\
[2026-01-01T10:00:30+0100] [ALPM] installed foo (1.0)\n\
@@dnf\n1799998000\tupgrade -y\tUpgrade\t35\n@@end\n";

    #[test]
    fn parses_the_host_history() {
        let h = parse_host(OUT, 1_799_500_000);
        assert!(h.notes.is_empty());
        let titles: Vec<&str> = h.events.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"Booted"), "{titles:?}");
        assert_eq!(
            h.events.iter().filter(|e| e.kind == EventKind::Boot).count(),
            1,
            "the older boot is before `since`"
        );
        let boot = h.events.iter().find(|e| e.kind == EventKind::Boot).unwrap();
        assert_eq!(boot.at, 1_799_990_000);
        assert!(boot.detail.contains("2h 46m"), "{}", boot.detail);
        assert!(titles.contains(&"backup.service failed"));
        assert_eq!(h.events.iter().filter(|e| e.kind == EventKind::Oom).count(), 1);
        assert!(titles.contains(&"Out of memory: killed java"));
        assert!(titles.contains(&"apt: upgraded 5, installed 1"));
        assert!(titles.contains(&"dnf: upgrade 35"));
        // pacman's 2026-01-01 is before `since`.
        assert!(!titles.iter().any(|t| t.starts_with("pacman")));
        let p = parse_host(OUT, 0);
        let pac = p.events.iter().find(|e| e.title.starts_with("pacman")).unwrap();
        assert_eq!(pac.title, "pacman: installed 1, upgraded 1");
        assert_eq!(pac.at, 1_767_258_000);
        assert_eq!(p.events.iter().filter(|e| e.kind == EventKind::Boot).count(), 2);
    }

    #[test]
    fn notes_what_it_couldnt_read() {
        let h = parse_host("@@now\n1\n@@btime\n1700000000\n@@journal\n@@boots\n\n@@units\n", 0);
        assert_eq!(h.events.len(), 1, "falls back to the current boot");
        assert_eq!(h.notes.len(), 2);
    }

    #[test]
    fn script_is_parameterised() {
        let s = host_script(1234);
        assert!(s.starts_with("S=1234\n"));
        assert!(host_script(-5).starts_with("S=0\n"));
    }

    #[test]
    fn iso_times() {
        assert_eq!(iso_offset("1970-01-01T00:00:00+0000"), Some(0));
        assert_eq!(iso_offset("1970-01-01T01:00:00+0100"), Some(0));
        assert_eq!(iso_offset("2026-11-15T21:00:00-0500"), Some(1_794_794_400));
        assert_eq!(iso_offset("junk"), None);
    }

    fn ev(at: i64, action: &str, name: &str, code: Option<i32>) -> DockerEvent {
        DockerEvent { at, action: action.into(), name: name.into(), exit_code: code }
    }

    #[test]
    fn folds_container_events() {
        let out = container_events(vec![
            ev(10, "die", "web", Some(1)),
            ev(12, "start", "web", None),
            ev(20, "die", "db", Some(0)),
            ev(30, "oom", "java", None),
            ev(31, "die", "java", Some(137)),
            ev(40, "health_status: unhealthy", "api", None),
            ev(50, "health_status: unhealthy", "api", None),
            ev(60, "health_status: healthy", "api", None),
            ev(70, "die", "cache", Some(143)),
            ev(71, "start", "cache", None),
            ev(71, "restart", "cache", None),
        ]);
        let t: Vec<(&str, Level)> = out.iter().map(|e| (e.title.as_str(), e.level)).collect();
        assert_eq!(
            t,
            vec![
                ("web restarted", Level::Warn),
                ("db stopped", Level::Info),
                ("java ran out of memory", Level::Critical),
                ("java exited", Level::Warn),
                ("api unhealthy", Level::Warn),
                ("api healthy again", Level::Ok),
                ("cache restarted", Level::Info),
            ]
        );
        assert_eq!(out[0].detail, "after exit 1");
        assert!(out[3].detail.contains("137"));
    }

    #[test]
    fn merges_newest_first_and_writes_text() {
        let a =
            vec![HostEvent { origin: Origin::Nightgrid, ..HostEvent::new(5, EventKind::Alert, Level::Warn, "A", "x") }];
        let b = vec![
            HostEvent::new(9, EventKind::Boot, Level::Info, "Booted", ""),
            HostEvent::new(1, EventKind::Unit, Level::Warn, "u failed", ""),
        ];
        let m = merge(a, b);
        assert_eq!(m.iter().map(|e| e.at).collect::<Vec<_>>(), vec![9, 5, 1]);
        let text = as_text(&m, |t| format!("t{t}"));
        assert_eq!(text, "t1 u failed [warn]\nt5 A [warn]: x\nt9 Booted");
        assert_eq!(span(59), "59s");
        assert_eq!(span(3_700), "1h 1m");
        assert_eq!(span(90_000), "1d 1h");
    }
}
