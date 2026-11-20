//! Windows Defender Firewall for System → FIREWALL: profiles and the rules
//! you made (not the hundreds Windows and apps ship), with the same safety
//! net as ufw and firewalld. Every change first exports the whole policy
//! (`netsh advfirewall export`) and arms a one-off SYSTEM scheduled task
//! that imports it back after the chosen time, unless Nightgrid cancels it
//! once the user confirms a new login still works. Needs an administrator.

use serde::{Deserialize, Serialize};

use crate::system::firewall::{proto_name, valid_comment_text, valid_port_range, valid_source, FwChange, Proto};
use crate::{Error, Result};

use super::{quote, IS_ADMIN};

fn invalid(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WinProfile {
    /// `Domain`, `Private` or `Public`.
    pub name: String,
    pub enabled: bool,
    /// `Block`, `Allow` or `NotConfigured` (which blocks).
    pub default_in: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WinRule {
    /// The rule's unique name (what changes refer to).
    pub name: String,
    pub display: String,
    pub enabled: bool,
    /// `Allow` or `Block`.
    pub action: String,
    /// `TCP`, `UDP`, `Any`, ...
    pub protocol: String,
    /// Local ports (`22`, `8000-8100`, `Any`).
    pub ports: String,
    /// Remote addresses (`Any`, `192.168.1.0/24`, ...).
    pub remote: String,
    /// Profiles it applies to (`Any`, `Private, Public`).
    pub profiles: String,
    /// The program it's limited to, if any.
    pub program: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WinFirewall {
    pub profiles: Vec<WinProfile>,
    /// Inbound rules you (or Nightgrid) made: no rule group.
    pub rules: Vec<WinRule>,
    /// Enabled inbound allow rules that Windows and apps ship (not listed).
    pub builtin_allow: u32,
}

/// The scheduled task that restores the export.
pub const ROLLBACK_TASK: &str = "Firewall rollback";

pub const READ: &str = r#"$ErrorActionPreference = 'Stop'
if (-not (__ADMIN__)) { [Console]::Error.WriteLine('Reading the firewall needs an administrator account. Log in to this host as an administrator (Windows has no sudo).'); exit 5 }
$profiles = @(Get-NetFirewallProfile | ForEach-Object { [ordered]@{ name = [string]$_.Name; enabled = ([string]$_.Enabled -eq 'True'); defaultIn = [string]$_.DefaultInboundAction } })
$in = @(Get-NetFirewallRule -Direction Inbound)
$own = @($in | Where-Object { -not $_.Group })
$rules = @($own | ForEach-Object {
  $pf = $_ | Get-NetFirewallPortFilter
  $af = $_ | Get-NetFirewallAddressFilter
  $ap = $_ | Get-NetFirewallApplicationFilter
  [ordered]@{
    name = [string]$_.Name; display = [string]$_.DisplayName; enabled = ([string]$_.Enabled -eq 'True'); action = [string]$_.Action
    protocol = [string]$pf.Protocol; ports = (@($pf.LocalPort) -join ', '); remote = (@($af.RemoteAddress) -join ', ')
    profiles = [string]$_.Profile; program = $(if ($ap.Program -and $ap.Program -ne 'Any') { [string]$ap.Program } else { $null })
  }
})
$builtin = @($in | Where-Object { $_.Group -and [string]$_.Enabled -eq 'True' -and [string]$_.Action -eq 'Allow' }).Count
$armed = [bool](Get-ScheduledTask -TaskPath '\Nightgrid\' -TaskName '__TASK__' -ErrorAction SilentlyContinue)
ConvertTo-Json -InputObject ([ordered]@{ profiles = $profiles; rules = $rules; builtinAllow = $builtin; armed = $armed }) -Depth 4 -Compress"#;

pub fn read_script() -> String {
    READ.replace("__ADMIN__", IS_ADMIN).replace("__TASK__", ROLLBACK_TASK)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    #[serde(default)]
    profiles: Vec<WinProfile>,
    #[serde(default)]
    rules: Vec<WinRule>,
    #[serde(default)]
    builtin_allow: u32,
    #[serde(default)]
    armed: bool,
}

/// Parses the [`READ`] JSON into the shared report.
pub fn parse(json: &str) -> Result<crate::system::firewall::FirewallReport> {
    let raw: Raw = serde_json::from_str(json.trim())
        .map_err(|e| invalid("firewall", format!("couldn't read the firewall: {e}")))?;
    let mut rules = raw.rules;
    rules.sort_by_key(|r| r.display.to_lowercase());
    Ok(crate::system::firewall::FirewallReport {
        backend: crate::system::firewall::Backend::Windows,
        active: raw.profiles.iter().any(|p| p.enabled),
        ufw: None,
        zones: Vec::new(),
        raw: String::new(),
        rollback_armed: raw.armed,
        windows: Some(WinFirewall { profiles: raw.profiles, rules, builtin_allow: raw.builtin_allow }),
    })
}

fn valid_rule_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 256 && !n.chars().any(|c| c.is_control() || "'\"`*?[]".contains(c))
}

/// The PowerShell for one change (run after the snapshot).
pub fn change_script(c: &FwChange) -> Result<String> {
    Ok(match c {
        FwChange::WinAllow { port, proto, from, comment } => {
            let port = port.trim();
            if !valid_port_range(port) {
                return Err(invalid("port", "a port (1-65535) or a range like 8000-8100"));
            }
            if *proto == Proto::Any {
                return Err(invalid("proto", "Windows port rules need tcp or udp"));
            }
            let from = from.as_deref().map(str::trim).filter(|f| !f.is_empty());
            if from.is_some_and(|f| !valid_source(f)) {
                return Err(invalid("from", "an IP address or network"));
            }
            let comment = comment.as_deref().map(str::trim).filter(|c| !c.is_empty());
            if comment.is_some_and(|c| !valid_comment_text(c)) {
                return Err(invalid("comment", "up to 60 letters, digits, spaces, dots, dashes"));
            }
            let p = proto_name(*proto).to_uppercase();
            let display =
                format!("Nightgrid: {}", comment.map_or_else(|| format!("{port}/{}", p.to_lowercase()), str::to_owned));
            format!(
                "New-NetFirewallRule -DisplayName {} -Direction Inbound -Action Allow -Protocol {p} -LocalPort {} -RemoteAddress {} -Profile Any | Out-Null",
                quote(&display),
                quote(port),
                quote(from.unwrap_or("Any"))
            )
        }
        FwChange::WinDelete { name } => {
            if !valid_rule_name(name) {
                return Err(invalid("name", "a rule name from the list"));
            }
            format!("Remove-NetFirewallRule -Name {}", quote(name))
        }
        FwChange::WinRuleEnable { name, enabled } => {
            if !valid_rule_name(name) {
                return Err(invalid("name", "a rule name from the list"));
            }
            format!("Set-NetFirewallRule -Name {} -Enabled {}", quote(name), if *enabled { "True" } else { "False" })
        }
        FwChange::WinEnable { enabled } => {
            format!("Set-NetFirewallProfile -All -Enabled {}", if *enabled { "True" } else { "False" })
        }
        FwChange::WinDefault { policy } => {
            let p = match policy.as_str() {
                "block" => "Block",
                "allow" => "Allow",
                _ => return Err(invalid("policy", "block or allow")),
            };
            format!("Set-NetFirewallProfile -All -DefaultInboundAction {p}")
        }
        _ => return Err(invalid("change", "not a Windows firewall change")),
    })
}

/// Export, arm the rollback task for `secs`, apply. Prints `@@armed task`
/// and `@@applied`, or `@@failed` after restoring at once.
pub fn apply_script(c: &FwChange, token: &str, secs: u32) -> Result<String> {
    if !crate::system::firewall::valid_token(token) {
        return Err(invalid("token", "digits"));
    }
    let change = change_script(c)?;
    let secs = secs.clamp(30, 900);
    Ok(format!(
        r#"$ErrorActionPreference = 'Stop'
if (-not ({IS_ADMIN})) {{ [Console]::Error.WriteLine('Changing the firewall needs an administrator account.'); exit 5 }}
$dir = Join-Path $env:ProgramData 'Nightgrid\fw-rollback'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Get-ChildItem $dir -Filter '*.wfw' | Sort-Object LastWriteTime -Descending | Select-Object -Skip 5 | Remove-Item -ErrorAction SilentlyContinue
$f = Join-Path $dir '{token}.wfw'
Remove-Item $f -ErrorAction SilentlyContinue
$o = & netsh advfirewall export $f 2>&1
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $f)) {{ [Console]::Error.WriteLine('could not save the current rules: ' + ($o -join ' ')); exit 2 }}
$a = New-ScheduledTaskAction -Execute 'netsh.exe' -Argument ('advfirewall import "' + $f + '"')
$t = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddSeconds({secs}))
$p = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
try {{
  Register-ScheduledTask -TaskPath '\Nightgrid\' -TaskName '{ROLLBACK_TASK}' -Action $a -Trigger $t -Principal $p -Force | Out-Null
}} catch {{ [Console]::Error.WriteLine('could not arm the rollback: ' + $_.Exception.Message); exit 2 }}
[Console]::Out.WriteLine('@@armed task')
try {{
  {change}
  [Console]::Out.WriteLine('@@applied')
}} catch {{
  [Console]::Error.WriteLine($_.Exception.Message)
  & netsh advfirewall import $f | Out-Null
  Unregister-ScheduledTask -TaskPath '\Nightgrid\' -TaskName '{ROLLBACK_TASK}' -Confirm:$false -ErrorAction SilentlyContinue
  [Console]::Out.WriteLine('@@failed 1')
}}"#
    ))
}

