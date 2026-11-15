//! systemd service units: list, status and actions.

use serde::{Deserialize, Serialize};

use super::sudo::{self, sh, SudoAuth};
use super::{sections, SystemError, SystemResult, SECTION};
use crate::files::path::shell_quote;
use crate::ssh::client::Connection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    /// Full name, e.g. `nginx.service`.
    pub name: String,
    /// `loaded`, `not-found`, `masked`, ...
    pub load: String,
    /// `active`, `inactive`, `failed`, `activating`, ...
    pub active: String,
    /// `running`, `exited`, `dead`, ...
    pub sub: String,
    pub description: String,
    /// Unit file state: `enabled`, `disabled`, `static`, `masked`, ... (None
    /// for units without a file, e.g. transient ones).
    pub file_state: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UnitAction {
    Start,
    Stop,
    Restart,
    Reload,
    Enable,
    Disable,
}

impl UnitAction {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Reload => "reload",
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }

    /// Interrupts what the unit is doing (needs a named confirmation).
    pub fn is_destructive(self) -> bool {
        matches!(self, Self::Stop | Self::Restart | Self::Disable)
    }
}

/// Unit names systemd accepts (letters, digits, `:-_.\@`), with a known
/// suffix. Rejects anything a shell or systemctl could read as an option.
pub fn valid_unit(name: &str) -> bool {
    const SUFFIXES: [&str; 6] = [".service", ".socket", ".timer", ".target", ".mount", ".path"];
    !name.is_empty()
        && name.len() <= 256
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || ":-_.\\@".contains(c))
        && SUFFIXES.iter().any(|s| name.ends_with(s) && name.len() > s.len())
}

/// Lists service units and their unit file states in one round trip.
pub fn list_command() -> String {
    sh(&format!(
        "command -v systemctl >/dev/null 2>&1 || {{ echo nosystemd; exit 0; }}; \
         systemctl list-units --type=service --all --no-legend --no-pager --plain 2>/dev/null; echo {SECTION}; \
         systemctl list-unit-files --type=service --no-legend --no-pager 2>/dev/null"
    ))
}

/// Parses `systemctl list-units --plain --no-legend` lines.
fn parse_units(text: &str) -> Vec<Unit> {
    text.lines()
        .filter_map(|line| {
            // Older systemd still prints the bullet for failed units.
            let line = line.trim_start_matches(['●', '*', ' ']).trim_end();
            let mut it = line.split_whitespace();
            let name = it.next()?;
            let (load, active, sub) = (it.next()?, it.next()?, it.next()?);
            if !name.contains('.') {
                return None;
            }
            let description = it.collect::<Vec<_>>().join(" ");
            Some(Unit {
                name: name.to_owned(),
                load: load.to_owned(),
                active: active.to_owned(),
                sub: sub.to_owned(),
                description,
                file_state: None,
            })
        })
        .collect()
}

/// Parses `systemctl list-unit-files --no-legend` lines: `name state [preset]`.
fn parse_unit_files(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.to_owned(), it.next()?.to_owned()))
        })
        .filter(|(n, _)| n.contains('.'))
        .collect()
}

/// Merges both lists. Unit files that aren't loaded (disabled, never
/// started) are listed as inactive so they can be started or enabled.
/// Templates (`foo@.service`) are left out: they can't be started as-is.
pub fn parse_list(out: &str) -> SystemResult<Vec<Unit>> {
    if out.trim() == "nosystemd" {
        return Err(SystemError::Missing { tool: "systemctl".into(), hint: "This host doesn't use systemd.".into() });
    }
    let parts = sections(out);
    let mut units = parse_units(parts.first().copied().unwrap_or(""));
    let files = parse_unit_files(parts.get(1).copied().unwrap_or(""));
    for (name, state) in files {
        if name.ends_with("@.service") {
            continue;
        }
        if let Some(u) = units.iter_mut().find(|u| u.name == name) {
            u.file_state = Some(state);
        } else {
            units.push(Unit {
                name,
                load: if state == "masked" { "masked".into() } else { "loaded".into() },
                active: "inactive".into(),
                sub: "dead".into(),
                description: String::new(),
                file_state: Some(state),
            });
        }
    }
    units.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(units)
}

pub async fn list(conn: &Connection) -> SystemResult<Vec<Unit>> {
    let (_, out, _) = conn.exec(&list_command()).await.map_err(SystemError::other)?;
    parse_list(&String::from_utf8_lossy(&out))
}

