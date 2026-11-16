//! Scheduled jobs on a host: systemd timers and cron (M10c). Read-only: one
//! script lists the timers with their last result, the user's crontab,
//! `/etc/crontab`, `/etc/cron.d/*` and the `cron.{hourly,daily,...}`
//! folders. Cron schedules are parsed here so the UI can say "daily at
//! 03:00" and when each job runs next (in the host's clock and offset).

use serde::Serialize;

use super::{SystemError, SystemResult};
use crate::ssh::client::Connection;

/// One systemd timer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    pub name: String,
    pub description: String,
    /// `active` / `inactive` / `failed`.
    pub active: String,
    /// `enabled` / `disabled` / `static` …, when known.
    pub enabled: Option<String>,
    /// The unit it starts (usually the `.service` of the same name).
    pub unit: String,
    /// `OnCalendar=` expressions.
    pub calendar: Vec<String>,
    /// `OnBootSec=` / `OnUnitActiveSec=` … settings.
    pub monotonic: Vec<String>,
    /// Next run, unix seconds.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub next: Option<f64>,
    /// Last run, unix seconds.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub last: Option<f64>,
    /// The started unit's last result (`success`, `exit-code`, `timeout` …).
    pub result: Option<String>,
    pub exit_status: Option<i32>,
}

/// One cron entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CronJob {
    /// `crontab` (the login user's), `/etc/crontab` or `/etc/cron.d/<file>`.
    pub source: String,
    /// The user it runs as (system crontabs name one).
    pub user: Option<String>,
    /// The schedule as written (`*/5 * * * *`, `@daily`).
    pub schedule: String,
    /// Plain-language schedule ("every 5 minutes", "daily at 03:00").
    pub every: String,
    pub command: String,
    /// Next run, unix seconds (none for `@reboot` or a schedule that never matches).
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub next: Option<f64>,
}

/// Scripts in `/etc/cron.<period>/` (run by run-parts or anacron).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Periodic {
    /// `hourly`, `daily`, `weekly` or `monthly`.
    pub period: String,
    pub scripts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Schedules {
    pub systemd: bool,
    pub timers: Vec<Timer>,
    pub cron: Vec<CronJob>,
    pub periodic: Vec<Periodic>,
    /// The host's clock when it was read, unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub now: f64,
    /// The host's UTC offset in minutes (cron runs in local time).
    pub utc_offset_min: i32,
}

const TIMER_PROPS: &str = "Id,Description,ActiveState,UnitFileState,Unit,TimersCalendar,TimersMonotonic,NextElapseUSecRealtime,LastTriggerUSec";

/// Everything in one round trip. Unprivileged: other users' crontabs in
/// `/var/spool/cron` need root and aren't read.
pub fn scan_command() -> String {
    super::sudo::sh(&format!(
        r#"LC_ALL=C; export LC_ALL
echo @@now; date +%s; date +%z
echo @@timers
if command -v systemctl >/dev/null 2>&1; then
  T=$( (systemctl list-units --type=timer --all --no-legend --plain; systemctl list-unit-files --type=timer --no-legend) 2>/dev/null | tr -s ' ' '\n' | grep '\.timer$' | grep -v '@\.timer$' | sort -u)
  if [ -n "$T" ]; then
    systemctl show --timestamp=unix -p {TIMER_PROPS} -- $T 2>/dev/null || systemctl show -p {TIMER_PROPS} -- $T 2>/dev/null
    echo @@services
    S=$(systemctl show -p Unit --value -- $T 2>/dev/null | sort -u)
    [ -n "$S" ] && systemctl show -p Id,Result,ExecMainStatus -- $S 2>/dev/null
  fi
else
  echo nosystemd
fi
echo @@crontab; crontab -l 2>/dev/null
echo @@etc; cat /etc/crontab 2>/dev/null
for f in /etc/cron.d/*; do [ -f "$f" ] && [ -r "$f" ] && {{ echo "@@file $f"; cat "$f"; }}; done
echo @@periodic
for p in hourly daily weekly monthly; do [ -d /etc/cron.$p ] && for f in /etc/cron.$p/*; do [ -f "$f" ] && echo "$p|${{f##*/}}"; done; done
echo @@end
"#
    ))
}

pub async fn scan(conn: &Connection) -> SystemResult<Schedules> {
    let (_, out, _) = conn.exec(&scan_command()).await.map_err(SystemError::other)?;
    Ok(parse_scan(&String::from_utf8_lossy(&out)))
}

