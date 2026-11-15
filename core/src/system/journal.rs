//! The systemd journal: `journalctl -o json`, optionally following.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::systemd::valid_unit;
use super::{SystemError, SystemResult};
use crate::files::path::shell_quote;

/// Most lines one query may ask for.
pub const MAX_LINES: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct JournalQuery {
    /// Only this unit (`-u`).
    pub unit: Option<String>,
    /// Most verbose priority shown, 0 (emerg) to 7 (debug) (`-p 0..N`).
    pub priority: Option<u8>,
    /// `--since` / `--until`: journalctl time specs such as `-1h`, `today`,
    /// `2026-11-14 08:00`.
    pub since: Option<String>,
    pub until: Option<String>,
    /// Backlog size (`-n`), at most [`MAX_LINES`].
    pub lines: u32,
    /// Keep streaming new entries (`-f`).
    pub follow: bool,
}

/// What journalctl accepts as a time, minus anything a shell would care
/// about (it is quoted anyway).
fn valid_time(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && !s.starts_with("--")
        && s.chars().all(|c| c.is_ascii_alphanumeric() || " :.-+".contains(c))
}

fn blank(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// The journalctl command line for `q` (not yet wrapped for sudo/sh).
pub fn command(q: &JournalQuery) -> SystemResult<String> {
    let mut cmd = format!("journalctl -o json --no-pager --no-hostname -n {}", q.lines.clamp(1, MAX_LINES));
    if let Some(u) = blank(&q.unit) {
        if !valid_unit(u) {
            return Err(SystemError::other(format!("invalid unit name: {u:?}")));
        }
        cmd.push_str(&format!(" -u {}", shell_quote(u)));
    }
    if let Some(p) = q.priority {
        if p > 7 {
            return Err(SystemError::other("priority must be 0–7"));
        }
        cmd.push_str(&format!(" -p 0..{p}"));
    }
    for (flag, v) in [("--since", &q.since), ("--until", &q.until)] {
        if let Some(t) = blank(v) {
            if !valid_time(t) {
                return Err(SystemError::other(format!(
                    "can't use {t:?} as a time (try -1h, today or 2026-11-14 08:00)"
                )));
            }
            cmd.push_str(&format!(" {flag} {}", shell_quote(t)));
        }
    }
    if q.follow {
        if blank(&q.until).is_some() {
            return Err(SystemError::other("follow can't be combined with an end time"));
        }
        cmd.push_str(" -f");
    }
    Ok(cmd)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    /// Unix time in milliseconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub ts: u64,
    /// Syslog priority 0–7 (6 = info if missing).
    pub priority: u8,
    pub unit: Option<String>,
    /// `SYSLOG_IDENTIFIER` (or the command name).
    pub ident: Option<String>,
    pub pid: Option<u32>,
    pub message: String,
}

fn field_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        // Non-UTF-8 or binary fields come as byte arrays.
        Value::Array(a) => {
            let bytes: Vec<u8> = a.iter().filter_map(|b| b.as_u64()).map(|b| b as u8).collect();
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// One `journalctl -o json` line.
pub fn parse_entry(line: &str) -> Option<JournalEntry> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;
    let get = |k: &str| obj.get(k).and_then(field_str);
    let ts = get("__REALTIME_TIMESTAMP").and_then(|s| s.parse::<u64>().ok()).map(|us| us / 1000)?;
    let message = get("MESSAGE").unwrap_or_default();
    let message = crate::docker::strip_ansi(message.trim_end());
    Some(JournalEntry {
        ts,
        priority: get("PRIORITY").and_then(|p| p.parse().ok()).filter(|p| *p <= 7).unwrap_or(6),
        unit: get("_SYSTEMD_UNIT").or_else(|| get("UNIT")),
        ident: get("SYSLOG_IDENTIFIER").or_else(|| get("_COMM")),
        pid: get("_PID").or_else(|| get("SYSLOG_PID")).and_then(|p| p.parse().ok()),
        message,
    })
}

/// journalctl's hint that only the user's own entries are visible.
pub fn is_limited_notice(stderr_line: &str) -> bool {
    let l = stderr_line.to_ascii_lowercase();
    l.contains("not seeing messages from other users")
        || l.contains("insufficient permissions")
        || l.contains("no journal files were opened")
}

/// stderr lines worth showing (journalctl's chatter about permissions is
/// turned into a flag instead).
pub fn is_noise(stderr_line: &str) -> bool {
    let l = stderr_line.trim();
    l.is_empty()
        || l.starts_with("Hint:")
        || l.starts_with("Users in groups")
        || l.starts_with("Pass -q to")
        || l.contains("'systemd-journal'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_commands() {
        let q = JournalQuery { lines: 200, ..Default::default() };
        assert_eq!(command(&q).unwrap(), "journalctl -o json --no-pager --no-hostname -n 200");
        let q = JournalQuery {
            unit: Some("nginx.service".into()),
            priority: Some(4),
            since: Some("-1h".into()),
            until: None,
            lines: 999_999,
            follow: true,
        };
        assert_eq!(
            command(&q).unwrap(),
            "journalctl -o json --no-pager --no-hostname -n 10000 -u 'nginx.service' -p 0..4 --since '-1h' -f"
        );
        let q = JournalQuery {
            since: Some("2026-11-14 08:00".into()),
            until: Some("today".into()),
            lines: 0,
            ..Default::default()
        };
        assert_eq!(
            command(&q).unwrap(),
            "journalctl -o json --no-pager --no-hostname -n 1 --since '2026-11-14 08:00' --until 'today'"
        );
    }

    #[test]
    fn rejects_bad_input() {
        let bad = |q: JournalQuery| assert!(command(&q).is_err(), "{q:?}");
        bad(JournalQuery { unit: Some("x; rm -rf /".into()), ..Default::default() });
        bad(JournalQuery { priority: Some(8), ..Default::default() });
        bad(JournalQuery { since: Some("$(id)".into()), ..Default::default() });
        bad(JournalQuery { since: Some("--output=x".into()), ..Default::default() });
        bad(JournalQuery { until: Some("now".into()), follow: true, ..Default::default() });
    }

    #[test]
    fn parses_entries() {
        let line = r#"{"__REALTIME_TIMESTAMP":"1731571200123456","PRIORITY":"3","_SYSTEMD_UNIT":"nginx.service","SYSLOG_IDENTIFIER":"nginx","_PID":"812","MESSAGE":"bind() to 0.0.0.0:80 failed (98: Address already in use)","_HOSTNAME":"server01"}"#;
        let e = parse_entry(line).unwrap();
        assert_eq!(e.ts, 1_731_571_200_123);
        assert_eq!(e.priority, 3);
        assert_eq!(e.unit.as_deref(), Some("nginx.service"));
        assert_eq!(e.ident.as_deref(), Some("nginx"));
        assert_eq!(e.pid, Some(812));
        assert!(e.message.starts_with("bind()"));

        // Binary message, no priority, kernel line.
        let line =
            r#"{"__REALTIME_TIMESTAMP":"1731571200000000","MESSAGE":[104,105,27,91,49,109,33],"_COMM":"kernel"}"#;
        let e = parse_entry(line).unwrap();
        assert_eq!((e.priority, e.message.as_str(), e.ident.as_deref()), (6, "hi!", Some("kernel")));

        // Null message (journald drops oversized fields).
        let e = parse_entry(r#"{"__REALTIME_TIMESTAMP":"1","MESSAGE":null}"#).unwrap();
        assert_eq!(e.message, "");

        assert!(parse_entry("-- No entries --").is_none());
        assert!(parse_entry(r#"{"MESSAGE":"no time"}"#).is_none());
    }

    #[test]
    fn notices() {
        assert!(is_limited_notice("Hint: You are currently not seeing messages from other users and the system."));
        assert!(is_limited_notice("No journal files were opened due to insufficient permissions."));
        assert!(!is_limited_notice("Failed to add filter"));
        assert!(is_noise("      Users in groups 'adm', 'systemd-journal', 'wheel' can see all messages."));
        assert!(!is_noise("Failed to parse timestamp: x"));
    }
}
