//! Windows Update for System → UPDATES and Fleet → UPDATES.
//!
//! Checking asks the Windows Update Agent directly. Installing can't: the
//! agent refuses to download or install for a remote (SSH) logon. So, like
//! Microsoft's own remote tools, the install runs as a one-off scheduled
//! task as SYSTEM, which writes a log that this session streams back, and
//! the task is removed when it ends. Installing needs an administrator.
//!
//! Holding an update (D-195) hides it in Windows Update, which every tool
//! on the host respects; releasing un-hides it.

use serde::Deserialize;

use crate::ssh::client::Connection;
use crate::system::packages::{Manager, PackageUpdate, UpdatesReport};
use crate::system::{SystemError, SystemResult};

use super::{run, IS_ADMIN};

const CHECK: &str = r#"$ErrorActionPreference = 'Stop'
$o = [ordered]@{ updates = @(); hidden = @(); error = $null; reboot = @() }
$rk = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired'
if (Test-Path $rk) { $o.reboot += 'Windows Update is waiting for a restart' }
if (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') { $o.reboot += 'a Windows component is waiting for a restart' }
try {
  $s = New-Object -ComObject Microsoft.Update.Session
  $s.ClientApplicationID = 'Nightgrid'
  $r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Software'")
  $all = @(foreach ($u in $r.Updates) {
    [ordered]@{
      title = [string]$u.Title
      kb = (@($u.KBArticleIDs) | ForEach-Object { 'KB' + $_ }) -join ', '
      cat = (@($u.Categories) | Select-Object -First 1 | ForEach-Object { $_.Name }) -join ''
      severity = [string]$u.MsrcSeverity
      hidden = [bool]$u.IsHidden
    }
  })
  $o.updates = @($all | Where-Object { -not $_.hidden })
  $o.hidden = @($all | Where-Object { $_.hidden })
} catch {
  $o.error = $_.Exception.Message
}
ConvertTo-Json -InputObject $o -Depth 3 -Compress"#;

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    updates: Vec<RawUpdate>,
    /// Updates hidden in Windows Update: held.
    #[serde(default)]
    hidden: Vec<RawUpdate>,
    error: Option<String>,
    #[serde(default)]
    reboot: Vec<String>,
}

#[derive(Deserialize)]
struct RawUpdate {
    title: String,
    #[serde(default)]
    kb: String,
    #[serde(default)]
    cat: String,
    #[serde(default)]
    severity: String,
}

/// Parses the [`CHECK`] JSON.
pub fn parse(json: &str) -> SystemResult<UpdatesReport> {
    let raw: Raw = serde_json::from_str(json.trim())
        .map_err(|e| SystemError::other(format!("couldn't read Windows Update's answer: {e}")))?;
    let note = raw.error.map(|e| {
        if e.contains("0x80070005") || e.to_ascii_lowercase().contains("access is denied") {
            "Windows Update didn't answer this account. Log in as an administrator to check for updates.".to_owned()
        } else {
            format!("Windows Update couldn't search: {}", e.trim())
        }
    });
    let held: Vec<String> = raw.hidden.iter().map(|u| u.title.clone()).collect();
    let hidden = raw.hidden.into_iter().map(|u| (u, true));
    let updates = raw
        .updates
        .into_iter()
        .map(|u| (u, false))
        .chain(hidden)
        .map(|(u, held)| {
            let repo = match (u.cat.trim(), u.severity.trim()) {
                ("", "") => None,
                (c, "") => Some(c.to_owned()),
                ("", s) => Some(format!("{s} security")),
                // "security" in the repo marks the row as a security update.
                (c, s) => Some(format!("{c} · {s} security")),
            };
            PackageUpdate {
                available: if u.kb.is_empty() { "update".into() } else { u.kb },
                name: u.title,
                current: None,
                repo,
                held,
            }
        })
        .collect();
    Ok(UpdatesReport {
        manager: Some(Manager::WindowsUpdate),
        updates,
        reboot_required: !raw.reboot.is_empty(),
        reboot_reasons: raw.reboot,
        note,
        held,
        hold_support: crate::system::packages::HoldSupport::Host,
    })
}

/// Hides (holds) or un-hides (releases) updates by title. Needs an
/// administrator, like installing.
const HOLD: &str = r#"$ErrorActionPreference = 'Stop'
if (-not (__ADMIN__)) { [Console]::Error.WriteLine('Holding Windows updates needs an administrator account. Log in to this host as an administrator (Windows has no sudo).'); exit 5 }
$want = @(__TITLES__)
$s = New-Object -ComObject Microsoft.Update.Session
$s.ClientApplicationID = 'Nightgrid'
$r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Software'")
$n = 0
foreach ($u in $r.Updates) { if ($want -contains [string]$u.Title) { $u.IsHidden = __HIDE__; $n++ } }
[Console]::Out.WriteLine('changed ' + $n)"#;