/// Splits the output into (`name`, lines) sections in order (cron.d files
/// repeat as `file <path>`).
fn sections(out: &str) -> Vec<(&str, Vec<&str>)> {
    let mut v: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("@@") {
            v.push((name.trim(), Vec::new()));
        } else if let Some(last) = v.last_mut() {
            last.1.push(line);
        }
    }
    v
}

pub fn parse_scan(out: &str) -> Schedules {
    let secs = sections(out);
    let get = |k: &str| secs.iter().find(|(n, _)| *n == k).map(|(_, l)| l.as_slice()).unwrap_or(&[]);
    let now_l = get("now");
    let now = now_l.first().and_then(|l| l.trim().parse::<f64>().ok()).unwrap_or(0.0);
    let utc_offset_min = now_l.get(1).and_then(|l| parse_offset(l.trim())).unwrap_or(0);
    let timers_raw = get("timers");
    let systemd = !timers_raw.iter().any(|l| l.trim() == "nosystemd");
    let mut timers = parse_timers(timers_raw);
    let results = parse_blocks(get("services"));
    for t in &mut timers {
        if let Some(r) = results.iter().find(|b| first(b, "Id") == Some(t.unit.as_str())) {
            t.result = first(r, "Result").filter(|s| !s.is_empty()).map(str::to_owned);
            t.exit_status = first(r, "ExecMainStatus").and_then(|s| s.parse().ok());
        }
    }
    timers.sort_by(|a, b| a.next.unwrap_or(f64::MAX).total_cmp(&b.next.unwrap_or(f64::MAX)).then(a.name.cmp(&b.name)));

    let local_now = now as i64 + i64::from(utc_offset_min) * 60;
    let to_unix = |local: i64| (local - i64::from(utc_offset_min) * 60) as f64;
    let mut cron = Vec::new();
    for (name, lines) in &secs {
        let (source, system) = match *name {
            "crontab" => ("crontab".to_owned(), false),
            "etc" => ("/etc/crontab".to_owned(), true),
            n if n.starts_with("file ") => (n[5..].trim().to_owned(), true),
            _ => continue,
        };
        for line in lines {
            if let Some(mut job) = parse_cron_line(line, &source, system) {
                job.next = CronSpec::parse(&job.schedule).and_then(|s| s.next_after(local_now)).map(to_unix);
                cron.push(job);
            }
        }
    }
    cron.sort_by(|a, b| a.next.unwrap_or(f64::MAX).total_cmp(&b.next.unwrap_or(f64::MAX)));

    let mut periodic: Vec<Periodic> = Vec::new();
    for l in get("periodic") {
        let Some((p, f)) = l.split_once('|') else { continue };
        match periodic.iter_mut().find(|x| x.period == p) {
            Some(x) => x.scripts.push(f.to_owned()),
            None => periodic.push(Periodic { period: p.to_owned(), scripts: vec![f.to_owned()] }),
        }
    }
    Schedules { systemd, timers, cron, periodic, now, utc_offset_min }
}

/// `+0130` → 90, `-0500` → -300.
fn parse_offset(s: &str) -> Option<i32> {
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    if rest.len() != 4 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h: i32 = rest[..2].parse().ok()?;
    let m: i32 = rest[2..].parse().ok()?;
    Some(sign * (h * 60 + m))
}

/// `systemctl show` output: `Key=value` blocks separated by blank lines.
/// Keys can repeat (several `TimersCalendar=` lines).
fn parse_blocks(lines: &[&str]) -> Vec<Vec<(String, String)>> {
    let mut blocks = Vec::new();
    let mut cur: Vec<(String, String)> = Vec::new();
    for l in lines {
        if l.trim().is_empty() {
            if !cur.is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            cur.push((k.to_owned(), v.to_owned()));
        }
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    blocks
}

fn first<'a>(b: &'a [(String, String)], key: &str) -> Option<&'a str> {
    b.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// `@1731650400` → 1731650400. Older systemd without `--timestamp=unix`
/// prints a local date we don't try to parse.
fn unix_ts(v: &str) -> Option<f64> {
    let n: f64 = v.trim().strip_prefix('@')?.parse().ok()?;
    (n > 0.0).then_some(n)
}

