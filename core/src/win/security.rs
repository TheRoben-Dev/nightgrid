//! Security checks for Windows hosts (System → SECURITY and posture): the
//! Windows counterparts of the Linux checks, read only and without
//! administrator rights where Windows allows it. Scored with the same
//! weights and grades as Linux hosts.

use serde::Deserialize;

use crate::ssh::client::Connection;
use crate::system::security::{
    is_sensitive, scope_of, score, service_name, sshd_option, Check, CheckStatus, Listener, Scope, SecurityReport,
};
use crate::system::{SystemError, SystemResult};

use super::{run, IS_ADMIN};

/// Everything in one JSON document. Each probe is wrapped so a missing
/// module or a refused read becomes null (an Unknown check).
const AUDIT: &str = r#"$ErrorActionPreference = 'Stop'
function Safe($b) { try { & $b } catch { $null } }
function Reg($p, $n) { try { (Get-ItemProperty -Path $p -Name $n -ErrorAction Stop).$n } catch { $null } }
$r = [ordered]@{}
$r.admin = [bool](__ADMIN__)
$r.defender = Safe { $m = Get-MpComputerStatus; [ordered]@{ av = [bool]$m.AntivirusEnabled; rt = [bool]$m.RealTimeProtectionEnabled; sigAge = [int]$m.AntivirusSignatureAge } }
$r.firewall = Safe { @(Get-NetFirewallProfile | ForEach-Object { [ordered]@{ name = [string]$_.Name; enabled = [string]$_.Enabled; inbound = [string]$_.DefaultInboundAction } }) }
$r.rdpDeny = Reg 'HKLM:\System\CurrentControlSet\Control\Terminal Server' 'fDenyTSConnections'
$r.rdpNla = Reg 'HKLM:\System\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp' 'UserAuthentication'
$r.smb1 = Safe { [bool](Get-SmbServerConfiguration).EnableSMB1Protocol }
if ($null -eq $r.smb1) { $f = Safe { (Get-WindowsOptionalFeature -Online -FeatureName SMB1Protocol).State }; if ($f) { $r.smb1 = ([string]$f -eq 'Enabled') } }
$r.uac = Reg 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' 'EnableLUA'
$r.lastPatch = Safe { $h = Get-HotFix | Where-Object { $_.InstalledOn } | Sort-Object InstalledOn -Descending | Select-Object -First 1; if ($h) { [int]((Get-Date) - $h.InstalledOn).TotalDays } else { $null } }
$r.guest = Safe { [bool](Get-LocalUser | Where-Object { $_.SID.Value -like '*-501' } | Select-Object -First 1).Enabled }
$r.admins = Safe { @(Get-LocalGroupMember -SID 'S-1-5-32-544' | ForEach-Object { [string]$_.Name }) }
$r.sshd = Safe { Get-Content -Raw (Join-Path $env:ProgramData 'ssh\sshd_config') }
$procs = @{}; Get-Process | ForEach-Object { $procs[[int]$_.Id] = $_.ProcessName }
$r.tcp = Safe { @(Get-NetTCPConnection -State Listen | ForEach-Object { [ordered]@{ a = [string]$_.LocalAddress; p = [int]$_.LocalPort; n = $procs[[int]$_.OwningProcess] } }) }
$r.udp = Safe { @(Get-NetUDPEndpoint | Where-Object { $_.LocalPort -lt 49152 } | ForEach-Object { [ordered]@{ a = [string]$_.LocalAddress; p = [int]$_.LocalPort; n = $procs[[int]$_.OwningProcess] } }) }
$r | ConvertTo-Json -Depth 4 -Compress"#;

