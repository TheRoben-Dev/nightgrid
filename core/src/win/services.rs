//! Windows services for System → UNITS: listed, described and started,
//! stopped, restarted or set to start automatically, as the user's own
//! account (Windows has no sudo: changing a service needs an administrator).
//! Rows reuse [`Unit`] so the tab works unchanged:
//! running → active, stopped auto-start services with an error exit code →
//! failed, start mode Auto/Manual/Disabled → enabled/manual/disabled.

use crate::ssh::client::Connection;
use crate::system::systemd::{Unit, UnitAction};
use crate::system::{SystemError, SystemResult};

use super::{quote, run, IS_ADMIN};

/// Tab-separated rows: name, state, start mode, exit code, display name.
const LIST: &str = r#"Get-CimInstance Win32_Service | Sort-Object Name | ForEach-Object {
  $d = ([string]$_.DisplayName) -replace '[\t\r\n]', ' '
  "{0}`t{1}`t{2}`t{3}`t{4}" -f $_.Name, $_.State, $_.StartMode, $_.ExitCode, $d
}"#;

/// Whether `name` can be a Windows service name passed to the cmdlets
/// (no wildcards, which `-Name` would expand, and no path separators).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && !name.starts_with('-')
        && !name.chars().any(|c| c.is_control() || "*?[]/\\\"'`".contains(c))
}

fn active_of(state: &str, mode: &str, exit: i64) -> (&'static str, String) {
    let sub = state.to_ascii_lowercase().replace(' ', "-");
    let active = match state {
        "Running" => "active",
        "Start Pending" | "Continue Pending" => "activating",
        "Stop Pending" | "Pause Pending" => "deactivating",
        "Paused" => "inactive",
        // 1077: never started since boot. Anything else on a stopped
        // automatic service means it failed.
        "Stopped" if mode == "Auto" && exit != 0 && exit != 1077 => "failed",
        _ => "inactive",
    };
    (active, sub)
}

fn mode_word(mode: &str) -> Option<String> {
    match mode {
        "Auto" => Some("enabled".into()),
        "Manual" => Some("manual".into()),
        "Disabled" => Some("disabled".into()),
        "Boot" | "System" => Some("static".into()),
        "" => None,
        m => Some(m.to_ascii_lowercase()),
    }
}

/// Parses [`LIST`] output.
pub fn parse_list(text: &str) -> Vec<Unit> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.trim_end_matches('\r').splitn(5, '\t');
            let (name, state, mode, exit, display) =
                (f.next()?, f.next()?, f.next()?, f.next()?, f.next().unwrap_or(""));
            if name.is_empty() {
                return None;
            }
            let (active, sub) = active_of(state, mode, exit.trim().parse().unwrap_or(0));
            Some(Unit {
                name: name.to_owned(),
                load: "loaded".into(),
                active: active.into(),
                sub,
                description: display.trim().to_owned(),
                file_state: mode_word(mode),
            })
        })
        .collect()
}

fn failed(what: &str, o: &super::Output) -> SystemError {
    let t = o.error_text();
    SystemError::other(if t.is_empty() { format!("{what} failed (exit {})", o.code) } else { t })
}

pub async fn list(conn: &Connection) -> SystemResult<Vec<Unit>> {
    let o = run(conn, LIST).await.map_err(SystemError::other)?;
    if o.code != 0 && o.stdout.trim().is_empty() {
        return Err(failed("Listing services", &o));
    }
    Ok(parse_list(&o.stdout))
}

/// Status text for one service: what it runs, as whom, and what depends on it.
pub fn status_script(name: &str) -> String {
    let q = quote(name);
    format!(
        r#"$ErrorActionPreference = 'Stop'
$s = Get-CimInstance Win32_Service -Filter ("Name='" + {q}.Replace("'", "''") + "'")
if (-not $s) {{ [Console]::Error.WriteLine('No service is named ' + {q} + '.'); exit 4 }}
$g = Get-Service -Name $s.Name
"{{0}} ({{1}})" -f $s.DisplayName, $s.Name
"  State:       {{0}}" -f $s.State
"  Startup:     {{0}}{{1}}" -f $s.StartMode, $(if ($s.DelayedAutoStart) {{ ' (delayed)' }} else {{ '' }})
"  Process id:  {{0}}" -f $s.ProcessId
"  Runs as:     {{0}}" -f $s.StartName
"  Command:     {{0}}" -f $s.PathName
"  Exit code:   {{0}}" -f $s.ExitCode
if ($g.ServicesDependedOn.Count) {{ "  Needs:       " + (($g.ServicesDependedOn | ForEach-Object {{ $_.Name }}) -join ', ') }}
if ($g.DependentServices.Count) {{ "  Needed by:   " + (($g.DependentServices | ForEach-Object {{ $_.Name }}) -join ', ') }}
if ($s.Description) {{ ''; $s.Description }}"#
    )
}