/// The PowerShell that holds (`hide`) or releases these updates.
pub fn hold_script(titles: &[String], hide: bool) -> SystemResult<String> {
    if titles.is_empty() {
        return Err(SystemError::other("no update named"));
    }
    if titles.iter().any(|t| t.trim().is_empty() || t.len() > 400 || t.chars().any(char::is_control)) {
        return Err(SystemError::other("not an update title"));
    }
    let list: Vec<String> = titles.iter().map(|t| super::quote(t)).collect();
    Ok(HOLD
        .replace("__ADMIN__", IS_ADMIN)
        .replace("__HIDE__", if hide { "$true" } else { "$false" })
        .replace("__TITLES__", &list.join(", ")))
}

/// Holds or releases updates on the host; how many it changed.
pub async fn hold(conn: &Connection, titles: &[String], hide: bool) -> SystemResult<u32> {
    let o = run(conn, &hold_script(titles, hide)?).await.map_err(SystemError::other)?;
    if o.code != 0 {
        let t = o.error_text();
        return Err(SystemError::other(if t.is_empty() { format!("exit {}", o.code) } else { t }));
    }
    Ok(o.stdout.trim().strip_prefix("changed ").and_then(|n| n.trim().parse().ok()).unwrap_or(0))
}

pub async fn check(conn: &Connection) -> SystemResult<UpdatesReport> {
    let o = run(conn, CHECK).await.map_err(SystemError::other)?;
    if o.code != 0 || o.stdout.trim().is_empty() {
        let t = o.error_text();
        return Err(SystemError::other(if t.is_empty() {
            format!("checking Windows Update failed (exit {})", o.code)
        } else {
            t
        }));
    }
    parse(&o.stdout)
}

/// Runs as SYSTEM in the scheduled task; `$log` is set before it.
const INSTALL_INNER: &str = r#"function L($m) { for ($t = 0; $t -lt 5; $t++) { try { Add-Content -Path $log -Value $m -Encoding UTF8 -ErrorAction Stop; return } catch { Start-Sleep -Milliseconds 200 } } }
try {
  $s = New-Object -ComObject Microsoft.Update.Session
  $s.ClientApplicationID = 'Nightgrid'
  L 'Searching for updates...'
  $r = $s.CreateUpdateSearcher().Search("IsInstalled=0 and IsHidden=0 and Type='Software'")
  if ($r.Updates.Count -eq 0) { L 'No updates to install.'; L '@@done 0'; exit }
  $c = New-Object -ComObject Microsoft.Update.UpdateColl
  foreach ($u in $r.Updates) { if (-not $u.EulaAccepted) { $u.AcceptEula() }; [void]$c.Add($u); L ('Queued: ' + $u.Title) }
  L ('Downloading ' + $c.Count + ' update(s)...')
  $d = $s.CreateUpdateDownloader(); $d.Updates = $c; [void]$d.Download()
  L 'Installing...'
  $i = $s.CreateUpdateInstaller(); $i.Updates = $c; $ir = $i.Install()
  $words = 'not started', 'in progress', 'installed', 'installed with errors', 'failed', 'aborted'
  for ($k = 0; $k -lt $c.Count; $k++) { L ($c.Item($k).Title + ': ' + $words[$ir.GetUpdateResult($k).ResultCode]) }
  if ($ir.RebootRequired) { L 'A restart is needed to finish installing.' }
  L ('@@done ' + $(if ($ir.ResultCode -eq 2) { 0 } else { 1 }))
} catch {
  L ('Error: ' + $_.Exception.Message)
  L '@@done 1'
}"#;