fn script() -> String {
    AUDIT.replace("__ADMIN__", IS_ADMIN)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Raw {
    admin: bool,
    defender: Option<Defender>,
    #[serde(deserialize_with = "many")]
    firewall: Option<Vec<Profile>>,
    rdp_deny: Option<i64>,
    rdp_nla: Option<i64>,
    smb1: Option<bool>,
    uac: Option<i64>,
    last_patch: Option<i64>,
    guest: Option<bool>,
    #[serde(deserialize_with = "many")]
    admins: Option<Vec<String>>,
    sshd: Option<String>,
    #[serde(deserialize_with = "many")]
    tcp: Option<Vec<Sock>>,
    #[serde(deserialize_with = "many")]
    udp: Option<Vec<Sock>>,
}

#[derive(Deserialize)]
struct Defender {
    av: bool,
    rt: bool,
    #[serde(rename = "sigAge")]
    sig_age: i64,
}

#[derive(Deserialize)]
struct Profile {
    name: String,
    enabled: String,
    inbound: String,
}

#[derive(Deserialize)]
struct Sock {
    a: String,
    p: u16,
    n: Option<String>,
}

/// PowerShell 5.1 writes a one-item array as the item itself.
fn many<'de, D, T>(d: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum M<T> {
        Many(Vec<T>),
        One(T),
    }
    Ok(Option::<M<T>>::deserialize(d)?.map(|m| match m {
        M::Many(v) => v,
        M::One(t) => vec![t],
    }))
}

fn listeners(raw: &Raw) -> Vec<Listener> {
    let mut out: Vec<Listener> = Vec::new();
    for (proto, socks) in [("tcp", &raw.tcp), ("udp", &raw.udp)] {
        for s in socks.iter().flatten() {
            let l = Listener {
                proto: proto.into(),
                addr: s.a.clone(),
                port: s.p,
                scope: scope_of(&s.a),
                process: s.n.clone().filter(|n| !n.is_empty()),
                container: None,
                service: service_name(s.p).map(str::to_owned),
            };
            if !out.iter().any(|o| o.proto == l.proto && o.addr == l.addr && o.port == l.port) {
                out.push(l);
            }
        }
    }
    out.sort_by(|a, b| (a.port, &a.proto, &a.addr).cmp(&(b.port, &b.proto, &b.addr)));
    out
}

