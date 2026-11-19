//! The Windows Event Log for System → JOURNAL: the System and Application
//! logs (and Security, for administrators, when "read all" is on), with the
//! same filters as journald: source, severity, time range, backlog size and
//! follow. Events become [`JournalEntry`] rows: Windows levels map to
//! syslog priorities (Critical 2, Error 3, Warning 4, Information 6,
//! Verbose 7), `ident` is the event source and `unit` is the log.

use crate::system::journal::{JournalEntry, JournalQuery, MAX_LINES};
use crate::system::{SystemError, SystemResult};

use super::quote;

/// Windows levels (1 Critical … 5 Verbose, 0 = always) shown at `priority`.
fn levels(priority: Option<u8>) -> Option<&'static str> {
    match priority {
        None | Some(7..) => None,
        Some(5..=6) => Some("0,1,2,3,4"),
        Some(4) => Some("1,2,3"),
        Some(3) => Some("1,2"),
        Some(_) => Some("1"),
    }
}

/// A journalctl-style time (`-1h`, `-30m`, `-2d`, `today`, `yesterday`,
/// `now`, `2026-11-14 08:00`) as a PowerShell `[datetime]` expression, read
/// on the host's clock like journalctl does.
pub fn ps_time(spec: &str) -> SystemResult<String> {
    let t = spec.trim();
    let bad = || SystemError::other(format!("can't use {t:?} as a time (try -1h, today or 2026-11-14 08:00)"));
    match t {
        "now" => return Ok("(Get-Date)".into()),
        "today" => return Ok("(Get-Date).Date".into()),
        "yesterday" => return Ok("(Get-Date).Date.AddDays(-1)".into()),
        _ => {}
    }
    if let Some(rel) = t.strip_prefix('-') {
        let split = rel.find(|c: char| !c.is_ascii_digit()).ok_or_else(bad)?;
        let (n, unit) = rel.split_at(split);
        let n: u32 = n.parse().map_err(|_| bad())?;
        let method = match unit.trim() {
            "s" | "sec" | "second" | "seconds" => "AddSeconds",
            "m" | "min" | "minute" | "minutes" => "AddMinutes",
            "h" | "hour" | "hours" => "AddHours",
            "d" | "day" | "days" => "AddDays",
            "w" | "week" | "weeks" => return Ok(format!("(Get-Date).AddDays(-{})", n * 7)),
            _ => return Err(bad()),
        };
        return Ok(format!("(Get-Date).{method}(-{n})"));
    }
    // An absolute date (and time): digits, '-', ':', ' ' only.
    let ok = t.len() >= 10
        && t.len() <= 19
        && t.chars().all(|c| c.is_ascii_digit() || "-: ".contains(c))
        && t.as_bytes()[4] == b'-'
        && t.as_bytes()[7] == b'-';
    if !ok {
        return Err(bad());
    }
    Ok(format!(
        "[datetime]::ParseExact({}, {}, [Globalization.CultureInfo]::InvariantCulture)",
        quote(t),
        quote(fmt_of(t))
    ))
}

fn fmt_of(t: &str) -> &'static str {
    match t.len() {
        10 => "yyyy-MM-dd",
        16 => "yyyy-MM-dd HH:mm",
        _ => "yyyy-MM-dd HH:mm:ss",
    }
}

/// Whether `s` can be an event source name (no wildcards or quotes).
pub fn valid_source(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.chars().any(|c| c.is_control() || "*?'\"`[]".contains(c))
}