/// Cancels the armed rollback (keeps the change). Prints `kept`.
pub fn confirm_script() -> String {
    format!(
        "Unregister-ScheduledTask -TaskPath '\\Nightgrid\\' -TaskName '{ROLLBACK_TASK}' -Confirm:$false -ErrorAction SilentlyContinue\n[Console]::Out.WriteLine('kept')"
    )
}

/// Restores the export now and disarms the task. Prints `restored`.
pub fn rollback_script(token: &str) -> Result<String> {
    if !crate::system::firewall::valid_token(token) {
        return Err(invalid("token", "digits"));
    }
    Ok(format!(
        r#"$f = Join-Path $env:ProgramData 'Nightgrid\fw-rollback\{token}.wfw'
if (-not (Test-Path $f)) {{ [Console]::Error.WriteLine('no snapshot to restore'); exit 2 }}
Unregister-ScheduledTask -TaskPath '\Nightgrid\' -TaskName '{ROLLBACK_TASK}' -Confirm:$false -ErrorAction SilentlyContinue
$o = & netsh advfirewall import $f 2>&1
if ($LASTEXITCODE -ne 0) {{ [Console]::Error.WriteLine($o -join ' '); exit 1 }}
[Console]::Out.WriteLine('restored')"#
    ))
}

fn covers(ports: &str, port: u16) -> bool {
    ports.split(',').map(str::trim).any(|p| match p.split_once('-') {
        _ if p.eq_ignore_ascii_case("any") => true,
        Some((a, b)) => matches!((a.parse::<u16>(), b.parse::<u16>()), (Ok(a), Ok(b)) if (a..=b).contains(&port)),
        None => p.parse::<u16>() == Ok(port),
    })
}