/// `systemctl status` text for one unit (no journal lines; the journal
/// viewer shows those).
pub async fn status(conn: &Connection, unit: &str) -> SystemResult<String> {
    if !valid_unit(unit) {
        return Err(SystemError::other(format!("invalid unit name: {unit:?}")));
    }
    let cmd = sh(&format!("SYSTEMD_COLORS=0 systemctl status --no-pager --full -n 0 -- {} 2>&1", shell_quote(unit)));
    let (_, out, _) = conn.exec(&cmd).await.map_err(SystemError::other)?;
    Ok(crate::docker::strip_ansi(String::from_utf8_lossy(&out).trim_end()))
}

pub fn action_command(unit: &str, action: UnitAction) -> String {
    format!("systemctl {} -- {}", action.verb(), shell_quote(unit))
}

/// Runs a unit action as root. The caller has confirmed destructive ones
/// with the unit and host named (security rule 5).
pub async fn act(conn: &Connection, auth: &SudoAuth, unit: &str, action: UnitAction) -> SystemResult<()> {
    if !valid_unit(unit) {
        return Err(SystemError::other(format!("invalid unit name: {unit:?}")));
    }
    let o = sudo::run(conn, auth, &format!("{} 2>&1", action_command(unit, action))).await?;
    if o.code == 0 {
        Ok(())
    } else {
        let msg = format!("{}\n{}", o.stdout.trim(), o.stderr.trim());
        let msg = sudo::clean_stderr(&msg);
        Err(SystemError::other(if msg.is_empty() {
            format!("systemctl {} failed ({})", action.verb(), o.code)
        } else {
            msg
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNITS: &str = "\
cron.service                         loaded    active   running Regular background program processing daemon
docker.service                       loaded    active   running Docker Application Container Engine
● nginx.service                        loaded    failed   failed  A high performance web server
ssh.service                          loaded    active   running OpenBSD Secure Shell server
systemd-fsck@dev-sda1.service        loaded    active   exited  File System Check on /dev/sda1
ufw.service                          not-found inactive dead    ufw.service
";

    const FILES: &str = "\
cron.service                           enabled         enabled
docker.service                         enabled         enabled
getty@.service                         enabled         enabled
nginx.service                          enabled         enabled
rsync.service                          disabled        enabled
ssh.service                            enabled         enabled
systemd-fsck@.service                  static          -
plymouth.service                       masked          enabled
";

    #[test]
    fn merges_units_and_files() {
        let out = format!("{UNITS}{SECTION}\n{FILES}");
        let u = parse_list(&out).unwrap();
        let names: Vec<_> = u.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "cron.service",
                "docker.service",
                "nginx.service",
                "plymouth.service",
                "rsync.service",
                "ssh.service",
                "systemd-fsck@dev-sda1.service",
                "ufw.service"
            ]
        );
        let nginx = u.iter().find(|u| u.name == "nginx.service").unwrap();
        assert_eq!((nginx.active.as_str(), nginx.sub.as_str()), ("failed", "failed"));
        assert_eq!(nginx.description, "A high performance web server");
        assert_eq!(nginx.file_state.as_deref(), Some("enabled"));
        let rsync = u.iter().find(|u| u.name == "rsync.service").unwrap();
        assert_eq!((rsync.active.as_str(), rsync.file_state.as_deref()), ("inactive", Some("disabled")));
        assert_eq!(u.iter().find(|u| u.name == "plymouth.service").unwrap().load, "masked");
        let ufw = u.iter().find(|u| u.name == "ufw.service").unwrap();
        assert_eq!((ufw.load.as_str(), ufw.file_state.as_deref()), ("not-found", None));
    }

    #[test]
    fn no_systemd() {
        assert!(matches!(parse_list("nosystemd\n"), Err(SystemError::Missing { .. })));
    }

    #[test]
    fn unit_names() {
        for ok in ["nginx.service", "systemd-fsck@dev-sda1.service", "getty@tty1.service", "a:b.timer", r"x\x2d.mount"]
        {
            assert!(valid_unit(ok), "{ok}");
        }
        for bad in ["", "-x.service", "nginx", ".service", "a b.service", "a;rm.service", "$(x).service", "a'.service"]
        {
            assert!(!valid_unit(bad), "{bad}");
        }
    }

    #[test]
    fn actions() {
        assert_eq!(action_command("nginx.service", UnitAction::Restart), "systemctl restart -- 'nginx.service'");
        assert!(UnitAction::Stop.is_destructive() && UnitAction::Disable.is_destructive());
        assert!(!UnitAction::Start.is_destructive() && !UnitAction::Enable.is_destructive());
    }
}