/// `{ OnCalendar=*-*-* 06:00:00 ; next_elapse=… }` → `OnCalendar=*-*-* 06:00:00`.
fn timer_setting(v: &str) -> Option<String> {
    let inner = v.trim().trim_start_matches('{').trim_end_matches('}').trim();
    let s = inner.split(" ; ").next()?.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

fn parse_timers(lines: &[&str]) -> Vec<Timer> {
    parse_blocks(lines)
        .into_iter()
        .filter_map(|b| {
            let name = first(&b, "Id")?.to_owned();
            if !name.ends_with(".timer") {
                return None;
            }
            let all = |key: &str| -> Vec<String> {
                b.iter().filter(|(k, _)| k == key).filter_map(|(_, v)| timer_setting(v)).collect()
            };
            let calendar = all("TimersCalendar")
                .into_iter()
                .map(|s| s.strip_prefix("OnCalendar=").unwrap_or(&s).to_owned())
                .collect();
            Some(Timer {
                description: first(&b, "Description").unwrap_or_default().to_owned(),
                active: first(&b, "ActiveState").unwrap_or("unknown").to_owned(),
                enabled: first(&b, "UnitFileState").filter(|s| !s.is_empty()).map(str::to_owned),
                unit: first(&b, "Unit").unwrap_or_default().to_owned(),
                calendar,
                monotonic: all("TimersMonotonic"),
                next: first(&b, "NextElapseUSecRealtime").and_then(unix_ts),
                last: first(&b, "LastTriggerUSec").and_then(unix_ts),
                result: None,
                exit_status: None,
                name,
            })
        })
        .collect()
}

/// Parses one crontab line. System crontabs have a user field after the
/// schedule. Comments, blank lines and `NAME=value` settings give `None`.
pub fn parse_cron_line(line: &str, source: &str, system: bool) -> Option<CronJob> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    // Environment settings: `MAILTO=root`, `PATH = /usr/bin`.
    if let Some((k, _)) = t.split_once('=') {
        let k = k.trim();
        if !k.is_empty() && !k.contains(char::is_whitespace) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }
    }
    let mut rest = t;
    let mut take = || -> Option<&str> {
        let r = rest.trim_start();
        let end = r.find(char::is_whitespace).unwrap_or(r.len());
        let (tok, after) = r.split_at(end);
        rest = after;
        (!tok.is_empty()).then_some(tok)
    };
    let first_tok = take()?;
    let schedule = if first_tok.starts_with('@') {
        first_tok.to_owned()
    } else {
        let mut f = vec![first_tok];
        for _ in 0..4 {
            f.push(take()?);
        }
        f.join(" ")
    };
    let user = if system { Some(take()?.to_owned()) } else { None };
    let command = rest.trim().to_owned();
    if command.is_empty() {
        return None;
    }
    let every = describe(&schedule);
    Some(CronJob { source: source.to_owned(), user, schedule, every, command, next: None })
}

/// A parsed 5-field cron schedule as bit sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSpec {
    minutes: u64,
    hours: u32,
    /// Bits 1..=31.
    dom: u32,
    /// Bits 1..=12.
    months: u16,
    /// Bits 0..=6, Sunday = 0.
    dow: u8,
    dom_any: bool,
    dow_any: bool,
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

fn field(s: &str, lo: u32, hi: u32, names: &[&str], name_base: u32) -> Option<u64> {
    let num = |t: &str| -> Option<u32> {
        if let Ok(n) = t.parse::<u32>() {
            return Some(n);
        }
        let l = t.to_ascii_lowercase();
        names.iter().position(|n| *n == l).map(|i| i as u32 + name_base)
    };
    let mut bits = 0u64;
    for item in s.split(',') {
        let (range, step) = match item.split_once('/') {
            Some((r, st)) => (r, st.parse::<u32>().ok().filter(|n| *n > 0)?),
            None => (item, 1),
        };
        let (a, b) = if range == "*" {
            (lo, hi)
        } else if let Some((x, y)) = range.split_once('-') {
            (num(x)?, num(y)?)
        } else {
            let a = num(range)?;
            // `5/15` means 5-hi every 15.
            (a, if item.contains('/') { hi } else { a })
        };
        if a < lo || b > hi || a > b {
            return None;
        }
        let mut v = a;
        while v <= b {
            bits |= 1 << v;
            v += step;
        }
    }
    Some(bits)
}