/// Whether an enabled own rule allows TCP `port` (other than `without`).
/// Windows' own OpenSSH rule ("OpenSSH-Server-In-TCP") has no group, so it
/// is among these.
fn allows(fw: &WinFirewall, port: u16, without: Option<&str>) -> bool {
    fw.rules.iter().any(|r| {
        r.enabled
            && r.action == "Allow"
            && Some(r.name.as_str()) != without
            && (r.protocol.eq_ignore_ascii_case("tcp") || r.protocol.eq_ignore_ascii_case("any"))
            && covers(&r.ports, port)
            && r.program.is_none()
    })
}

/// Why a Windows change could cut off SSH on `ssh_port`.
pub fn lockout_risk(fw: &WinFirewall, c: &FwChange, ssh_port: u16) -> Option<String> {
    let msg = |what: &str| Some(format!("{what}: Nightgrid connects on port {ssh_port} and could be locked out."));
    let target = |name: &str| fw.rules.iter().find(|r| r.name == name);
    match c {
        FwChange::WinEnable { enabled: true } | FwChange::WinDefault { .. } if !allows(fw, ssh_port, None) => {
            if matches!(c, FwChange::WinDefault { policy } if policy == "allow") {
                None
            } else {
                msg("No rule you can see here allows SSH")
            }
        }
        FwChange::WinDelete { name } | FwChange::WinRuleEnable { name, enabled: false }
            if target(name).is_some_and(|r| r.action == "Allow" && covers(&r.ports, ssh_port))
                && !allows(fw, ssh_port, Some(name)) =>
        {
            msg("This is the rule that lets SSH in")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fw() -> WinFirewall {
        parse(
            r#"{"profiles":[{"name":"Domain","enabled":true,"defaultIn":"NotConfigured"},{"name":"Private","enabled":true,"defaultIn":"Block"},{"name":"Public","enabled":true,"defaultIn":"Block"}],
                "rules":[{"name":"OpenSSH-Server-In-TCP","display":"OpenSSH SSH Server (sshd)","enabled":true,"action":"Allow","protocol":"TCP","ports":"22","remote":"Any","profiles":"Any","program":null},
                         {"name":"{abc}","display":"Plex","enabled":false,"action":"Allow","protocol":"TCP","ports":"32400","remote":"Any","profiles":"Private","program":"C:\\Plex\\plex.exe"}],
                "builtinAllow":57,"armed":false}"#,
        )
        .unwrap()
        .windows
        .unwrap()
    }

    #[test]
    fn reads() {
        let w = fw();
        assert_eq!(w.profiles.len(), 3);
        assert_eq!(w.rules[0].display, "OpenSSH SSH Server (sshd)");
        assert_eq!(w.builtin_allow, 57);
    }

    #[test]
    fn scripts() {
        let s = change_script(&FwChange::WinAllow {
            port: "8080".into(),
            proto: Proto::Tcp,
            from: Some("192.168.1.0/24".into()),
            comment: Some("web ui".into()),
        })
        .unwrap();
        assert!(
            s.contains("-DisplayName 'Nightgrid: web ui'")
                && s.contains("-LocalPort '8080'")
                && s.contains("-RemoteAddress '192.168.1.0/24'"),
            "{s}"
        );
        assert!(change_script(&FwChange::WinAllow { port: "x".into(), proto: Proto::Tcp, from: None, comment: None })
            .is_err());
        assert!(change_script(&FwChange::WinAllow { port: "80".into(), proto: Proto::Any, from: None, comment: None })
            .is_err());
        assert!(change_script(&FwChange::WinDelete { name: "a'b".into() }).is_err());
        assert!(change_script(&FwChange::WinDefault { policy: "drop".into() }).is_err());
        let a = apply_script(&FwChange::WinEnable { enabled: true }, "1763500000", 120).unwrap();
        assert!(
            a.contains("netsh advfirewall export")
                && a.contains("AddSeconds(120)")
                && a.contains("Set-NetFirewallProfile -All -Enabled True")
        );
        assert!(apply_script(&FwChange::WinEnable { enabled: true }, "x;y", 120).is_err());
    }

    #[test]
    fn lockout() {
        let w = fw();
        assert!(lockout_risk(&w, &FwChange::WinDelete { name: "OpenSSH-Server-In-TCP".into() }, 22).is_some());
        assert!(lockout_risk(&w, &FwChange::WinDelete { name: "{abc}".into() }, 22).is_none());
        assert!(lockout_risk(&w, &FwChange::WinDefault { policy: "block".into() }, 22).is_none());
        assert!(lockout_risk(&w, &FwChange::WinDefault { policy: "block".into() }, 2222).is_some());
        assert!(lockout_risk(&w, &FwChange::WinDefault { policy: "allow".into() }, 2222).is_none());
    }
}