/// Runs over SSH: registers the task, streams its log, removes it.
const INSTALL_OUTER: &str = r#"$ErrorActionPreference = 'Stop'
if (-not (__ADMIN__)) { [Console]::Error.WriteLine('Installing Windows updates needs an administrator account. Log in to this host as an administrator (Windows has no sudo).'); exit 5 }
$dir = Join-Path $env:ProgramData 'Nightgrid'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$id = [guid]::NewGuid().ToString('N')
$log = Join-Path $dir ('windows-update-' + $id + '.log')
$inner = "`$log = '" + $log + "'`n" + [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__INNER__'))
$enc = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($inner))
$name = 'Windows Update ' + $id
$tp = '\Nightgrid\'
$a = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument ('-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand ' + $enc)
$p = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
$set = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 4)
Register-ScheduledTask -TaskName $name -TaskPath $tp -Action $a -Principal $p -Settings $set -Force | Out-Null
$done = $null
try {
  Start-ScheduledTask -TaskName $name -TaskPath $tp
  [Console]::Out.WriteLine('Started Windows Update as SYSTEM (scheduled task ' + $tp + $name + ').')
  [Console]::Out.Flush()
  $pos = 0; $started = Get-Date
  while ($null -eq $done) {
    Start-Sleep -Seconds 2
    $lines = @()
    if (Test-Path $log) { try { $lines = @(Get-Content -Path $log -Encoding UTF8 -ErrorAction Stop) } catch { } }
    for (; $pos -lt $lines.Count; $pos++) {
      $l = $lines[$pos]
      if ($l -like '@@done *') { $done = [int]$l.Substring(7) } else { [Console]::Out.WriteLine($l) }
    }
    [Console]::Out.Flush()
    $st = (Get-ScheduledTask -TaskName $name -TaskPath $tp).State
    if ($null -eq $done -and $st -ne 'Running' -and $st -ne 'Queued' -and ((Get-Date) - $started).TotalSeconds -gt 20) {
      Start-Sleep -Seconds 2
      if (Test-Path $log) { $lines = @(Get-Content -Path $log -Encoding UTF8); for (; $pos -lt $lines.Count; $pos++) { if ($lines[$pos] -like '@@done *') { $done = [int]$lines[$pos].Substring(7) } else { [Console]::Out.WriteLine($lines[$pos]) } } }
      if ($null -eq $done) { [Console]::Error.WriteLine('The Windows Update task ended without finishing (state ' + $st + ').'); $done = 1 }
    }
  }
} finally {
  Unregister-ScheduledTask -TaskName $name -TaskPath $tp -Confirm:$false -ErrorAction SilentlyContinue
  Remove-Item -Path $log -ErrorAction SilentlyContinue
}
exit $done"#;

/// The PowerShell that installs every available update and streams progress.
pub fn install_script() -> String {
    use base64::Engine as _;
    let inner = base64::engine::general_purpose::STANDARD.encode(INSTALL_INNER.as_bytes());
    INSTALL_OUTER.replace("__ADMIN__", IS_ADMIN).replace("__INNER__", &inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_check() {
        let r = parse(
            r#"{"updates":[{"title":"Cumulative Update (KB5049983)","kb":"KB5049983","cat":"Security Updates","severity":"Critical"},
                          {"title":"Defender update","kb":"KB2267602","cat":"Definition Updates","severity":""}],
                "error":null,"reboot":["Windows Update is waiting for a restart"]}"#,
        )
        .unwrap();
        assert_eq!(r.manager, Some(Manager::WindowsUpdate));
        assert_eq!(r.updates.len(), 2);
        assert_eq!(r.updates[0].available, "KB5049983");
        assert!(r.updates[0].repo.as_deref().unwrap().contains("security"));
        assert_eq!(r.updates[1].repo.as_deref(), Some("Definition Updates"));
        assert!(r.reboot_required);
        assert!(r.note.is_none());
    }

    #[test]
    fn hidden_updates_are_held() {
        let r = parse(
            r#"{"updates":[{"title":"Feature update","kb":"KB1","cat":"Upgrades","severity":""}],
                "hidden":[{"title":"Driver update","kb":"","cat":"Drivers","severity":""}],"error":null,"reboot":[]}"#,
        )
        .unwrap();
        assert_eq!(r.updates.len(), 2);
        assert!(r.updates[1].held && !r.updates[0].held);
        assert_eq!(r.held, ["Driver update"]);
        assert_eq!(r.pending(), 1);
        let s = hold_script(&["It's an update".to_owned()], true).unwrap();
        assert!(s.contains("'It''s an update'") && s.contains("$u.IsHidden = $true") && !s.contains("__"));
        assert!(hold_script(&[], true).is_err());
        assert!(hold_script(&["a\nb".to_owned()], false).is_err());
    }

    #[test]
    fn denied_search_is_a_note() {
        let r = parse(r#"{"updates":[],"error":"Exception from HRESULT: 0x80070005 (E_ACCESSDENIED)","reboot":[]}"#)
            .unwrap();
        assert!(r.note.unwrap().contains("administrator"));
        assert!(!r.reboot_required);
    }

    #[test]
    fn install_script_embeds_the_inner_script() {
        let s = install_script();
        assert!(s.contains("IsInRole") && !s.contains("__INNER__") && !s.contains("__ADMIN__"));
    }

    #[cfg(windows)]
    #[test]
    fn checks_locally() {
        let o = super::super::run_local(CHECK, b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        parse(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
    }
}
