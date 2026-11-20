//! Fleet → BACKUPS on Windows hosts. A "unit" job is a Task Scheduler task
//! (`\Backups\Nightly`, or Windows Server Backup's own
//! `\Microsoft\Windows\Backup\Microsoft-Windows-WindowsBackup`): its last
//! run time and result come from `Get-ScheduledTaskInfo`. A path job reads
//! the newest file and size under a folder (`D:\Backups`, `\\nas\share`).
//! Detection lists tasks that run restic, borg, rclone, robocopy, wbadmin …

use serde::Deserialize;

use crate::backup_jobs::{
    mark_stale, BackupJob, BackupResult, BackupSource, BackupStatus, BackupSuggestion, BackupTool,
};

use super::quote;

/// Task results that mean "still running" or "never ran".
const RUNNING: i64 = 0x41301;
const NOT_YET: i64 = 0x41303;

/// One script for every job of a host; prints one JSON object.
pub fn status_script(jobs: &[BackupJob]) -> String {
    let mut s = String::from(
        r#"$ErrorActionPreference = 'SilentlyContinue'
function U($d) { if ($d -and $d.Year -gt 2000) { [int64](($d.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds) } else { $null } }
$os = Get-CimInstance Win32_OperatingSystem
$r = @{ now = (U (Get-Date)); uptime = [int64]((Get-Date) - $os.LastBootUpTime).TotalSeconds; jobs = @() }
"#,
    );
    for j in jobs {
        let t = quote(&j.target);
        match j.source {
            BackupSource::Unit => s.push_str(&format!(
                r#"$p = {t}; $i = $p.LastIndexOf('\'); $tp = $p.Substring(0, $i + 1); $tn = $p.Substring($i + 1)
$k = Get-ScheduledTask -TaskPath $tp -TaskName $tn
if ($k) {{ $n = $k | Get-ScheduledTaskInfo; $r.jobs += @{{ id = {id}; found = $true; state = [string]$k.State; last = (U $n.LastRunTime); result = [int64]$n.LastTaskResult }} }}
else {{ $r.jobs += @{{ id = {id}; found = $false }} }}
"#,
                id = j.id
            )),
            BackupSource::Path => s.push_str(&format!(
                r#"$p = {t}
if (Test-Path -LiteralPath $p) {{
  $it = Get-Item -LiteralPath $p -Force
  if ($it.PSIsContainer) {{
    $sw = [Diagnostics.Stopwatch]::StartNew(); $size = [int64]0; $newest = $null; $cut = $false
    foreach ($f in (Get-ChildItem -LiteralPath $p -File -Recurse -Force)) {{
      $size += $f.Length
      if (-not $newest -or $f.LastWriteTimeUtc -gt $newest) {{ $newest = $f.LastWriteTimeUtc }}
      if ($sw.Elapsed.TotalSeconds -gt 20) {{ $cut = $true; break }}
    }}
    $r.jobs += @{{ id = {id}; found = $true; dir = $true; last = (U $newest); size = $(if ($cut) {{ $null }} else {{ $size }}) }}
  }} else {{ $r.jobs += @{{ id = {id}; found = $true; dir = $false; last = (U $it.LastWriteTime); size = [int64]$it.Length }} }}
}} elseif (Test-Path -LiteralPath (Split-Path $p)) {{ $r.jobs += @{{ id = {id}; found = $false }} }}
else {{ $r.jobs += @{{ id = {id}; found = $false; denied = $true }} }}
"#,
                id = j.id
            )),
        }
    }
    s.push_str("[Console]::Out.Write((ConvertTo-Json -InputObject $r -Depth 4 -Compress))\n");
    s
}

#[derive(Deserialize)]
struct Raw {
    now: Option<f64>,
    uptime: Option<f64>,
    #[serde(default, deserialize_with = "super::tasks::one_or_many")]
    jobs: Vec<RawJob>,
}

#[derive(Deserialize)]
struct RawJob {
    id: i64,
    #[serde(default)]
    found: bool,
    #[serde(default)]
    denied: bool,
    #[serde(default)]
    dir: bool,
    state: Option<String>,
    last: Option<f64>,
    result: Option<i64>,
    size: Option<u64>,
}

/// Statuses from [`status_script`]'s output; `now` is used when the host's
/// clock is missing.
pub fn parse_status(out: &str, jobs: &[BackupJob], now: f64) -> Result<Vec<BackupStatus>, String> {
    let start = out.find('{').ok_or("the host gave no answer")?;
    let raw: Raw = serde_json::from_str(&out[start..]).map_err(|e| format!("unreadable answer: {e}"))?;
    let host_now = raw.now.unwrap_or(now);
    Ok(jobs
        .iter()
        .map(|j| {
            let mut st = BackupStatus::empty(j.id);
            match raw.jobs.iter().find(|r| r.id == i64::from(j.id)) {
                None => st.error = Some("No answer from the host for this job.".into()),
                Some(r) if !r.found => {
                    st.error = Some(match (j.source, r.denied) {
                        (BackupSource::Unit, _) => "The scheduled task wasn't found on this host.".into(),
                        (_, true) => "The path can't be read by this login.".into(),
                        _ => "The path doesn't exist on this host.".into(),
                    })
                }
                Some(r) if j.source == BackupSource::Path => {
                    st.last_run = r.last;
                    st.last_ok = r.last;
                    st.size = r.size;
                    st.result = if r.last.is_some() { BackupResult::Ok } else { BackupResult::Unknown };
                    if r.dir && r.size.is_none() {
                        st.summary = Some("Size not read (took too long)".into());
                    }
                }
                Some(r) => {
                    let code = r.result.unwrap_or(NOT_YET);
                    st.last_run = r.last;
                    if r.state.as_deref() == Some("Running") || code == RUNNING {
                        st.result = BackupResult::Running;
                        st.summary = Some("Running now".into());
                    } else if r.last.is_none() || code == NOT_YET {
                        st.summary = Some("The task hasn't run yet".into());
                        st.last_run = None;
                    } else if code == 0 {
                        st.result = BackupResult::Ok;
                        st.last_ok = r.last;
                    } else {
                        st.result = BackupResult::Failed;
                        st.exit_code = i32::try_from(code).ok();
                        st.summary = Some(format!("Last result 0x{code:X}"));
                    }
                    if r.state.as_deref() == Some("Disabled") {
                        st.summary =
                            Some(format!("{} (task disabled)", st.summary.as_deref().unwrap_or("Not running")));
                    }
                }
            }
            mark_stale(&mut st, j, host_now, raw.uptime);
            st
        })
        .collect())
}

/// Lists scheduled tasks whose actions run a backup tool, one per line:
/// `T \t path \t command`.
pub const DETECT: &str = r#"$ErrorActionPreference = 'SilentlyContinue'
foreach ($t in @(Get-ScheduledTask)) {
  if ($t.TaskPath -like '\Microsoft\*' -and $t.TaskName -ne 'Microsoft-Windows-WindowsBackup') { continue }
  $cmd = (@($t.Actions | Where-Object { $_.Execute } | ForEach-Object { $_.Execute + ' ' + $_.Arguments }) -join ' ; ')
  if ($t.TaskName -eq 'Microsoft-Windows-WindowsBackup') { $cmd = 'wbadmin ' + $cmd }
  $cmd = ($cmd -replace '\s+', ' ')
  if ($cmd.Length -gt 600) { $cmd = $cmd.Substring(0, 600) }
  [Console]::Out.WriteLine("T`t" + $t.TaskPath + $t.TaskName + "`t" + $cmd)
}
"#;

/// Suggestions from [`DETECT`]'s output.
pub fn parse_detect(out: &str) -> Vec<BackupSuggestion> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.splitn(3, '\t');
            if f.next()? != "T" {
                return None;
            }
            let path = f.next()?.trim();
            let cmd = f.next().unwrap_or("").trim();
            // `C:\restic\restic.exe backup …` → words the POSIX detector knows.
            let words = cmd.replace('\\', "/").to_lowercase().replace(".exe", "");
            let tool = BackupTool::detect(&words)?;
            if !crate::backup_jobs::valid_unit(path) {
                return None;
            }
            let name = if path.ends_with("Microsoft-Windows-WindowsBackup") {
                "Windows Server Backup".to_owned()
            } else {
                path.rsplit('\\').next().unwrap_or(path).to_owned()
            };
            let detail = format!("scheduled task {path}: {cmd}");
            Some(BackupSuggestion {
                name,
                tool,
                source: BackupSource::Unit,
                target: path.to_owned(),
                detail: crate::backup_jobs::masked(&detail),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: i32, source: BackupSource, target: &str) -> BackupJob {
        BackupJob {
            id,
            host_id: 1,
            name: "j".into(),
            tool: BackupTool::Restic,
            source,
            target: target.into(),
            stale_hours: 24,
        }
    }

    #[test]
    fn statuses() {
        let jobs = vec![
            job(1, BackupSource::Unit, "\\Backups\\Nightly"),
            job(2, BackupSource::Unit, "\\Backups\\Broken"),
            job(3, BackupSource::Path, "D:\\Backups"),
            job(4, BackupSource::Unit, "\\Gone"),
            job(5, BackupSource::Unit, "\\Backups\\Old"),
        ];
        let out = r#"{"now":1000000,"uptime":5000,"jobs":[
            {"id":1,"found":true,"state":"Ready","last":999000,"result":0},
            {"id":2,"found":true,"state":"Ready","last":999500,"result":2147942402},
            {"id":3,"found":true,"dir":true,"last":990000,"size":4096},
            {"id":4,"found":false},
            {"id":5,"found":true,"state":"Ready","last":800000,"result":0}]}"#;
        let st = parse_status(out, &jobs, 0.0).unwrap();
        assert_eq!(st[0].result, BackupResult::Ok);
        assert!(!st[0].stale);
        assert_eq!(st[1].result, BackupResult::Failed);
        assert!(st[1].summary.as_deref().unwrap().contains("0x80070002"));
        assert_eq!(st[2].size, Some(4096));
        assert!(st[3].error.as_deref().unwrap().contains("scheduled task"));
        assert!(st[4].stale);
    }

    #[test]
    fn one_job_is_not_an_array() {
        let jobs = vec![job(7, BackupSource::Path, "D:\\x")];
        let out = r#"{"now":10,"uptime":5,"jobs":{"id":7,"found":false,"denied":true}}"#;
        let st = parse_status(out, &jobs, 0.0).unwrap();
        assert!(st[0].error.as_deref().unwrap().contains("can't be read"));
    }

    #[test]
    fn detects_tasks() {
        let out = "T\t\\Backups\\Nightly restic\tC:\\restic\\restic.exe backup D:\\data\n\
                   T\t\\Other\\Cleanup\tcmd.exe /c del temp\n\
                   T\t\\Microsoft\\Windows\\Backup\\Microsoft-Windows-WindowsBackup\twbadmin %windir%\\System32\\wbadmin.exe start backup\n";
        let s = parse_detect(out);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].tool, BackupTool::Restic);
        assert_eq!(s[0].target, "\\Backups\\Nightly restic");
        assert_eq!(s[1].name, "Windows Server Backup");
        assert_eq!(s[1].tool, BackupTool::Other);
    }

    #[test]
    fn scripts_are_ps51() {
        let s = status_script(&[job(1, BackupSource::Unit, "\\A\\B"), job(2, BackupSource::Path, "D:\\x")]);
        assert!(!s.contains("??") && !s.contains("&&") && !DETECT.contains("&&"));
    }

    #[cfg(windows)]
    #[test]
    fn runs_locally() {
        let jobs = vec![
            job(1, BackupSource::Unit, "\\Microsoft\\Windows\\Defrag\\ScheduledDefrag"),
            job(2, BackupSource::Path, "C:\\Windows\\Fonts"),
        ];
        let o = super::super::run_local(&status_script(&jobs), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let st = parse_status(&o.stdout, &jobs, 0.0).unwrap();
        assert!(st[0].error.is_none(), "{:?} / {}", st[0], o.stdout);
        assert!(st[1].last_run.is_some(), "{:?} / {}", st[1], o.stdout);
        let d = super::super::run_local(DETECT, b"").unwrap();
        assert_eq!(d.code, 0, "{}", d.stderr);
    }
}