/// The PowerShell that prints matching events oldest first, one per line
/// (`ms \t priority \t log \t source \t pid \t id: message`), and keeps
/// polling for new ones when following. `all` adds the Security log.
pub fn script(q: &JournalQuery, all: bool) -> SystemResult<String> {
    let lines = q.lines.clamp(1, MAX_LINES);
    let mut filter = vec![];
    if let Some(u) = q.unit.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !valid_source(u) {
            return Err(SystemError::other(format!("invalid event source: {u:?}")));
        }
        filter.push(format!("ProviderName = {}", quote(u)));
    }
    if let Some(l) = levels(q.priority) {
        filter.push(format!("Level = @({l})"));
    }
    if q.priority.is_some_and(|p| p > 7) {
        return Err(SystemError::other("priority must be 0–7"));
    }
    let since = match q.since.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => format!("$since = {}", ps_time(s)?),
        None => "$since = $null".into(),
    };
    let until = match q.until.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            if q.follow {
                return Err(SystemError::other("follow can't be combined with an end time"));
            }
            format!("$until = {}", ps_time(s)?)
        }
        None => "$until = $null".into(),
    };
    let logs = if all { "'System','Application','Security'" } else { "'System','Application'" };
    Ok(format!(
        r#"$ErrorActionPreference = 'Continue'
$logs = @({logs})
{since}
{until}
$lines = {lines}
$follow = ${follow}
$base = @{{ {filter} }}
function Prio($l) {{ switch ([int]$l) {{ 1 {{ 2 }} 2 {{ 3 }} 3 {{ 4 }} 5 {{ 7 }} default {{ 6 }} }} }}
function Line($e) {{
  $m = $e.Message
  if (-not $m) {{ $m = '(no description for this event)' }}
  $m = ($m -replace '\s*\r?\n\s*', ' | ').Trim(' ', '|')
  $ms = [int64](($e.TimeCreated.ToUniversalTime() - [datetime]'1970-01-01').TotalMilliseconds)
  [Console]::Out.WriteLine(("{{0}}`t{{1}}`t{{2}}`t{{3}}`t{{4}}`t{{5}}: {{6}}" -f $ms, (Prio $e.Level), $e.LogName, $e.ProviderName, $e.ProcessId, $e.Id, $m))
}}
function Read($log, $start, $max) {{
  $f = $base.Clone(); $f.LogName = $log
  if ($start) {{ $f.StartTime = $start }}
  if ($until) {{ $f.EndTime = $until }}
  try {{
    if ($max) {{ Get-WinEvent -FilterHashtable $f -MaxEvents $max -ErrorAction Stop }} else {{ Get-WinEvent -FilterHashtable $f -ErrorAction Stop }}
  }} catch {{
    if ($_.FullyQualifiedErrorId -notmatch 'NoMatchingEventsFound') {{
      [Console]::Error.WriteLine($log + ': ' + $_.Exception.Message)
    }}
  }}
}}
$last = @{{}}
$all = foreach ($l in $logs) {{
  $last[$l] = 0
  Read $l $since $lines
}}
$all = @($all | Sort-Object TimeCreated, RecordId | Select-Object -Last $lines)
foreach ($e in $all) {{ Line $e; if ($e.RecordId -gt $last[$e.LogName]) {{ $last[$e.LogName] = $e.RecordId }} }}
[Console]::Out.Flush()
if (-not $all.Count) {{ [Console]::Error.WriteLine('No events match these filters.') }}
$from = if ($all.Count) {{ $all[-1].TimeCreated.AddSeconds(-2) }} else {{ (Get-Date).AddSeconds(-2) }}
while ($follow) {{
  Start-Sleep -Seconds 2
  $new = foreach ($l in $logs) {{ Read $l $from 0 | Where-Object {{ $_.RecordId -gt $last[$l] }} }}
  foreach ($e in @($new | Sort-Object TimeCreated, RecordId)) {{
    Line $e
    if ($e.RecordId -gt $last[$e.LogName]) {{ $last[$e.LogName] = $e.RecordId }}
    $from = $e.TimeCreated.AddSeconds(-2)
  }}
  [Console]::Out.Flush()
}}"#,
        follow = if q.follow { "true" } else { "false" },
        filter = filter.join("; "),
    ))
}

/// Parses one output line of [`script`].
pub fn parse_line(line: &str) -> Option<JournalEntry> {
    let mut f = line.splitn(6, '\t');
    let ts: u64 = f.next()?.trim().parse().ok()?;
    let priority: u8 = f.next()?.parse().ok()?;
    let log = f.next()?;
    let source = f.next()?;
    let pid = f.next()?.parse().ok().filter(|p| *p != 0);
    let message = f.next()?.to_owned();
    Some(JournalEntry {
        ts,
        priority,
        unit: Some(log.to_owned()).filter(|s| !s.is_empty()),
        ident: Some(source.to_owned()).filter(|s| !s.is_empty()),
        pid,
        message,
    })
}

/// The "no events" note isn't an error.
pub fn is_empty_notice(line: &str) -> bool {
    line.starts_with("No events match")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q() -> JournalQuery {
        JournalQuery { unit: None, priority: None, since: None, until: None, lines: 100, follow: false }
    }

    #[test]
    fn times() {
        assert_eq!(ps_time("-1h").unwrap(), "(Get-Date).AddHours(-1)");
        assert_eq!(ps_time("-2w").unwrap(), "(Get-Date).AddDays(-14)");
        assert_eq!(ps_time("today").unwrap(), "(Get-Date).Date");
        assert!(ps_time("2026-11-14 08:00").unwrap().contains("'yyyy-MM-dd HH:mm'"));
        for bad in ["", "-1y", "soon", "2026/11/14", "-h", "2026-11-14'; x"] {
            assert!(ps_time(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn filters() {
        let s = script(&JournalQuery { unit: Some("Service Control Manager".into()), priority: Some(4), ..q() }, false)
            .unwrap();
        assert!(s.contains("ProviderName = 'Service Control Manager'; Level = @(1,2,3)"), "{s}");
        assert!(s.contains("$follow = $false"));
        assert!(!s.contains("Security"));
        assert!(script(&q(), true).unwrap().contains("'Security'"));
        assert!(script(&JournalQuery { unit: Some("a*".into()), ..q() }, false).is_err());
        assert!(script(&JournalQuery { until: Some("today".into()), follow: true, ..q() }, false).is_err());
    }

    #[test]
    fn lines() {
        let e = parse_line(
            "1763500000000\t3\tSystem\tService Control Manager\t712\t7031: The X service terminated unexpectedly.",
        )
        .unwrap();
        assert_eq!((e.ts, e.priority, e.pid), (1_763_500_000_000, 3, Some(712)));
        assert_eq!(e.unit.as_deref(), Some("System"));
        assert_eq!(e.ident.as_deref(), Some("Service Control Manager"));
        assert!(e.message.starts_with("7031: "));
        assert_eq!(parse_line("1\t6\tApplication\tx\t0\t1: m").unwrap().pid, None);
        assert!(parse_line("garbage").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn reads_the_local_log() {
        let o = super::super::run_local(&script(&JournalQuery { lines: 20, ..q() }, false).unwrap(), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let rows: Vec<_> = o.stdout.lines().filter_map(parse_line).collect();
        assert!(!rows.is_empty(), "stdout: {}\nstderr: {}", o.stdout, o.stderr);
        assert!(rows.len() <= 20);
        assert!(rows.windows(2).all(|w| w[0].ts <= w[1].ts));
    }
}
