//! Fleet → ACCESS on Windows hosts: one read-only PowerShell script that
//! prints the same `@@section` layout as the POSIX audit, so
//! [`crate::access::parse`] builds the report. Logins come from the
//! OpenSSH/Operational event log (Win32-OpenSSH writes the same
//! `Accepted …` / `Failed …` lines sshd writes on Linux), local accounts
//! stand in for passwd (RID as uid), the Administrators group for sudo, and
//! `sshd -T` (or `sshd_config`) for the settings. Fail2ban and CrowdSec
//! don't exist there, so they show as missing.

use crate::access::{self, AccessReport};
use crate::hosts::HostId;

use super::IS_ADMIN;

/// The audit script, reading [`access::DAYS`] back.
pub fn script() -> String {
    format!(
        r#"$ErrorActionPreference = 'SilentlyContinue'
$admin = {IS_ADMIN}
$out = New-Object System.Text.StringBuilder
function W($s) {{ [void]$out.AppendLine([string]$s) }}
W '@@id'
W $env:USERNAME
W '0'
W ([int64](((Get-Date).ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds))
W '+0000'
if ($admin) {{ W '@admin' }}
$users = @(Get-LocalUser | Where-Object {{ $_.Enabled }})
W '@@passwd'
foreach ($u in $users) {{
  $rid = [int]($u.SID.Value.Split('-')[-1])
  $hd = Join-Path (Split-Path $env:USERPROFILE) $u.Name
  W ('{{0}}:x:{{1}}:0::{{2}}:powershell' -f $u.Name, $rid, $hd.Replace(':', '|'))
}}
W '@@group'
$admins = @(Get-LocalGroupMember -SID 'S-1-5-32-544' | Where-Object {{ $_.ObjectClass -eq 'User' }} | ForEach-Object {{ $_.Name.Split('\')[-1] }})
W ('admin:x:544:' + ($admins -join ','))
$since = (Get-Date).AddDays(-{days})
$ev = @()
try {{
  $ev = @(Get-WinEvent -FilterHashtable @{{ LogName = 'OpenSSH/Operational'; StartTime = $since }} -MaxEvents {lines} -ErrorAction Stop |
    Where-Object {{ $_.Message -match '(Accepted |Failed |Invalid user )' }})
}} catch {{ }}
if ($ev.Count) {{
  W '@@journal'
  foreach ($e in @($ev | Sort-Object TimeCreated)) {{
    $m = ($e.Message -replace '\s*\r?\n\s*', ' ').Trim()
    if ($m -notmatch '^sshd[^:]*: ') {{ $m = 'sshd: ' + $m }}
    $t = (($e.TimeCreated.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds).ToString('0.000', [Globalization.CultureInfo]::InvariantCulture)
    W ($t + ' ' + $env:COMPUTERNAME + ' ' + $m)
  }}
}} else {{ W '@@authlog' }}
W '@@last'
$q = & quser.exe 2>$null
foreach ($l in @($q | Select-Object -Skip 1 -First {sessions})) {{ W $l.Trim() }}
W '@@f2b'
W '@missing'
W '@@cscli'
W '@missing'
W '@@sudoers'
W '@@keys'
foreach ($u in $users) {{
  $f = Join-Path (Join-Path (Split-Path $env:USERPROFILE) $u.Name) '.ssh\authorized_keys'
  if (Test-Path -LiteralPath $f) {{ W ('@user ' + $u.Name); foreach ($l in @(Get-Content -LiteralPath $f)) {{ W $l }} }}
}}
$ak = Join-Path $env:ProgramData 'ssh\administrators_authorized_keys'
if (Test-Path -LiteralPath $ak) {{ W '@user Administrators'; foreach ($l in @(Get-Content -LiteralPath $ak)) {{ W $l }} }}
W '@@sshd'
$re = '^\s*({keys})\s'
$exe = Join-Path $env:SystemRoot 'System32\OpenSSH\sshd.exe'
$t = $null
if ($admin -and (Test-Path $exe)) {{ $t = & $exe -T 2>$null }}
if ($t) {{
  W '@effective'
  foreach ($l in @($t | Where-Object {{ $_ -match $re }})) {{ W $l }}
}} else {{
  foreach ($l in @(Get-Content (Join-Path $env:ProgramData 'ssh\sshd_config') | Where-Object {{ $_ -match $re }})) {{ W $l.Trim() }}
}}
W '@@end'
[Console]::Out.Write($out.ToString())
"#,
        days = access::DAYS,
        lines = access::LOG_LINES,
        sessions = access::MAX_SESSIONS,
        keys = access::SSHD_KEYS,
    )
}

/// The report from [`script`]'s output; `as_root` when it ran elevated.
pub fn parse(host_id: HostId, out: &str, now: i64) -> AccessReport {
    let admin = out.lines().any(|l| l.trim() == "@admin");
    let mut r = access::parse(host_id, out, admin, now);
    // Windows OpenSSH reads `Match Group administrators` keys from
    // administrators_authorized_keys: count them on the admins too.
    let admin_keys = r.keys.iter().filter(|k| k.user == "Administrators").count() as u32;
    let admins: Vec<String> = out
        .lines()
        .find_map(|l| l.strip_prefix("admin:x:544:"))
        .map(|m| m.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    for u in r.users.iter_mut() {
        u.shell = "PowerShell".into();
        // The drive's colon was sent as `|` to keep the passwd layout.
        u.home = u.home.replace('|', ":");
        if admins.iter().any(|a| a.eq_ignore_ascii_case(&u.name)) {
            u.keys += admin_keys;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl laptop";

    #[test]
    fn script_is_ps51() {
        let s = script();
        assert!(!s.contains("??") && !s.contains("&&"));
        assert!(s.contains("OpenSSH/Operational"));
    }

    #[test]
    fn parses_a_windows_report() {
        let out = format!(
            "@@id\nAdministrator\n0\n1794801000\n+0000\n@admin\n@@passwd\nAdministrator:x:500:0::C|\\Users\\Administrator:powershell\nops:x:1001:0::C|\\Users\\ops:powershell\n@@group\nadmin:x:544:Administrator\n@@journal\n1794800000.000 WIN sshd: Accepted publickey for administrator from 100.64.0.2 port 51000 ssh2: ED25519 SHA256:abc\n1794800100.000 WIN sshd: Failed password for ops from 203.0.113.9 port 4000 ssh2\n@@last\n@@f2b\n@missing\n@@cscli\n@missing\n@@sudoers\n@@keys\n@user Administrators\n{KEY}\n@@sshd\n@effective\npasswordauthentication yes\n@@end\n"
        );
        let r = parse(2, &out, 0);
        assert!(r.as_root);
        assert_eq!(r.logins.len(), 1);
        assert_eq!(r.logins[0].user, "administrator");
        assert_eq!(r.failed_total, 1);
        let a = r.users.iter().find(|u| u.name == "Administrator").unwrap();
        assert!(a.sudo && !a.system);
        assert_eq!(a.keys, 1);
        assert_eq!(a.home, "C:\\Users\\Administrator");
        assert!(!r.users.iter().find(|u| u.name == "ops").unwrap().sudo);
        assert!(r.findings.iter().any(|f| f.text.contains("nothing bans them")));
    }

    #[cfg(windows)]
    #[test]
    fn runs_locally() {
        let o = super::super::run_local(&script(), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stdout.contains("@@end"), "stdout: {}\nstderr: {}", o.stdout, o.stderr);
        let r = parse(1, &o.stdout, 0);
        assert!(!r.users.is_empty());
    }
}