impl CronSpec {
    pub fn parse(schedule: &str) -> Option<Self> {
        let s = match schedule.trim() {
            "@yearly" | "@annually" => "0 0 1 1 *",
            "@monthly" => "0 0 1 * *",
            "@weekly" => "0 0 * * 0",
            "@daily" | "@midnight" => "0 0 * * *",
            "@hourly" => "0 * * * *",
            s if s.starts_with('@') => return None,
            s => s,
        };
        let f: Vec<&str> = s.split_whitespace().collect();
        if f.len() != 5 {
            return None;
        }
        let dow7 = field(f[4], 0, 7, &DAYS, 0)?;
        Some(Self {
            minutes: field(f[0], 0, 59, &[], 0)?,
            hours: field(f[1], 0, 23, &[], 0)? as u32,
            dom: field(f[2], 1, 31, &[], 0)? as u32,
            months: field(f[3], 1, 12, &MONTHS, 1)? as u16,
            dow: ((dow7 | (dow7 >> 7)) & 0x7f) as u8,
            dom_any: f[2].starts_with('*'),
            dow_any: f[4].starts_with('*'),
        })
    }

    fn day_matches(&self, month: u32, day: u32, weekday: u32) -> bool {
        if self.months & (1 << month) == 0 {
            return false;
        }
        let d = self.dom & (1 << day) != 0;
        let w = self.dow & (1 << weekday) != 0;
        // Classic cron: when both are restricted, either one matching is enough.
        match (self.dom_any, self.dow_any) {
            (true, true) => true,
            (true, false) => w,
            (false, true) => d,
            (false, false) => d || w,
        }
    }

    /// The first matching minute strictly after `local` (seconds in the
    /// host's local time, counted like unix time), looking a bit over four
    /// years ahead (leap days).
    pub fn next_after(&self, local: i64) -> Option<i64> {
        let start = local.div_euclid(60) * 60 + 60;
        let first_day = start.div_euclid(86400);
        let mut first_min = start.rem_euclid(86400) / 60;
        for day in first_day..first_day + 366 * 4 + 1 {
            let (_, m, d) = civil_from_days(day);
            let wd = (day + 4).rem_euclid(7) as u32;
            if self.day_matches(m, d, wd) {
                for min_of_day in first_min..1440 {
                    let (h, mi) = (min_of_day / 60, min_of_day % 60);
                    if self.hours & (1 << h) != 0 && self.minutes & (1 << mi) != 0 {
                        return Some(day * 86400 + min_of_day * 60);
                    }
                }
            }
            first_min = 0;
        }
        None
    }
}

