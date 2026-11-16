//! Custom command widgets (M10x): any read-only command on one of your hosts,
//! shown as a dashboard tile. A parse rule turns the output into a value
//! (the first number, a regex capture, a line count, or just the exit code)
//! and optional thresholds colour it. Widgets refresh on their own, so a
//! command that would change something (rule 5) or needs sudo is refused.

use regex::RegexBuilder;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::automation::danger;
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type CmdWidgetId = i32;

pub const MAX_NAME: usize = 40;
pub const MAX_COMMAND: usize = 1000;
pub const MAX_PATTERN: usize = 300;
pub const MIN_INTERVAL: u32 = 10;
pub const MAX_INTERVAL: u32 = 86_400;
/// Output kept for display, bytes.
pub const MAX_OUTPUT: usize = 4096;
/// Parse rules.
pub const PARSES: &[&str] = &["text", "number", "regex", "lines", "exit"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CmdWidget {
    pub id: CmdWidgetId,
    pub host_id: HostId,
    pub name: String,
    pub command: String,
    /// One of [`PARSES`].
    pub parse: String,
    /// The regex for `parse = "regex"`; its first group (or the whole match) is the value.
    pub pattern: String,
    /// Shown after the value, e.g. `%` or `GB`.
    pub unit: String,
    pub warn: Option<f64>,
    pub crit: Option<f64>,
    /// Thresholds trip when the value falls to or below them (free space, say).
    pub below: bool,
    /// Seconds between refreshes.
    pub interval_secs: u32,
}

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CmdReading {
    pub value: Option<f64>,
    /// The value as text (regex captures that aren't numbers, "OK"/"FAIL").
    pub text: String,
    /// `ok`, `warn`, `crit`, `info` (no thresholds) or `error`.
    pub level: String,
    pub exit: i32,
    /// Output, trimmed to [`MAX_OUTPUT`] bytes (stderr appended when stdout is empty).
    pub output: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: i64,
}

fn inv(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

const ELEVATE: &[&str] = &["sudo", "doas", "su", "pkexec", "run0"];

/// Checks a widget before it is saved.
pub fn validate(w: &CmdWidget) -> Result<()> {
    let name = w.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(inv("name", format!("1 to {MAX_NAME} characters")));
    }
    let cmd = w.command.trim();
    if cmd.is_empty() || cmd.len() > MAX_COMMAND || cmd.contains('\0') {
        return Err(inv("command", format!("1 to {MAX_COMMAND} characters")));
    }
    if cmd.split(|c: char| c.is_whitespace() || ";|&()`".contains(c)).any(|w| ELEVATE.contains(&w)) {
        return Err(inv("command", "widgets run unattended, so sudo (or su/doas) can't be used"));
    }
    let bad = danger::destructive(cmd);
    if !bad.is_empty() {
        return Err(inv(
            "command",
            format!("widgets refresh on their own, so they must only read; this would {}", bad.join(", ")),
        ));
    }
    if !PARSES.contains(&w.parse.as_str()) {
        return Err(inv("parse", format!("one of {}", PARSES.join(", "))));
    }
    if w.parse == "regex" {
        if w.pattern.is_empty() || w.pattern.len() > MAX_PATTERN {
            return Err(inv("pattern", format!("1 to {MAX_PATTERN} characters")));
        }
        compile(&w.pattern).map_err(|e| inv("pattern", e))?;
    }
    if w.unit.chars().count() > 12 || w.unit.chars().any(char::is_control) {
        return Err(inv("unit", "at most 12 characters"));
    }
    for (f, v) in [("warn", w.warn), ("crit", w.crit)] {
        if v.is_some_and(|v| !v.is_finite()) {
            return Err(inv(f, "must be a number"));
        }
    }
    if let (Some(a), Some(b)) = (w.warn, w.crit) {
        let order = if w.below { b <= a } else { a <= b };
        if !order {
            return Err(inv(
                "crit",
                if w.below { "critical must be at or below warning" } else { "critical must be at or above warning" },
            ));
        }
    }
    if !(MIN_INTERVAL..=MAX_INTERVAL).contains(&w.interval_secs) {
        return Err(inv("intervalSecs", format!("{MIN_INTERVAL} s to 24 h")));
    }
    Ok(())
}

fn compile(p: &str) -> std::result::Result<regex::Regex, String> {
    RegexBuilder::new(p).size_limit(1 << 20).build().map_err(|e| e.to_string())
}

/// The first number in `s`: `-12.5`, `42`, `1,234` (thousands commas dropped).
pub fn first_number(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let neg = b[i] == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit);
        if b[i].is_ascii_digit() || neg {
            let start = i;
            let mut dot = false;
            i += 1;
            while i < b.len() {
                let next_digit = b.get(i + 1).is_some_and(u8::is_ascii_digit);
                if b[i].is_ascii_digit() || (b[i] == b',' && !dot && next_digit) {
                    i += 1;
                } else if b[i] == b'.' && !dot && next_digit {
                    dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let t: String = s[start..i].chars().filter(|c| *c != ',').collect();
            return t.parse().ok();
        }
        i += 1;
    }
    None
}

/// Level for a value against the widget's thresholds.
pub fn level(w: &CmdWidget, v: f64) -> &'static str {
    let trips = |t: Option<f64>| t.is_some_and(|t| if w.below { v <= t } else { v >= t });
    if trips(w.crit) {
        "crit"
    } else if trips(w.warn) {
        "warn"
    } else if w.warn.is_some() || w.crit.is_some() {
        "ok"
    } else {
        "info"
    }
}