/// Builds the report from the audit's JSON.
pub fn report(json: &str) -> SystemResult<SecurityReport> {
    let raw: Raw = serde_json::from_str(json.trim())
        .map_err(|e| SystemError::other(format!("couldn't read the security checks: {e}")))?;
    let mut checks = Vec::new();
    let check = |id: &str, title: &str, status: CheckStatus, detail: String, fix: Option<&str>, weight: u32| Check {
        id: id.into(),
        title: title.into(),
        status,
        detail,
        fix: fix.map(str::to_owned),
        weight,
    };
    let unknown = |what: &str| format!("Windows didn't report {what} to this account.");
    let listeners = listeners(&raw);
    let open = |port: u16| listeners.iter().any(|l| l.proto == "tcp" && l.port == port && l.scope == Scope::All);

    // Defender.
    checks.push(match &raw.defender {
        None => check("win-defender", "Antivirus is on", CheckStatus::Unknown, "Microsoft Defender didn't answer (it may be replaced by another antivirus, which Nightgrid can't read).".into(), None, 15),
        Some(d) if !d.av || !d.rt => check(
            "win-defender",
            "Antivirus is on",
            CheckStatus::Fail,
            format!("Defender antivirus is {}, real-time protection is {}.", on(d.av), on(d.rt)),
            Some("Turn real-time protection on: `Set-MpPreference -DisableRealtimeMonitoring $false` (as an administrator), or check that another antivirus is running."),
            15,
        ),
        Some(d) if d.sig_age > 7 => check(
            "win-defender",
            "Antivirus is on",
            CheckStatus::Warn,
            format!("Defender is on, but its signatures are {} days old.", d.sig_age),
            Some("Update them with `Update-MpSignature` and check that Windows Update can reach Microsoft."),
            15,
        ),
        Some(_) => check("win-defender", "Antivirus is on", CheckStatus::Pass, "Defender antivirus and real-time protection are on.".into(), None, 15),
    });

    // Firewall profiles.
    checks.push(match &raw.firewall {
        None => check("firewall", "A firewall is on", CheckStatus::Unknown, unknown("its firewall profiles"), None, 20),
        Some(p) => {
            let off: Vec<&str> = p.iter().filter(|x| x.enabled != "True").map(|x| x.name.as_str()).collect();
            let allow: Vec<&str> = p.iter().filter(|x| x.enabled == "True" && x.inbound == "Allow").map(|x| x.name.as_str()).collect();
            if !off.is_empty() {
                check(
                    "firewall",
                    "A firewall is on",
                    CheckStatus::Fail,
                    format!("Windows Defender Firewall is off for the {} profile{}.", off.join(", "), if off.len() == 1 { "" } else { "s" }),
                    Some("Turn it on with `Set-NetFirewallProfile -All -Enabled True` (as an administrator). Allow SSH first if it isn't: `New-NetFirewallRule -DisplayName OpenSSH -Direction Inbound -Protocol TCP -LocalPort 22 -Action Allow`."),
                    20,
                )
            } else if !allow.is_empty() {
                check(
                    "firewall",
                    "A firewall is on",
                    CheckStatus::Warn,
                    format!("The firewall is on, but the {} profile allows all inbound traffic by default.", allow.join(", ")),
                    Some("Block by default: `Set-NetFirewallProfile -All -DefaultInboundAction Block` (as an administrator); allowed apps keep their rules."),
                    20,
                )
            } else {
                check("firewall", "A firewall is on", CheckStatus::Pass, "Windows Defender Firewall is on for every profile and blocks unasked inbound traffic.".into(), None, 20)
            }
        }
    });

    // SSH password logins.
    match raw.sshd.as_deref() {
        None => checks.push(check(
            "ssh-password",
            "SSH needs keys, not passwords",
            CheckStatus::Unknown,
            unknown("the sshd config"),
            None,
            15,
        )),
        Some(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let pw = sshd_option(&lines, "passwordauthentication").unwrap_or_else(|| "yes".into());
            let (st, detail) = if pw == "no" {
                (CheckStatus::Pass, "Password logins are off.".to_owned())
            } else if open(22) {
                (CheckStatus::Fail, "Password logins are allowed and SSH listens on every interface.".to_owned())
            } else {
                (CheckStatus::Warn, "Password logins are allowed.".to_owned())
            };
            checks.push(check(
                "ssh-password",
                "SSH needs keys, not passwords",
                st,
                detail,
                Some("Once your key works (Settings → SSH keys can push it), set `PasswordAuthentication no` in `C:\\ProgramData\\ssh\\sshd_config` and run `Restart-Service sshd` as an administrator. Keep this session open until a new login works."),
                15,
            ));
        }
    }

    // RDP.
    checks.push(match (raw.rdp_deny, raw.rdp_nla) {
        (None, _) => check("win-rdp", "Remote Desktop needs sign-in first", CheckStatus::Unknown, unknown("the Remote Desktop setting"), None, 10),
        (Some(1), _) => check("win-rdp", "Remote Desktop needs sign-in first", CheckStatus::Pass, "Remote Desktop is off.".into(), None, 10),
        (Some(_), Some(0)) => check(
            "win-rdp",
            "Remote Desktop needs sign-in first",
            if open(3389) { CheckStatus::Fail } else { CheckStatus::Warn },
            "Remote Desktop is on without Network Level Authentication, so anyone who reaches port 3389 gets a sign-in screen.".into(),
            Some("Require NLA in System Properties → Remote, or `Set-ItemProperty 'HKLM:\\System\\CurrentControlSet\\Control\\Terminal Server\\WinStations\\RDP-Tcp' UserAuthentication 1`. Reach RDP over your tailnet or an SSH tunnel rather than opening 3389."),
            10,
        ),
        (Some(_), _) => check(
            "win-rdp",
            "Remote Desktop needs sign-in first",
            if open(3389) { CheckStatus::Warn } else { CheckStatus::Pass },
            format!("Remote Desktop is on with Network Level Authentication{}.", if open(3389) { ", listening on every interface" } else { "" }),
            Some("Keep 3389 off the internet: reach it over your tailnet or an SSH tunnel (Nightgrid's Tunnels can make one)."),
            10,
        ),
    });

    // SMBv1.
    checks.push(match raw.smb1 {
        None => check(
            "win-smb1",
            "SMBv1 is off",
            CheckStatus::Unknown,
            "Reading the SMB server settings needs an administrator account.".into(),
            None,
            10,
        ),
        Some(true) => check(
            "win-smb1",
            "SMBv1 is off",
            CheckStatus::Fail,
            "The SMB server still accepts SMBv1, the protocol WannaCry spread through.".into(),
            Some("Turn it off: `Set-SmbServerConfiguration -EnableSMB1Protocol $false -Force` (as an administrator)."),
            10,
        ),
        Some(false) => check(
            "win-smb1",
            "SMBv1 is off",
            CheckStatus::Pass,
            "The SMB server doesn't accept SMBv1.".into(),
            None,
            10,
        ),
    });

    // Updates.
    checks.push(match raw.last_patch {
        None => check("updates", "Updates are recent", CheckStatus::Unknown, unknown("installed updates"), None, 15),
        Some(d) if d > 60 => check(
            "updates",
            "Updates are recent",
            CheckStatus::Fail,
            format!("The newest installed update is {d} days old."),
            Some("Run Windows Update (Settings → Windows Update, or `sconfig` on Server Core). Windows publishes security updates monthly."),
            15,
        ),
        Some(d) if d > 35 => check(
            "updates",
            "Updates are recent",
            CheckStatus::Warn,
            format!("The newest installed update is {d} days old: last month's security update may be missing."),
            Some("Run Windows Update (Settings → Windows Update, or `sconfig` on Server Core)."),
            15,
        ),
        Some(d) => check("updates", "Updates are recent", CheckStatus::Pass, format!("An update was installed {d} days ago."), None, 15),
    });

    // UAC.
    checks.push(match raw.uac {
        None => check("win-uac", "User Account Control is on", CheckStatus::Unknown, unknown("the UAC setting"), None, 5),
        Some(0) => check(
            "win-uac",
            "User Account Control is on",
            CheckStatus::Warn,
            "UAC is off, so every program an administrator starts runs with full rights.".into(),
            Some("Set `EnableLUA` to 1 under `HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System` and restart."),
            5,
        ),
        Some(_) => check("win-uac", "User Account Control is on", CheckStatus::Pass, "UAC is on.".into(), None, 5),
    });

    // Guest account.
    checks.push(match raw.guest {
        None => {
            check("win-guest", "The Guest account is off", CheckStatus::Unknown, unknown("local accounts"), None, 5)
        }
        Some(true) => check(
            "win-guest",
            "The Guest account is off",
            CheckStatus::Fail,
            "The built-in Guest account is enabled.".into(),
            Some("Turn it off: `Disable-LocalUser -Name Guest` (as an administrator)."),
            5,
        ),
        Some(false) => check(
            "win-guest",
            "The Guest account is off",
            CheckStatus::Pass,
            "The Guest account is disabled.".into(),
            None,
            5,
        ),
    });

    // Databases on every interface.
    let exposed: Vec<String> = listeners
        .iter()
        .filter(|l| l.scope == Scope::All && is_sensitive(l.port))
        .map(|l| format!("{} on {}", l.service.as_deref().unwrap_or("?"), l.port))
        .collect();
    checks.push(if raw.tcp.is_none() {
        check("exposed-services", "Databases aren't open to the network", CheckStatus::Unknown, unknown("listening ports"), None, 20)
    } else if exposed.is_empty() {
        check("exposed-services", "Databases aren't open to the network", CheckStatus::Pass, "No database, cache or Docker API port listens on every interface.".into(), None, 20)
    } else {
        check(
            "exposed-services",
            "Databases aren't open to the network",
            CheckStatus::Fail,
            format!("Listening on every interface: {}.", exposed.join(", ")),
            Some("Bind them to 127.0.0.1 or the LAN address in their own settings, or block the ports in Windows Defender Firewall."),
            20,
        )
    });

    // Administrators (context).
    if let Some(a) = &raw.admins {
        checks.push(check(
            "win-admins",
            "Who is an administrator",
            CheckStatus::Info,
            if a.is_empty() { "No members were listed.".into() } else { format!("Administrators: {}.", a.join(", ")) },
            None,
            0,
        ));
    }
    if !raw.admin {
        checks.push(check(
            "win-account",
            "Checked as a standard account",
            CheckStatus::Info,
            "This account isn't an administrator, so some settings may show as unknown.".into(),
            None,
            0,
        ));
    }

    let (score, grade) = score(&checks);
    // Get-Process names every process, so nothing is hidden.
    Ok(SecurityReport { score, grade, checks, listeners, processes_partial: false })
}

