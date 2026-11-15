//! Package updates: count what's upgradable (apt, dnf, pacman), spot a
//! pending reboot and build the upgrade command.

use serde::{Deserialize, Serialize};

use super::sudo::sh;
use super::{sections, SystemError, SystemResult, SECTION};
use crate::ssh::client::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Manager {
    Apt,
    Dnf,
    Pacman,
    /// Image-based OS (Fedora Atomic, Bazzite, ...): updated with
    /// rpm-ostree/bootc, not package by package. Listed, not upgraded here.
    Ostree,
}

impl Manager {
    pub fn label(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Ostree => "rpm-ostree",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PackageUpdate {
    pub name: String,
    pub current: Option<String>,
    pub available: String,
    /// apt suite / dnf repository.
    pub repo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdatesReport {
    /// `None` when no supported package manager was found.
    pub manager: Option<Manager>,
    pub updates: Vec<PackageUpdate>,
    pub reboot_required: bool,
    /// Why a reboot is needed (packages from `/var/run/reboot-required.pkgs`,
    /// or "running kernel was removed", ...).
    pub reboot_reasons: Vec<String>,
    /// Something the user should know (stale package lists, ...).
    pub note: Option<String>,
}

/// One round trip: which manager, what's upgradable and whether a reboot is
/// pending.
pub fn check_command() -> String {
    sh(&format!(
        "if [ -e /run/ostree-booted ]; then echo '@mgr ostree'; \
         elif command -v apt-get >/dev/null 2>&1; then echo '@mgr apt'; \
           s=$(stat -c %Y /var/cache/apt/pkgcache.bin 2>/dev/null || stat -c %Y /var/lib/apt/lists 2>/dev/null || echo 0); echo \"@age $(( $(date +%s) - s ))\"; \
           LANG=C apt list --upgradable 2>/dev/null; \
         elif command -v dnf >/dev/null 2>&1; then echo '@mgr dnf'; LANG=C dnf -q check-update 2>/dev/null; \
         elif command -v pacman >/dev/null 2>&1; then echo '@mgr pacman'; \
           if command -v checkupdates >/dev/null 2>&1; then checkupdates 2>/dev/null; else echo '@stale'; pacman -Qu 2>/dev/null; fi; \
         else echo '@mgr none'; fi; \
         echo {SECTION}; \
         if [ -f /var/run/reboot-required ]; then echo '@flag'; cat /var/run/reboot-required.pkgs 2>/dev/null; fi; \
         k=$(uname -r); [ -d /lib/modules/$k ] || [ -d /usr/lib/modules/$k ] || echo '@kernel'; \
         if command -v needs-restarting >/dev/null 2>&1; then needs-restarting -r >/dev/null 2>&1; [ $? -eq 1 ] && echo '@needs-restarting'; fi; \
         true"
    ))
}

fn parse_apt(text: &str) -> Vec<PackageUpdate> {
    // bash/jammy-updates 5.1-6ubuntu1.1 amd64 [upgradable from: 5.1-6ubuntu1]
    text.lines()
        .filter(|l| l.contains("[upgradable from:"))
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let (name, repo) = it.next()?.split_once('/')?;
            let available = it.next()?.to_owned();
            let current = l.split("[upgradable from:").nth(1).map(|s| s.trim().trim_end_matches(']').trim().to_owned());
            Some(PackageUpdate { name: name.to_owned(), current, available, repo: Some(repo.to_owned()) })
        })
        .collect()
}

fn parse_dnf(text: &str) -> Vec<PackageUpdate> {
    // name.arch   version   repo   (dnf4 wraps a long name onto its own line)
    let mut out = Vec::new();
    let mut carry: Option<String> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if l.starts_with("Obsoleting") || l.starts_with("Security:") {
            break;
        }
        let mut toks: Vec<&str> = l.split_whitespace().collect();
        let joined;
        if let Some(c) = carry.take() {
            joined = format!("{c} {l}");
            toks = joined.split_whitespace().collect();
        }
        match toks.len() {
            1 if toks[0].contains('.') => carry = Some(toks[0].to_owned()),
            3 if toks[0].contains('.') => {
                let name = toks[0].rsplit_once('.').map_or(toks[0], |(n, _)| n);
                out.push(PackageUpdate {
                    name: name.to_owned(),
                    current: None,
                    available: toks[1].to_owned(),
                    repo: Some(toks[2].to_owned()),
                });
            }
            _ => {}
        }
    }
    out
}