/// Days since 1970-01-01 → (year, month 1-12, day 1-31). Howard Hinnant's
/// algorithm.
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A plain-language version of common schedules; anything unusual is
/// returned as written.
pub fn describe(schedule: &str) -> String {
    match schedule.trim() {
        "@reboot" => return "at boot".into(),
        "@yearly" | "@annually" => return "yearly on 1 Jan at 00:00".into(),
        "@monthly" => return "monthly on the 1st at 00:00".into(),
        "@weekly" => return "weekly on Sun at 00:00".into(),
        "@daily" | "@midnight" => return "daily at 00:00".into(),
        "@hourly" => return "hourly at :00".into(),
        _ => {}
    }
    let f: Vec<&str> = schedule.split_whitespace().collect();
    if f.len() != 5 || CronSpec::parse(schedule).is_none() {
        return schedule.to_owned();
    }
    let num = |s: &str| s.parse::<u32>().ok();
    let step = |s: &str| s.strip_prefix("*/").and_then(|n| n.parse::<u32>().ok());
    let (mi, h, dom, mon, dow) = (f[0], f[1], f[2], f[3], f[4]);
    let rest_any = dom == "*" && mon == "*" && dow == "*";
    if rest_any && h == "*" {
        if mi == "*" {
            return "every minute".into();
        }
        if let Some(n) = step(mi) {
            return format!("every {n} minutes");
        }
        if let Some(m) = num(mi) {
            return format!("hourly at :{m:02}");
        }
    }
    if rest_any {
        if let (Some(m), Some(n)) = (num(mi), step(h)) {
            return format!("every {n} hours at :{m:02}");
        }
    }
    let (Some(m), Some(hh)) = (num(mi), num(h)) else {
        return schedule.to_owned();
    };
    let at = format!("{hh:02}:{m:02}");
    let day_name = |t: &str| -> Option<String> {
        let i =
            t.parse::<usize>().ok().map(|n| n % 7).or_else(|| DAYS.iter().position(|d| d.eq_ignore_ascii_case(t)))?;
        let n = DAYS[i];
        Some(format!("{}{}", n[..1].to_ascii_uppercase(), &n[1..]))
    };
    if mon == "*" && dom == "*" {
        if dow == "*" {
            return format!("daily at {at}");
        }
        if dow == "1-5" {
            return format!("weekdays at {at}");
        }
        let names: Option<Vec<String>> = dow.split(',').map(day_name).collect();
        if let Some(n) = names {
            return format!("{} at {at}", n.join(", "));
        }
    }
    if mon == "*" && dow == "*" {
        if let Some(d) = num(dom) {
            return format!("monthly on day {d} at {at}");
        }
    }
    schedule.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-11-15 (a Sunday) 10:07:30 UTC.
    const NOW: i64 = 1_794_737_250;

    fn at(s: &str) -> i64 {
        CronSpec::parse(s).unwrap().next_after(NOW).unwrap()
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(NOW / 86400), (2026, 11, 15));
        assert_eq!((NOW / 86400 + 4).rem_euclid(7), 0, "a Sunday");
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn next_runs() {
        let day = NOW / 86400 * 86400;
        assert_eq!(at("*/5 * * * *"), day + 10 * 3600 + 10 * 60);
        assert_eq!(at("* * * * *"), day + 10 * 3600 + 8 * 60);
        assert_eq!(at("30 3 * * *"), day + 86400 + 3 * 3600 + 30 * 60);
        assert_eq!(at("0 12 * * *"), day + 12 * 3600);
        // Next Monday.
        assert_eq!(at("0 9 * * mon"), day + 86400 + 9 * 3600);
        assert_eq!(at("0 9 * * 7"), day + 7 * 86400 + 9 * 3600, "7 is Sunday; today's 09:00 has passed");
        // 1 Dec.
        assert_eq!(at("@monthly"), day + 16 * 86400);
        // Either the 20th or a Friday, whichever comes first (Fri 20 Nov).
        assert_eq!(at("0 0 20 * 5"), day + 5 * 86400);
        assert_eq!(at("0 0 1 jan *"), day + 47 * 86400);
        // 29 Feb 2028.
        assert!(CronSpec::parse("0 0 29 2 *").unwrap().next_after(NOW).is_some());
        assert_eq!(CronSpec::parse("0 0 31 2 *").unwrap().next_after(NOW), None);
        assert_eq!(CronSpec::parse("@reboot"), None);
        assert_eq!(CronSpec::parse("61 * * * *"), None);
        assert_eq!(CronSpec::parse("* * *"), None);
        assert_eq!(at("5/20 * * * *"), day + 10 * 3600 + 25 * 60);
    }

    #[test]
    fn descriptions() {
        assert_eq!(describe("*/5 * * * *"), "every 5 minutes");
        assert_eq!(describe("17 * * * *"), "hourly at :17");
        assert_eq!(describe("0 */6 * * *"), "every 6 hours at :00");
        assert_eq!(describe("30 3 * * *"), "daily at 03:30");
        assert_eq!(describe("0 9 * * 1-5"), "weekdays at 09:00");
        assert_eq!(describe("0 9 * * mon,thu"), "Mon, Thu at 09:00");
        assert_eq!(describe("0 4 1 * *"), "monthly on day 1 at 04:00");
        assert_eq!(describe("@reboot"), "at boot");
        assert_eq!(describe("0 4 1-7 * 0"), "0 4 1-7 * 0");
    }

    #[test]
    fn cron_lines() {
        assert_eq!(parse_cron_line("# comment", "crontab", false), None);
        assert_eq!(parse_cron_line("MAILTO=root", "crontab", false), None);
        assert_eq!(parse_cron_line("PATH = /usr/bin:/bin", "crontab", false), None);
        let j = parse_cron_line("*/10 * * * * /usr/local/bin/backup.sh --quiet > /dev/null 2>&1", "crontab", false)
            .unwrap();
        assert_eq!(
            (j.schedule.as_str(), j.command.as_str(), j.user.as_deref()),
            ("*/10 * * * *", "/usr/local/bin/backup.sh --quiet > /dev/null 2>&1", None)
        );
        let j =
            parse_cron_line("17 *\t* * *\troot    cd / && run-parts --report /etc/cron.hourly", "/etc/crontab", true)
                .unwrap();
        assert_eq!(
            (j.user.as_deref(), j.command.as_str(), j.every.as_str()),
            (Some("root"), "cd / && run-parts --report /etc/cron.hourly", "hourly at :17")
        );
        let j = parse_cron_line("@reboot sleep 30 && /opt/start.sh", "crontab", false).unwrap();
        assert_eq!((j.schedule.as_str(), j.command.as_str()), ("@reboot", "sleep 30 && /opt/start.sh"));
        // A job whose command has `=` in it is still a job.
        assert!(parse_cron_line("0 1 * * * FOO=1 /bin/x", "crontab", false).is_some());
        assert_eq!(parse_cron_line("0 1 * * *", "crontab", false), None);
    }

    #[test]
    fn parses_a_scan() {
        let out = "@@now\n1794737250\n+0100\n@@timers\n\
            Id=apt-daily.timer\nDescription=Daily apt download activities\nActiveState=active\nUnitFileState=enabled\nUnit=apt-daily.service\n\
            TimersCalendar={ OnCalendar=*-*-* 06,18:00:00 ; next_elapse=@1794765600 }\nTimersMonotonic=\nNextElapseUSecRealtime=@1794765600\nLastTriggerUSec=@1794722400\n\n\
            Id=fstrim.timer\nDescription=Discard unused blocks once a week\nActiveState=active\nUnitFileState=enabled\nUnit=fstrim.service\n\
            TimersCalendar={ OnCalendar=weekly ; next_elapse=@1794956400 }\nNextElapseUSecRealtime=@1794956400\nLastTriggerUSec=\n\n\
            Id=backup.timer\nDescription=Backup\nActiveState=active\nUnitFileState=enabled\nUnit=backup.service\n\
            TimersMonotonic={ OnUnitActiveSec=1d ; next_elapse=0 }\nNextElapseUSecRealtime=\nLastTriggerUSec=@1794700000\n\n\
            @@services\nId=apt-daily.service\nResult=success\nExecMainStatus=0\n\nId=backup.service\nResult=exit-code\nExecMainStatus=2\n\n\
            @@crontab\nMAILTO=me\n0 3 * * * /home/me/bin/sync.sh\n\
            @@etc\nSHELL=/bin/sh\n17 * * * * root cd / && run-parts --report /etc/cron.hourly\n\
            @@file /etc/cron.d/certbot\n0 */12 * * * root certbot -q renew\n\
            @@periodic\ndaily|logrotate\ndaily|man-db\nweekly|man-db\n@@end\n";
        let s = parse_scan(out);
        assert!(s.systemd);
        assert_eq!(s.utc_offset_min, 60);
        assert_eq!(
            s.timers.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["apt-daily.timer", "fstrim.timer", "backup.timer"]
        );
        let apt = &s.timers[0];
        assert_eq!(apt.calendar, ["*-*-* 06,18:00:00"]);
        assert_eq!(
            (apt.next, apt.last, apt.result.as_deref(), apt.exit_status),
            (Some(1794765600.0), Some(1794722400.0), Some("success"), Some(0))
        );
        let b = &s.timers[2];
        assert_eq!(
            (b.monotonic.clone(), b.next, b.result.as_deref(), b.exit_status),
            (vec!["OnUnitActiveSec=1d".to_owned()], None, Some("exit-code"), Some(2))
        );
        assert_eq!(s.cron.len(), 3);
        // 10:07 UTC is 11:07 local; the hourly :17 job runs at 11:17 local = 10:17 UTC.
        let hourly = s.cron.iter().find(|j| j.source == "/etc/crontab").unwrap();
        assert_eq!(hourly.next, Some((NOW / 60 * 60 + 10 * 60) as f64));
        // 03:00 local tomorrow = 02:00 UTC.
        let sync = s.cron.iter().find(|j| j.source == "crontab").unwrap();
        assert_eq!(sync.next, Some((NOW / 86400 * 86400 + 86400 + 2 * 3600) as f64));
        assert_eq!(s.cron.iter().find(|j| j.source == "/etc/cron.d/certbot").unwrap().every, "every 12 hours at :00");
        assert_eq!(
            s.periodic,
            vec![
                Periodic { period: "daily".into(), scripts: vec!["logrotate".into(), "man-db".into()] },
                Periodic { period: "weekly".into(), scripts: vec!["man-db".into()] },
            ]
        );
        assert!(!parse_scan("@@now\n1\n+0000\n@@timers\nnosystemd\n@@end\n").systemd);
    }

    #[test]
    fn offsets() {
        assert_eq!(parse_offset("+0130"), Some(90));
        assert_eq!(parse_offset("-0500"), Some(-300));
        assert_eq!(parse_offset("UTC"), None);
    }
}
#[cfg(test)]
mod script_tests {
    #[test]
    fn script_runs_under_sh() {
        let out = std::process::Command::new("sh").arg("-c").arg(super::scan_command()).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(text.contains("@@end"), "{text}");
        let s = super::parse_scan(&text);
        assert!(s.now > 1.0e9);
    }
}
