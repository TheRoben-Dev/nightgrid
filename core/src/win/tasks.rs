//! Windows Task Scheduler for System → SCHEDULES: your own tasks (not the
//! hundreds Windows ships under `\Microsoft\`), with their triggers, next
//! and last runs and last result, shown as timer rows. Tasks can be run
//! now, enabled or disabled (system tasks need an administrator).

use serde::Deserialize;

use crate::ssh::client::Connection;
use crate::system::schedule::{Schedules, Timer};
use crate::system::{SystemError, SystemResult};

use super::{quote, run, IS_ADMIN};

/// One JSON document: the host clock and the tasks.
const LIST: &str = r#"$ErrorActionPreference = 'Stop'
function Unix($d) { if (-not $d -or $d.Year -lt 2000) { return $null }; [int64](($d.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds) }
function Trig($t) {
  $c = $t.CimClass.CimClassName
  $at = ''
  if ($t.StartBoundary) { try { $at = ([datetime]$t.StartBoundary).ToString('HH:mm') } catch { } }
  $w = switch ($c) {
    'MSFT_TaskDailyTrigger' { if ($t.DaysInterval -gt 1) { "every $($t.DaysInterval) days at $at" } else { "daily at $at" } }
    'MSFT_TaskWeeklyTrigger' {
      $names = 'Sun','Mon','Tue','Wed','Thu','Fri','Sat'
      $days = @(); for ($i = 0; $i -lt 7; $i++) { if ($t.DaysOfWeek -band (1 -shl $i)) { $days += $names[$i] } }
      "weekly on $($days -join ',') at $at"
    }
    'MSFT_TaskTimeTrigger' { "once at " + $(if ($t.StartBoundary) { ([datetime]$t.StartBoundary).ToString('yyyy-MM-dd HH:mm') } else { '?' }) }
    'MSFT_TaskBootTrigger' { 'at boot' }
    'MSFT_TaskLogonTrigger' { 'at logon' }
    'MSFT_TaskIdleTrigger' { 'when idle' }
    'MSFT_TaskEventTrigger' { 'on an event' }
    'MSFT_TaskSessionStateChangeTrigger' { 'on session change' }
    'MSFT_TaskRegistrationTrigger' { 'when registered' }
    default { ($c -replace '^MSFT_Task', '' -replace 'Trigger$', '').ToLower() }
  }
  if ($t.Repetition -and $t.Repetition.Interval) { $w += ', repeating ' + $t.Repetition.Interval }
  if (-not $t.Enabled) { $w += ' (off)' }
  $w
}
$tasks = @(Get-ScheduledTask | Where-Object { $_.TaskPath -notlike '\Microsoft\*' } | ForEach-Object {
  $i = $null; try { $i = $_ | Get-ScheduledTaskInfo -ErrorAction Stop } catch { }
  $acts = @($_.Actions | ForEach-Object { if ($_.Execute) { (($_.Execute + ' ' + $_.Arguments).Trim()) } else { $_.CimClass.CimClassName } })
  [ordered]@{
    path = $_.TaskPath + $_.TaskName
    description = [string]$_.Description
    state = [string]$_.State
    triggers = @($_.Triggers | Where-Object { $_ } | ForEach-Object { Trig $_ })
    action = ($acts -join '; ')
    user = [string]$_.Principal.UserId
    next = $(if ($i) { Unix $i.NextRunTime } else { $null })
    last = $(if ($i) { Unix $i.LastRunTime } else { $null })
    result = $(if ($i) { [int64]$i.LastTaskResult } else { $null })
  }
})
$now = Get-Date
@{ now = (Unix $now); offset = [int][TimeZoneInfo]::Local.GetUtcOffset($now).TotalMinutes; tasks = $tasks } | ConvertTo-Json -Depth 4 -Compress"#;

#[derive(Deserialize)]
struct Doc {
    now: f64,
    offset: i32,
    #[serde(default, deserialize_with = "one_or_many")]
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
struct Task {
    path: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default, deserialize_with = "one_or_many")]
    triggers: Vec<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    user: Option<String>,
    next: Option<f64>,
    last: Option<f64>,
    result: Option<i64>,
}

/// PowerShell 5.1 writes a one-item array as the item itself.
fn one_or_many<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum M<T> {
        Many(Vec<T>),
        One(T),
        None(()),
    }
    Ok(match Option::<M<T>>::deserialize(d)? {
        Some(M::Many(v)) => v,
        Some(M::One(t)) => vec![t],
        _ => Vec::new(),
    })
}

/// Task Scheduler's "has not run yet" and "is running" results.
const NOT_RUN: i64 = 0x41303;
const RUNNING: i64 = 0x41301;