fn clip(s: &str) -> String {
    if s.len() <= MAX_OUTPUT {
        return s.to_owned();
    }
    let mut end = MAX_OUTPUT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Turns a run's exit code and output into a reading.
pub fn read(w: &CmdWidget, exit: i32, stdout: &str, stderr: &str, at: i64) -> CmdReading {
    let out = stdout.trim_end();
    let shown = if out.trim().is_empty() { stderr.trim_end() } else { out };
    let output = clip(shown);
    let mk = |value: Option<f64>, text: String, level: &str| CmdReading {
        value,
        text,
        level: level.into(),
        exit,
        output: output.clone(),
        at,
    };
    if w.parse == "exit" {
        return if exit == 0 {
            mk(Some(0.0), "OK".into(), "ok")
        } else {
            mk(Some(f64::from(exit)), format!("FAIL ({exit})"), "crit")
        };
    }
    if exit != 0 && out.trim().is_empty() {
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("no output").trim();
        return mk(None, format!("exit {exit}: {first}"), "error");
    }
    let value = match w.parse.as_str() {
        "number" => first_number(out),
        "lines" => Some(out.lines().filter(|l| !l.trim().is_empty()).count() as f64),
        "regex" => {
            let Ok(re) = compile(&w.pattern) else { return mk(None, "bad pattern".into(), "error") };
            match re.captures(out) {
                Some(c) => {
                    let m = c.get(1).or_else(|| c.get(0)).map_or("", |m| m.as_str()).trim().to_owned();
                    match first_number(&m) {
                        Some(v) if m.trim_start_matches('-').starts_with(|c: char| c.is_ascii_digit()) => {
                            return mk(Some(v), m, level(w, v));
                        }
                        _ => return mk(None, m, "info"),
                    }
                }
                None => return mk(None, "no match".into(), "error"),
            }
        }
        _ => {
            let first = out.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_owned();
            return mk(None, first, if exit == 0 { "info" } else { "crit" });
        }
    };
    match value {
        Some(v) => mk(Some(v), fmt_num(v), level(w, v)),
        None => mk(None, "no number in the output".into(), "error"),
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

const COLS: &str = "id, host_id, name, command, parse, pattern, unit, warn, crit, below, interval_secs";

fn row(r: &Row<'_>) -> rusqlite::Result<CmdWidget> {
    Ok(CmdWidget {
        id: r.get(0)?,
        host_id: r.get(1)?,
        name: r.get(2)?,
        command: r.get(3)?,
        parse: r.get(4)?,
        pattern: r.get(5)?,
        unit: r.get(6)?,
        warn: r.get(7)?,
        crit: r.get(8)?,
        below: r.get(9)?,
        interval_secs: r.get(10)?,
    })
}

impl Store {
    pub fn cmd_widgets(&self) -> Result<Vec<CmdWidget>> {
        let mut st = self.conn().prepare(&format!("SELECT {COLS} FROM cmd_widgets ORDER BY name, id"))?;
        let rows = st.query_map([], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn cmd_widget(&self, id: CmdWidgetId) -> Result<Option<CmdWidget>> {
        let mut st = self.conn().prepare(&format!("SELECT {COLS} FROM cmd_widgets WHERE id = ?1"))?;
        let mut rows = st.query_map([id], row)?;
        Ok(rows.next().transpose()?)
    }

    /// Adds (`id == 0`) or updates a widget after [`validate`].
    pub fn save_cmd_widget(&mut self, w: &CmdWidget) -> Result<CmdWidget> {
        validate(w)?;
        let name = w.name.trim();
        let cmd = w.command.trim();
        let id = if w.id == 0 {
            self.conn().execute(
                "INSERT INTO cmd_widgets (host_id, name, command, parse, pattern, unit, warn, crit, below, interval_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![w.host_id, name, cmd, w.parse, w.pattern, w.unit, w.warn, w.crit, w.below, w.interval_secs],
            )?;
            self.conn().last_insert_rowid() as CmdWidgetId
        } else {
            let n = self.conn().execute(
                "UPDATE cmd_widgets SET host_id = ?2, name = ?3, command = ?4, parse = ?5, pattern = ?6, unit = ?7,
                 warn = ?8, crit = ?9, below = ?10, interval_secs = ?11 WHERE id = ?1",
                params![
                    w.id,
                    w.host_id,
                    name,
                    cmd,
                    w.parse,
                    w.pattern,
                    w.unit,
                    w.warn,
                    w.crit,
                    w.below,
                    w.interval_secs
                ],
            )?;
            if n == 0 {
                return Err(inv("id", "no such widget"));
            }
            w.id
        };
        self.cmd_widget(id)?.ok_or_else(|| inv("id", "no such widget"))
    }

    pub fn delete_cmd_widget(&mut self, id: CmdWidgetId) -> Result<bool> {
        Ok(self.conn().execute("DELETE FROM cmd_widgets WHERE id = ?1", [id])? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(parse: &str) -> CmdWidget {
        CmdWidget {
            id: 0,
            host_id: 1,
            name: "Root free".into(),
            command: "df -P / | tail -1".into(),
            parse: parse.into(),
            pattern: String::new(),
            unit: "%".into(),
            warn: Some(80.0),
            crit: Some(90.0),
            below: false,
            interval_secs: 60,
        }
    }

    #[test]
    fn refuses_writes_sudo_and_bad_fields() {
        assert!(validate(&w("number")).is_ok());
        let mut x = w("number");
        x.command = "docker system prune -f".into();
        assert!(validate(&x).unwrap_err().to_string().contains("only read"));
        x.command = "sudo zpool status".into();
        assert!(validate(&x).unwrap_err().to_string().contains("sudo"));
        x.command = "uptime > /tmp/x".into();
        assert!(validate(&x).is_err());
        x.command = "cat /proc/loadavg 2>/dev/null".into();
        assert!(validate(&x).is_ok());
        // The UI's examples and everyday reads pass.
        for ok in [
            "df -P / | tail -1",
            "who",
            "systemctl --failed --no-legend --plain",
            "docker ps -q",
            "zpool status -x",
            "ss -ltn | grep -q ':443 '",
            "grep -c down /var/log/syslog",
            "apt list --upgradable 2>/dev/null | wc -l",
        ] {
            x.command = ok.into();
            assert!(validate(&x).is_ok(), "{ok}");
        }
        x.command = "cat /proc/loadavg 2>/dev/null".into();
        x.parse = "regex".into();
        assert!(validate(&x).is_err());
        x.pattern = "(".into();
        assert!(validate(&x).is_err());
        x.pattern = r"(\d+)".into();
        assert!(validate(&x).is_ok());
        x.warn = Some(95.0);
        assert!(validate(&x).unwrap_err().to_string().contains("above"));
        x.below = true;
        assert!(validate(&x).is_ok());
        x.interval_secs = 5;
        assert!(validate(&x).is_err());
    }

    #[test]
    fn finds_numbers() {
        assert_eq!(first_number("load: 0.42 0.5"), Some(0.42));
        assert_eq!(first_number("temp -3.5C"), Some(-3.5));
        assert_eq!(first_number("1,234 files"), Some(1234.0));
        assert_eq!(first_number("v1.2.3"), Some(1.2));
        assert_eq!(first_number("a-b"), None);
        assert_eq!(first_number("nothing"), None);
    }

    #[test]
    fn reads_outputs() {
        let r = read(&w("number"), 0, "/dev/sda1 100 92 8 92% /\n", "", 5);
        // First number is 1 (from sda1), which is why regex exists.
        assert_eq!(r.value, Some(1.0));
        let mut x = w("regex");
        x.pattern = r"(\d+)%".into();
        let r = read(&x, 0, "/dev/sda1 100 92 8 92% /\n", "", 5);
        assert_eq!((r.value, r.level.as_str(), r.text.as_str()), (Some(92.0), "crit", "92"));
        let r = read(&x, 0, "nothing here", "", 5);
        assert_eq!(r.level, "error");
        x.pattern = r"state: (\w+)".into();
        let r = read(&x, 0, "pool: tank\nstate: ONLINE\n", "", 5);
        assert_eq!((r.value, r.text.as_str(), r.level.as_str()), (None, "ONLINE", "info"));

        let r = read(&w("lines"), 0, "a\n\nb\nc\n", "", 5);
        assert_eq!((r.value, r.level.as_str()), (Some(3.0), "ok"));
        let r = read(&w("exit"), 3, "", "", 5);
        assert_eq!((r.text.as_str(), r.level.as_str()), ("FAIL (3)", "crit"));
        let r = read(&w("number"), 127, "", "sh: foo: not found\n", 5);
        assert_eq!(r.level, "error");
        assert!(r.text.contains("not found") && r.output.contains("not found"));
        let r = read(&w("text"), 0, "\nhello world\nmore", "", 5);
        assert_eq!((r.text.as_str(), r.level.as_str()), ("hello world", "info"));

        let mut free = w("number");
        free.below = true;
        free.warn = Some(20.0);
        free.crit = Some(10.0);
        assert_eq!(read(&free, 0, "15", "", 0).level, "warn");
        assert_eq!(read(&free, 0, "5.5", "", 0).text, "5.5");
        assert_eq!(read(&free, 0, "50", "", 0).level, "ok");
        let big = "x".repeat(MAX_OUTPUT + 10);
        assert!(read(&w("text"), 0, &big, "", 0).output.ends_with('…'));
    }

    #[test]
    fn stores_widgets() {
        let mut s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (id, name, hostname) VALUES (1, 'h', 'h')", []).unwrap();
        let mut x = w("number");
        let saved = s.save_cmd_widget(&x).unwrap();
        assert!(saved.id > 0);
        let mut y = saved.clone();
        y.name = "  Renamed ".into();
        y.warn = None;
        let y = s.save_cmd_widget(&y).unwrap();
        assert_eq!((y.name.as_str(), y.warn), ("Renamed", None));
        assert_eq!(s.cmd_widgets().unwrap().len(), 1);
        x.command = "rm -rf /tmp/x".into();
        assert!(s.save_cmd_widget(&x).is_err());
        assert!(s.delete_cmd_widget(y.id).unwrap());
        assert!(s.cmd_widgets().unwrap().is_empty());
    }
}