fn parse_pacman(text: &str) -> Vec<PackageUpdate> {
    // linux 6.11.5.arch1-1 -> 6.11.6.arch1-1   (pacman -Qu may append [ignored])
    text.lines()
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.len() >= 4 && t[2] == "->" && !l.contains("[ignored]") {
                Some(PackageUpdate {
                    name: t[0].into(),
                    current: Some(t[1].into()),
                    available: t[3].into(),
                    repo: None,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Package lists older than this get a "refresh first" note (apt).
const STALE_SECS: i64 = 2 * 24 * 3600;

pub fn parse_check(out: &str) -> UpdatesReport {
    let parts = sections(out);
    let list = parts.first().copied().unwrap_or("");
    let reboot = parts.get(1).copied().unwrap_or("");
    let mgr_line = list.lines().find_map(|l| l.strip_prefix("@mgr ")).unwrap_or("none").trim();
    let manager = match mgr_line {
        "apt" => Some(Manager::Apt),
        "dnf" => Some(Manager::Dnf),
        "pacman" => Some(Manager::Pacman),
        "ostree" => Some(Manager::Ostree),
        _ => None,
    };
    let body: String = list.lines().filter(|l| !l.starts_with('@')).collect::<Vec<_>>().join("\n");
    let updates = match manager {
        Some(Manager::Apt) => parse_apt(&body),
        Some(Manager::Dnf) => parse_dnf(&body),
        Some(Manager::Pacman) => parse_pacman(&body),
        _ => Vec::new(),
    };
    let note = match manager {
        Some(Manager::Apt) => list
            .lines()
            .find_map(|l| l.strip_prefix("@age "))
            .and_then(|a| a.trim().parse::<i64>().ok())
            .filter(|a| *a > STALE_SECS)
            .map(|a| format!("Package lists are {} days old: refresh for an accurate count.", a / 86_400)),
        Some(Manager::Pacman) if list.lines().any(|l| l == "@stale") => {
            Some("checkupdates (pacman-contrib) isn't installed, so this uses the last synced database.".into())
        }
        Some(Manager::Ostree) => Some("Image-based system: update it with rpm-ostree or bootc.".into()),
        None => Some("No supported package manager (apt, dnf, pacman) found.".into()),
        _ => None,
    };
    let mut reasons = Vec::new();
    let mut lines = reboot.lines().map(str::trim).filter(|l| !l.is_empty()).peekable();
    while let Some(l) = lines.next() {
        match l {
            "@flag" => {
                let pkgs: Vec<&str> = std::iter::from_fn(|| lines.next_if(|n| !n.starts_with('@'))).collect();
                if pkgs.is_empty() {
                    reasons.push("the system asks for a reboot".to_owned());
                } else {
                    let mut pkgs = pkgs;
                    pkgs.dedup();
                    reasons.push(format!("updated: {}", pkgs.join(", ")));
                }
            }
            "@kernel" => reasons.push("the running kernel was replaced".to_owned()),
            "@needs-restarting" => reasons.push("core libraries or the kernel were updated".to_owned()),
            _ => {}
        }
    }
    UpdatesReport { manager, updates, reboot_required: !reasons.is_empty(), reboot_reasons: reasons, note }
}

pub async fn check(conn: &Connection) -> SystemResult<UpdatesReport> {
    let (_, out, _) = conn.exec(&check_command()).await.map_err(SystemError::other)?;
    Ok(parse_check(&String::from_utf8_lossy(&out)))
}

/// What an upgrade run does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UpgradeOp {
    /// Refresh the package lists only (apt).
    Refresh,
    /// Install all available upgrades.
    Upgrade,
}

/// The root command for `op` (runs non-interactively; answers default to
/// keeping existing config files).
pub fn upgrade_command(manager: Manager, op: UpgradeOp) -> SystemResult<String> {
    const APT_OPTS: &str = "-o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold";
    let cmd = match (manager, op) {
        (Manager::Apt, UpgradeOp::Refresh) => "LANG=C apt-get update 2>&1".to_owned(),
        (Manager::Apt, UpgradeOp::Upgrade) => format!(
            "export DEBIAN_FRONTEND=noninteractive LANG=C; apt-get update 2>&1 && apt-get -y {APT_OPTS} --with-new-pkgs upgrade 2>&1"
        ),
        (Manager::Dnf, UpgradeOp::Upgrade) => "LANG=C dnf -y upgrade --refresh 2>&1".to_owned(),
        (Manager::Pacman, UpgradeOp::Upgrade) => "LANG=C pacman -Syu --noconfirm 2>&1".to_owned(),
        (Manager::Ostree, _) => {
            return Err(SystemError::other("image-based systems are updated with rpm-ostree or bootc, not from here"))
        }
        (m, UpgradeOp::Refresh) => {
            return Err(SystemError::other(format!(
                "{} refreshes its lists while checking; no separate refresh needed",
                m.label()
            )))
        }
    };
    Ok(cmd)
}

pub fn reboot_command() -> &'static str {
    "systemctl reboot 2>&1 || shutdown -r now 2>&1"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(list: &str, reboot: &str) -> UpdatesReport {
        parse_check(&format!("{list}{SECTION}\n{reboot}"))
    }

    #[test]
    fn apt() {
        let r = report(
            "@mgr apt\n@age 600\nListing...\n\
             bash/jammy-updates 5.1-6ubuntu1.1 amd64 [upgradable from: 5.1-6ubuntu1]\n\
             openssl/jammy-security,jammy-updates 3.0.2-0ubuntu1.18 amd64 [upgradable from: 3.0.2-0ubuntu1.17]\n",
            "@flag\nlinux-image-6.8.0-49-generic\nlinux-base\nlinux-base\n",
        );
        assert_eq!(r.manager, Some(Manager::Apt));
        assert_eq!(r.updates.len(), 2);
        assert_eq!(
            r.updates[1],
            PackageUpdate {
                name: "openssl".into(),
                current: Some("3.0.2-0ubuntu1.17".into()),
                available: "3.0.2-0ubuntu1.18".into(),
                repo: Some("jammy-security,jammy-updates".into())
            }
        );
        assert!(r.reboot_required);
        assert_eq!(r.reboot_reasons, ["updated: linux-image-6.8.0-49-generic, linux-base"]);
        assert_eq!(r.note, None);
    }

    #[test]
    fn apt_stale_lists() {
        let r = report("@mgr apt\n@age 864000\nListing...\n", "");
        assert!(r.updates.is_empty() && !r.reboot_required);
        assert_eq!(r.note.as_deref(), Some("Package lists are 10 days old: refresh for an accurate count."));
    }

    #[test]
    fn dnf() {
        let r = report(
            "@mgr dnf\n\n\
             kernel.x86_64                     6.11.6-200.fc40          updates\n\
             python3-very-long-package-name-that-wraps.noarch\n\
             \x20                                 1.2.3-1.fc40             updates\n\
             vim-minimal.x86_64                2:9.1.785-1.fc40         updates\n\
             Obsoleting Packages\n\
             grub2-tools.x86_64                1:2.06-1.fc40            updates\n",
            "@kernel\n@needs-restarting\n",
        );
        let names: Vec<_> = r.updates.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["kernel", "python3-very-long-package-name-that-wraps", "vim-minimal"]);
        assert_eq!(r.updates[2].available, "2:9.1.785-1.fc40");
        assert_eq!(r.reboot_reasons.len(), 2);
    }

    #[test]
    fn pacman() {
        let r = report("@mgr pacman\nlinux 6.11.5.arch1-1 -> 6.11.6.arch1-1\nfoo 1-1 -> 2-1 [ignored]\n", "");
        assert_eq!(r.updates.len(), 1);
        assert_eq!(r.updates[0].current.as_deref(), Some("6.11.5.arch1-1"));
        assert_eq!(r.note, None);
        let r = report("@mgr pacman\n@stale\n", "");
        assert!(r.note.unwrap().contains("checkupdates"));
    }

    #[test]
    fn unsupported() {
        let r = report("@mgr none\n", "");
        assert_eq!(r.manager, None);
        assert!(r.note.is_some());
        let r = report("@mgr ostree\n", "@flag\n");
        assert_eq!(r.manager, Some(Manager::Ostree));
        assert_eq!(r.reboot_reasons, ["the system asks for a reboot"]);
        assert!(upgrade_command(Manager::Ostree, UpgradeOp::Upgrade).is_err());
    }

    #[test]
    fn commands() {
        assert!(upgrade_command(Manager::Apt, UpgradeOp::Upgrade).unwrap().contains("--force-confold"));
        assert!(upgrade_command(Manager::Dnf, UpgradeOp::Refresh).is_err());
        assert_eq!(
            upgrade_command(Manager::Pacman, UpgradeOp::Upgrade).unwrap(),
            "LANG=C pacman -Syu --noconfirm 2>&1"
        );
    }
}