fn to_timer(t: Task) -> Timer {
    let (active, enabled) = match t.state.as_str() {
        "Disabled" => ("inactive", "disabled"),
        _ => ("active", "enabled"),
    };
    let (result, exit_status) = match t.result {
        None | Some(NOT_RUN) | Some(RUNNING) => (None, None),
        Some(0) => (Some("success".to_owned()), None),
        Some(c) => (Some("exit-code".to_owned()), Some(c as i32)),
    };
    let mut description = t.description.unwrap_or_default().trim().to_owned();
    let extra: Vec<String> = [
        t.action.filter(|a| !a.is_empty()).map(|a| format!("runs {a}")),
        t.user.filter(|u| !u.is_empty()).map(|u| format!("as {u}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !extra.is_empty() {
        if !description.is_empty() {
            description.push_str(" · ");
        }
        description.push_str(&extra.join(" "));
    }
    Timer {
        name: t.path,
        description,
        active: if t.state == "Running" { "active".into() } else { active.into() },
        enabled: Some(enabled.into()),
        unit: String::new(),
        calendar: t.triggers,
        monotonic: Vec::new(),
        next: t.next,
        last: t.last,
        result,
        exit_status,
    }
}

/// Parses [`LIST`] output.
pub fn parse(json: &str) -> SystemResult<Schedules> {
    let d: Doc = serde_json::from_str(json.trim())
        .map_err(|e| SystemError::other(format!("couldn't read the task list: {e}")))?;
    Ok(Schedules {
        systemd: false,
        timers: d.tasks.into_iter().map(to_timer).collect(),
        cron: Vec::new(),
        periodic: Vec::new(),
        now: d.now,
        utc_offset_min: d.offset,
    })
}

pub async fn scan(conn: &Connection) -> SystemResult<Schedules> {
    let o = run(conn, LIST).await.map_err(SystemError::other)?;
    if o.code != 0 {
        let t = o.error_text();
        return Err(SystemError::other(if t.is_empty() {
            format!("reading Task Scheduler failed (exit {})", o.code)
        } else {
            t
        }));
    }
    parse(&o.stdout)
}

/// What can be done to a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TaskAction {
    Run,
    Enable,
    Disable,
}

/// Whether `path` can be a task path (`\Folder\Name`).
pub fn valid_path(path: &str) -> bool {
    path.starts_with('\\')
        && path.len() > 1
        && path.len() <= 512
        && !path.ends_with('\\')
        && !path.chars().any(|c| c.is_control() || "*?[]'\"`".contains(c))
}

pub fn action_script(path: &str, action: TaskAction) -> SystemResult<String> {
    if !valid_path(path) {
        return Err(SystemError::other(format!("invalid task: {path:?}")));
    }
    let i = path.rfind('\\').unwrap_or(0);
    let (folder, name) = (&path[..=i], &path[i + 1..]);
    let verb = match action {
        TaskAction::Run => "Start-ScheduledTask",
        TaskAction::Enable => "Enable-ScheduledTask",
        TaskAction::Disable => "Disable-ScheduledTask",
    };
    Ok(format!(
        "$ErrorActionPreference = 'Stop'\ntry {{ {verb} -TaskPath {} -TaskName {} | Out-Null }} catch {{\n  if (-not ({IS_ADMIN})) {{ [Console]::Error.WriteLine('Windows refused: this task needs an administrator account. ' + $_.Exception.Message) }} else {{ [Console]::Error.WriteLine($_.Exception.Message) }}\n  exit 1\n}}",
        quote(folder),
        quote(name)
    ))
}

pub async fn act(conn: &Connection, path: &str, action: TaskAction) -> SystemResult<()> {
    let o = run(conn, &action_script(path, action)?).await.map_err(SystemError::other)?;
    if o.code == 0 {
        Ok(())
    } else {
        let t = o.error_text();
        Err(SystemError::other(if t.is_empty() { format!("the task action failed (exit {})", o.code) } else { t }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tasks() {
        let s = parse(
            r#"{"now":1763500000,"offset":60,"tasks":[
              {"path":"\\Backups\\Nightly","description":"Copies D:","state":"Ready","triggers":"daily at 03:00","action":"robocopy.exe D:\\ E:\\","user":"SYSTEM","next":1763521200,"last":1763434800,"result":0},
              {"path":"\\Broken","description":null,"state":"Disabled","triggers":["at boot","weekly on Mon at 04:00"],"action":"","user":"","next":null,"last":1763434800,"result":2147942402},
              {"path":"\\New","state":"Ready","triggers":null,"next":null,"last":null,"result":267011}
            ]}"#,
        )
        .unwrap();
        assert!(!s.systemd);
        assert_eq!(s.utc_offset_min, 60);
        let [a, b, c] = &s.timers[..] else { panic!("{:?}", s.timers) };
        assert_eq!(a.name, "\\Backups\\Nightly");
        assert_eq!(a.calendar, vec!["daily at 03:00"]);
        assert_eq!(a.result.as_deref(), Some("success"));
        assert!(
            a.description.contains("runs robocopy.exe") && a.description.contains("as SYSTEM"),
            "{}",
            a.description
        );
        assert_eq!(
            (b.active.as_str(), b.enabled.as_deref(), b.result.as_deref()),
            ("inactive", Some("disabled"), Some("exit-code"))
        );
        assert_eq!(b.calendar.len(), 2);
        assert_eq!((c.result.as_deref(), c.calendar.len()), (None, 0));
        // One task comes as an object, none as null.
        assert_eq!(
            parse(r#"{"now":1,"offset":0,"tasks":{"path":"\\A","state":"Ready","next":null,"last":null,"result":0}}"#)
                .unwrap()
                .timers
                .len(),
            1
        );
        assert_eq!(parse(r#"{"now":1,"offset":0,"tasks":null}"#).unwrap().timers.len(), 0);
    }

    #[test]
    fn actions() {
        let s = action_script("\\Backups\\Nightly", TaskAction::Run).unwrap();
        assert!(s.contains("Start-ScheduledTask -TaskPath '\\Backups\\' -TaskName 'Nightly'"), "{s}");
        assert!(action_script("\\Top", TaskAction::Disable).unwrap().contains("-TaskPath '\\' -TaskName 'Top'"));
        for bad in ["", "\\", "Nightly", "\\a*", "\\a'b", "\\dir\\"] {
            assert!(action_script(bad, TaskAction::Run).is_err(), "{bad:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn lists_local_tasks() {
        let o = super::super::run_local(LIST, b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let s = parse(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
        assert!(s.now > 1_700_000_000.0);
    }
}