fn on(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

pub async fn audit(conn: &Connection) -> SystemResult<SecurityReport> {
    let o = run(conn, &script()).await.map_err(SystemError::other)?;
    if o.code != 0 || o.stdout.trim().is_empty() {
        let t = o.error_text();
        return Err(SystemError::other(if t.is_empty() {
            format!("the security checks failed (exit {})", o.code)
        } else {
            t
        }));
    }
    report(&o.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(r: &SecurityReport, id: &str) -> CheckStatus {
        r.checks.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id}")).status
    }

    #[test]
    fn a_tidy_server() {
        let r = report(
            r#"{"admin":true,"defender":{"av":true,"rt":true,"sigAge":1},
            "firewall":[{"name":"Domain","enabled":"True","inbound":"Block"},{"name":"Private","enabled":"True","inbound":"NotConfigured"},{"name":"Public","enabled":"True","inbound":"Block"}],
            "rdpDeny":1,"rdpNla":1,"smb1":false,"uac":1,"lastPatch":12,"guest":false,"admins":"HOST\\Administrator",
            "sshd":"PasswordAuthentication no\r\nMatch Group administrators\r\n  AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys\r\n",
            "tcp":[{"a":"0.0.0.0","p":22,"n":"sshd"},{"a":"127.0.0.1","p":5432,"n":"postgres"}],"udp":null}"#,
        )
        .unwrap();
        assert_eq!(r.score, 100, "{:#?}", r.checks);
        assert_eq!(r.grade, "A");
        assert_eq!(status(&r, "win-admins"), CheckStatus::Info);
        assert_eq!(r.listeners.len(), 2);
        assert_eq!(r.listeners[0].process.as_deref(), Some("sshd"));
    }

    #[test]
    fn a_loose_one() {
        let r = report(
            r#"{"admin":false,"defender":{"av":true,"rt":false,"sigAge":30},
            "firewall":{"name":"Public","enabled":"False","inbound":"Block"},
            "rdpDeny":0,"rdpNla":0,"smb1":true,"uac":0,"lastPatch":90,"guest":true,"admins":null,"sshd":"",
            "tcp":[{"a":"0.0.0.0","p":22,"n":"sshd"},{"a":"::","p":3389,"n":"svchost"},{"a":"0.0.0.0","p":6379,"n":"redis-server"}],"udp":[]}"#,
        )
        .unwrap();
        for id in [
            "win-defender",
            "firewall",
            "ssh-password",
            "win-rdp",
            "win-smb1",
            "updates",
            "win-guest",
            "exposed-services",
        ] {
            assert_eq!(status(&r, id), CheckStatus::Fail, "{id}");
        }
        assert_eq!(status(&r, "win-uac"), CheckStatus::Warn);
        assert_eq!(r.grade, "F");
        assert!(r.checks.iter().any(|c| c.id == "win-account"));
    }

    #[test]
    fn nothing_readable() {
        let r = report(r#"{"admin":false}"#).unwrap();
        assert!(r.checks.iter().filter(|c| c.weight > 0).all(|c| c.status == CheckStatus::Unknown));
        assert_eq!(r.score, 100);
    }

    #[cfg(windows)]
    #[test]
    fn audits_locally() {
        let o = super::super::run_local(&script(), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let r = report(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", o.stdout));
        assert!(!r.listeners.is_empty(), "{}", o.stdout);
        assert!(r.checks.len() >= 8);
    }
}