pub async fn status(conn: &Connection, name: &str) -> SystemResult<String> {
    if !valid_name(name) {
        return Err(SystemError::other(format!("invalid service name: {name:?}")));
    }
    let o = run(conn, &status_script(name)).await.map_err(SystemError::other)?;
    if o.code != 0 {
        return Err(failed("Reading the service", &o));
    }
    Ok(o.stdout.trim_end().to_owned())
}

/// The PowerShell for `action`, or why Windows has no such action.
pub fn action_script(name: &str, action: UnitAction) -> SystemResult<String> {
    let q = quote(name);
    let body = match action {
        UnitAction::Start => format!("Start-Service -Name {q}"),
        UnitAction::Stop => format!("Stop-Service -Name {q}"),
        UnitAction::Restart => format!("Restart-Service -Name {q}"),
        UnitAction::Enable => format!("Set-Service -Name {q} -StartupType Automatic"),
        UnitAction::Disable => format!("Set-Service -Name {q} -StartupType Disabled"),
        UnitAction::Reload => {
            return Err(SystemError::other(
                "Windows services can't reload their settings in place. Use Restart instead.",
            ))
        }
    };
    Ok(format!(
        "$ErrorActionPreference = 'Stop'\nif (-not ({IS_ADMIN})) {{ [Console]::Error.WriteLine('{NOT_ADMIN}'); exit 5 }}\n{body}"
    ))
}

/// Shown when the account can't change services.
pub const NOT_ADMIN: &str = "Changing Windows services needs an administrator account. Log in to this host as an administrator (Windows has no sudo).";

pub async fn act(conn: &Connection, name: &str, action: UnitAction) -> SystemResult<()> {
    if !valid_name(name) {
        return Err(SystemError::other(format!("invalid service name: {name:?}")));
    }
    let o = run(conn, &action_script(name, action)?).await.map_err(SystemError::other)?;
    if o.code == 0 {
        Ok(())
    } else {
        Err(failed(&format!("{} {name}", action.verb()), &o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows() {
        let u = parse_list(
            "Spooler\tRunning\tAuto\t0\tPrint Spooler\r\n\
             wuauserv\tStopped\tManual\t0\tWindows Update\n\
             Bad\tStopped\tAuto\t1066\tBroken thing\n\
             Fresh\tStopped\tAuto\t1077\tNever started\n\
             Off\tStopped\tDisabled\t0\t\n",
        );
        assert_eq!(u.len(), 5);
        assert_eq!(
            (u[0].active.as_str(), u[0].sub.as_str(), u[0].file_state.as_deref()),
            ("active", "running", Some("enabled"))
        );
        assert_eq!((u[1].active.as_str(), u[1].file_state.as_deref()), ("inactive", Some("manual")));
        assert_eq!(u[2].active, "failed");
        assert_eq!(u[3].active, "inactive");
        assert_eq!(u[4].file_state.as_deref(), Some("disabled"));
        assert_eq!(u[0].description, "Print Spooler");
    }

    #[test]
    fn names() {
        assert!(valid_name("Spooler"));
        assert!(valid_name("MSSQL$SQLEXPRESS"));
        assert!(valid_name("Some Service_1.2"));
        for bad in ["", "-x", "a*", "a?b", "a/b", "a\\b", "a'b", "a\nb"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn actions() {
        assert!(action_script("x", UnitAction::Reload).is_err());
        let s = action_script("Spooler", UnitAction::Disable).unwrap();
        assert!(s.contains("Set-Service -Name 'Spooler' -StartupType Disabled"));
        assert!(s.contains("IsInRole"));
    }

    #[cfg(windows)]
    #[test]
    fn lists_and_describes_locally() {
        let o = super::super::run_local(LIST, b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        let u = parse_list(&o.stdout);
        assert!(
            u.iter().any(|s| s.name.eq_ignore_ascii_case("EventLog") && s.active == "active"),
            "{:?}",
            &u[..3.min(u.len())]
        );
        let o = super::super::run_local(&status_script("EventLog"), b"").unwrap();
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stdout.contains("State:"), "{}", o.stdout);
        let o = super::super::run_local(&status_script("NoSuchServiceHere"), b"").unwrap();
        assert_eq!(o.code, 4, "{}", o.stdout);
    }
}
