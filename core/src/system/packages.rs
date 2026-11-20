//! Package updates: count what's upgradable (apt, dnf, pacman), spot a
//! pending reboot and build the upgrade command. Packages held on the host
//! (apt-mark hold, dnf versionlock, pacman IgnorePkg) are listed and marked,
//! and can be held or released (D-195).

use serde::{Deserialize, Serialize};

use super::sudo::sh;
use super::{sections, SystemError, SystemResult, SECTION};
use crate::files::path::shell_quote;
use crate::holds::valid_package;
use crate::ssh::client::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Manager {
    Apt,
    Dnf,
    Pacman,
    /// Image-based OS (Fedora Atomic, Bazzite, ...): updated as one image
    /// with rpm-ostree, which stages it for the next boot.
    Ostree,
    /// Windows Update (Windows hosts).
    WindowsUpdate,
}

impl Manager {
    pub fn label(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Ostree => "rpm-ostree",
            Self::WindowsUpdate => "Windows Update",
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
    /// Held: on the host (apt-mark hold, IgnorePkg, a hidden Windows
    /// update) or in Nightgrid. Upgrades leave it alone; it isn't pending.
    pub held: bool,
}

/// How this host can hold a package back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum HoldSupport {
    /// The package manager holds it (apt-mark hold, dnf versionlock, hiding a
    /// Windows update), so every tool on the host respects it.
    Host,
    /// Only Nightgrid's upgrades leave it out (pacman, dnf without the
    /// versionlock plugin).
    Nightgrid,
    /// Packages can't be held one by one (image-based systems, no manager):
    /// hold the whole host instead.
    None,
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
    /// Packages held on the host (some aren't in `updates`: dnf hides
    /// locked ones from its list).
    pub held: Vec<String>,
    pub hold_support: HoldSupport,
}

impl UpdatesReport {
    /// Marks the rows named in `nightgrid_holds` (held in Nightgrid) or held
    /// on the host as held, and lists them in `held`.
    pub fn apply_holds(&mut self, nightgrid_holds: &[String]) {
        for u in &mut self.updates {
            if self.held.contains(&u.name) || nightgrid_holds.contains(&u.name) {
                u.held = true;
            }
        }
        for n in nightgrid_holds {
            if !self.held.contains(n) {
                self.held.push(n.clone());
            }
        }
    }

    /// Updates that aren't held.
    pub fn pending(&self) -> usize {
        self.updates.iter().filter(|u| !u.held).count()
    }
}

/// One round trip: which manager, what's upgradable and whether a reboot is
/// pending.
pub fn check_command() -> String {
    sh(&format!(
        "if [ -e /run/ostree-booted ]; then echo '@mgr ostree'; \
           if command -v rpm-ostree >/dev/null 2>&1; then \
             LANG=C rpm-ostree upgrade --check 2>&1 | sed 's/^/@os /'; \
             printf '@json '; rpm-ostree status --json 2>/dev/null | tr -d '\\n'; echo; \
           else echo '@os-missing'; fi; \
         elif command -v apt-get >/dev/null 2>&1; then echo '@mgr apt'; \
           s=$(stat -c %Y /var/cache/apt/pkgcache.bin 2>/dev/null || stat -c %Y /var/lib/apt/lists 2>/dev/null || echo 0); echo \"@age $(( $(date +%s) - s ))\"; \
           LANG=C apt list --upgradable 2>/dev/null; apt-mark showhold 2>/dev/null | sed 's/^/@held /'; \
         elif command -v dnf >/dev/null 2>&1; then echo '@mgr dnf'; LANG=C dnf -q check-update 2>/dev/null; \
           if dnf versionlock --help >/dev/null 2>&1; then echo '@vl'; LANG=C dnf -q versionlock list 2>/dev/null | sed 's/^/@lock /'; fi; \
         elif command -v pacman >/dev/null 2>&1; then echo '@mgr pacman'; \
           if command -v checkupdates >/dev/null 2>&1; then checkupdates 2>/dev/null; else echo '@stale'; pacman -Qu 2>/dev/null; fi; \
           sed -n 's/^[[:space:]]*IgnorePkg[[:space:]]*=//p' /etc/pacman.conf 2>/dev/null | tr ' \t' '\n\n' | sed '/^$/d;s/^/@held /'; \
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
            Some(PackageUpdate { name: name.to_owned(), current, available, repo: Some(repo.to_owned()), held: false })
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
                    held: false,
                });
            }
            _ => {}
        }
    }
    out
}

fn parse_pacman(text: &str) -> Vec<PackageUpdate> {
    // linux 6.11.5.arch1-1 -> 6.11.6.arch1-1   (pacman -Qu appends [ignored] for IgnorePkg)
    text.lines()
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.len() >= 4 && t[2] == "->" {
                Some(PackageUpdate {
                    name: t[0].into(),
                    current: Some(t[1].into()),
                    available: t[3].into(),
                    repo: None,
                    held: l.contains("[ignored]"),
                })
            } else {
                None
            }
        })
        .collect()
}

/// What `rpm-ostree upgrade --check` and `rpm-ostree status --json` say:
/// the new image (as one update row), whether one is already staged, and
/// a note when the check failed.
fn parse_ostree(list: &str) -> (Vec<PackageUpdate>, bool, Option<String>) {
    if list.lines().any(|l| l == "@os-missing") {
        return (Vec::new(), false, Some("Image-based system without rpm-ostree: update it with bootc.".into()));
    }
    let check: Vec<&str> = list.lines().filter_map(|l| l.strip_prefix("@os ")).map(str::trim).collect();
    let status: serde_json::Value = list
        .lines()
        .find_map(|l| l.strip_prefix("@json "))
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();
    let deployments = status["deployments"].as_array().cloned().unwrap_or_default();
    let version = |d: &serde_json::Value| {
        d["version"].as_str().or_else(|| d["checksum"].as_str().map(|c| &c[..c.len().min(12)])).map(str::to_owned)
    };
    let booted = deployments.iter().find(|d| d["booted"].as_bool() == Some(true));
    // The first deployment is the one the next boot uses.
    let staged = deployments.first().is_some_and(|d| d["booted"].as_bool() == Some(false));
    let mut updates = Vec::new();
    let mut note = None;
    if let Some(i) = check.iter().position(|l| l.starts_with("AvailableUpdate")) {
        let field = |k: &str| check[i + 1..].iter().find_map(|l| l.strip_prefix(k)).map(|v| v.trim().to_owned());
        let available = field("Version:")
            .map(|v| v.split(" (").next().unwrap_or(&v).to_owned())
            .or_else(|| {
                field("Digest:")
                    .or_else(|| field("Commit:"))
                    .map(|c| c.trim_start_matches("sha256:").chars().take(12).collect())
            })
            .unwrap_or_else(|| "newer image".into());
        // `ostree-image-signed:docker://ghcr.io/org/image:tag` → `ghcr.io/org/image:tag`.
        let repo = booted
            .and_then(|d| d["container-image-reference"].as_str().or_else(|| d["origin"].as_str()))
            .map(|r| r.rsplit("//").next().unwrap_or(r).to_owned());
        updates.push(PackageUpdate {
            name: "system image".into(),
            current: booted.and_then(version),
            available,
            repo,
            held: false,
        });
    } else if !check.iter().any(|l| l.contains("No updates available") || l.contains("No upgrade available")) {
        let why = check.iter().rev().find(|l| l.starts_with("error") || l.contains("rror:")).or(check.last());
        note = Some(match why {
            Some(w) => format!("Couldn't check for a new image: {w}"),
            None => "Couldn't check for a new image.".into(),
        });
    }
    if note.is_none() {
        note = Some(if staged {
            "A new image is staged: it applies on the next reboot.".into()
        } else {
            "Image-based system: Upgrade stages the new image for the next reboot.".into()
        });
    }
    (updates, staged, note)
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
    let (ostree_updates, staged, ostree_note) =
        if manager == Some(Manager::Ostree) { parse_ostree(list) } else { (Vec::new(), false, None) };
    let updates = match manager {
        Some(Manager::Apt) => parse_apt(&body),
        Some(Manager::Dnf) => parse_dnf(&body),
        Some(Manager::Pacman) => parse_pacman(&body),
        Some(Manager::Ostree) => ostree_updates,
        Some(Manager::WindowsUpdate) | None => Vec::new(),
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
        Some(Manager::Ostree) => ostree_note,
        None => Some("No supported package manager (apt, dnf, pacman) found.".into()),
        _ => None,
    };
    let held: Vec<String> = list
        .lines()
        .filter_map(|l| l.strip_prefix("@held ").map(str::trim).map(str::to_owned))
        .chain(list.lines().filter_map(|l| l.strip_prefix("@lock ")).filter_map(versionlock_name))
        .filter(|n| valid_package(n))
        .fold(Vec::new(), |mut v, n| {
            if !v.contains(&n) {
                v.push(n);
            }
            v
        });
    let hold_support = match manager {
        Some(Manager::Apt) => HoldSupport::Host,
        Some(Manager::Dnf) if list.lines().any(|l| l == "@vl") => HoldSupport::Host,
        Some(Manager::Dnf) | Some(Manager::Pacman) => HoldSupport::Nightgrid,
        Some(Manager::WindowsUpdate) => HoldSupport::Host,
        Some(Manager::Ostree) | None => HoldSupport::None,
    };
    let mut reasons = Vec::new();
    if staged {
        reasons.push("a new system image is staged".to_owned());
    }
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
    let mut report = UpdatesReport {
        manager,
        updates,
        reboot_required: !reasons.is_empty(),
        reboot_reasons: reasons,
        note,
        held,
        hold_support,
    };
    report.apply_holds(&[]);
    report
}

/// The package name in one `dnf versionlock list` line: dnf4 prints
/// `name-[epoch:]version-release.*` (`!` first for an exclude), dnf5 prints
/// `Package name: name` blocks.
fn versionlock_name(line: &str) -> Option<String> {
    let l = line.trim();
    if let Some(n) = l.strip_prefix("Package name:") {
        return Some(n.trim().to_owned());
    }
    if l.is_empty() || l.starts_with('#') || l.contains(' ') || l.contains('=') {
        return None;
    }
    let l = l.trim_start_matches('!').trim_end_matches(".*");
    // Drop "-version-release" (the last two dash-separated parts).
    let mut parts = l.rsplitn(3, '-');
    let (_rel, _ver, name) = (parts.next()?, parts.next()?, parts.next()?);
    Some(name.to_owned())
}

/// The root command that holds (or releases) packages on the host, where
/// the package manager can. Names are checked and quoted.
pub fn hold_command(manager: Manager, names: &[String], hold: bool) -> SystemResult<String> {
    if names.is_empty() {
        return Err(SystemError::other("no package named"));
    }
    if let Some(bad) = names.iter().find(|n| !valid_package(n)) {
        return Err(SystemError::other(format!("{bad:?} isn't a package name")));
    }
    let list: Vec<String> = names.iter().map(|n| shell_quote(n)).collect();
    let list = list.join(" ");
    Ok(match manager {
        Manager::Apt => format!("LANG=C apt-mark {} {list} 2>&1", if hold { "hold" } else { "unhold" }),
        Manager::Dnf => format!("LANG=C dnf -q versionlock {} {list} 2>&1", if hold { "add" } else { "delete" }),
        m => {
            return Err(SystemError::other(format!("{} can't hold single packages on the host", m.label())));
        }
    })
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
    upgrade_command_excluding(manager, op, &[])
}

/// [`upgrade_command`] leaving out packages Nightgrid holds (dnf `-x`,
/// pacman `--ignore`). apt and Windows hold on the host instead, so their
/// upgrades skip held packages by themselves.
pub fn upgrade_command_excluding(manager: Manager, op: UpgradeOp, exclude: &[String]) -> SystemResult<String> {
    const APT_OPTS: &str = "-o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold";
    let exclude: Vec<&String> = exclude.iter().filter(|n| valid_package(n)).collect();
    let dnf_x: String = exclude.iter().map(|n| format!(" -x {}", shell_quote(n))).collect();
    let pacman_ignore = if exclude.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = exclude.iter().map(|n| n.as_str()).collect();
        format!(" --ignore {}", shell_quote(&names.join(",")))
    };
    let cmd = match (manager, op) {
        (Manager::Apt, UpgradeOp::Refresh) => "LANG=C apt-get update 2>&1".to_owned(),
        (Manager::Apt, UpgradeOp::Upgrade) => format!(
            "export DEBIAN_FRONTEND=noninteractive LANG=C; apt-get update 2>&1 && apt-get -y {APT_OPTS} --with-new-pkgs upgrade 2>&1"
        ),
        (Manager::Dnf, UpgradeOp::Upgrade) => format!("LANG=C dnf -y upgrade --refresh{dnf_x} 2>&1"),
        (Manager::Pacman, UpgradeOp::Upgrade) => format!("LANG=C pacman -Syu --noconfirm{pacman_ignore} 2>&1"),
        (Manager::Ostree, UpgradeOp::Upgrade) => "LANG=C rpm-ostree upgrade 2>&1".to_owned(),
        (Manager::WindowsUpdate, UpgradeOp::Upgrade) => {
            return Err(SystemError::other("Windows Update runs through PowerShell (win::updates), not a shell command"))
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
                repo: Some("jammy-security,jammy-updates".into()),
                held: false,
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
        assert_eq!(r.updates.len(), 2);
        assert!(r.updates[1].held && !r.updates[0].held);
        assert_eq!(r.pending(), 1);
        assert_eq!(r.hold_support, HoldSupport::Nightgrid);
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
        let r = report("@mgr ostree\n@os-missing\n", "@flag\n");
        assert_eq!(r.manager, Some(Manager::Ostree));
        assert_eq!(r.reboot_reasons, ["the system asks for a reboot"]);
        assert!(r.note.unwrap().contains("bootc"));
        assert!(upgrade_command(Manager::Ostree, UpgradeOp::Refresh).is_err());
    }

    #[test]
    fn ostree() {
        let json = r#"{"deployments":[{"booted":true,"version":"41.20241101.0","container-image-reference":"ostree-image-signed:docker://ghcr.io/example/os:stable"}]}"#;
        let r = report(
            &format!(
                "@mgr ostree\n@os 1 metadata, 0 images\n@os AvailableUpdate:\n@os         Version: 41.20241110.0 (2024-11-10T04:00:00Z)\n@os          Digest: sha256:abcdef0123456789\n@json {json}\n"
            ),
            "",
        );
        assert_eq!(
            r.updates,
            [PackageUpdate {
                name: "system image".into(),
                current: Some("41.20241101.0".into()),
                available: "41.20241110.0".into(),
                repo: Some("ghcr.io/example/os:stable".into()),
                held: false,
            }]
        );
        assert!(!r.reboot_required);
        assert!(r.note.unwrap().contains("next reboot"));
        // Staged: the first deployment isn't the booted one.
        let json = r#"{"deployments":[{"booted":false,"version":"41.2"},{"booted":true,"version":"41.1"}]}"#;
        let r = report(&format!("@mgr ostree\n@os No updates available.\n@json {json}\n"), "");
        assert!(r.updates.is_empty());
        assert_eq!(r.reboot_reasons, ["a new system image is staged"]);
        // Check refused (polkit): the reason is kept.
        let r = report("@mgr ostree\n@os error: Authorization required\n@json {}\n", "");
        assert_eq!(r.note.as_deref(), Some("Couldn't check for a new image: error: Authorization required"));
        assert_eq!(upgrade_command(Manager::Ostree, UpgradeOp::Upgrade).unwrap(), "LANG=C rpm-ostree upgrade 2>&1");
    }

    #[test]
    fn holds() {
        // apt: held packages are still listed by apt, and marked.
        let r = report(
            "@mgr apt\n@age 60\nListing...\n\
             bash/jammy-updates 5.1-6ubuntu1.1 amd64 [upgradable from: 5.1-6ubuntu1]\n\
             docker-ce/jammy 5:27.3.1-1 amd64 [upgradable from: 5:27.2.0-1]\n\
             @held docker-ce\n@held containerd.io\n",
            "",
        );
        assert_eq!(r.held, ["docker-ce", "containerd.io"]);
        assert!(r.updates[1].held && !r.updates[0].held);
        assert_eq!(r.pending(), 1);
        assert_eq!(r.hold_support, HoldSupport::Host);
        // dnf4 and dnf5 versionlock lists.
        let r = report(
            "@mgr dnf\nvim-minimal.x86_64   2:9.1.785-1.fc40   updates\n@vl\n\
             @lock kernel-0:6.11.5-200.fc40.*\n@lock !podman-5:5.2.0-1.fc40.*\n\
             @lock # Added by 'versionlock add' command on 2024-11-10\n@lock Package name: mesa-dri-drivers\n@lock evr = 24.2.5-1.fc41\n",
            "",
        );
        assert_eq!(r.held, ["kernel", "podman", "mesa-dri-drivers"]);
        assert_eq!(r.hold_support, HoldSupport::Host);
        let r = report("@mgr dnf\n", "");
        assert_eq!(r.hold_support, HoldSupport::Nightgrid);
        // pacman IgnorePkg, then a Nightgrid hold on top.
        let mut r = report("@mgr pacman\nlinux 1-1 -> 2-1\nmesa 1-1 -> 2-1\n@held linux\n", "");
        assert!(r.updates[0].held);
        r.apply_holds(&["mesa".to_owned()]);
        assert!(r.updates[1].held);
        assert_eq!(r.held, ["linux", "mesa"]);
        assert_eq!(r.pending(), 0);
        assert_eq!(report("@mgr ostree\n@os-missing\n", "").hold_support, HoldSupport::None);
        // Commands.
        let names = vec!["docker-ce".to_owned(), "containerd.io".to_owned()];
        assert_eq!(
            hold_command(Manager::Apt, &names, true).unwrap(),
            "LANG=C apt-mark hold 'docker-ce' 'containerd.io' 2>&1"
        );
        assert_eq!(
            hold_command(Manager::Dnf, &names[..1], false).unwrap(),
            "LANG=C dnf -q versionlock delete 'docker-ce' 2>&1"
        );
        assert!(hold_command(Manager::Pacman, &names, true).is_err());
        assert!(hold_command(Manager::Apt, &["x; reboot".to_owned()], true).is_err());
        assert!(hold_command(Manager::Apt, &[], true).is_err());
        assert_eq!(
            upgrade_command_excluding(Manager::Dnf, UpgradeOp::Upgrade, &names).unwrap(),
            "LANG=C dnf -y upgrade --refresh -x 'docker-ce' -x 'containerd.io' 2>&1"
        );
        assert_eq!(
            upgrade_command_excluding(Manager::Pacman, UpgradeOp::Upgrade, &names).unwrap(),
            "LANG=C pacman -Syu --noconfirm --ignore 'docker-ce,containerd.io' 2>&1"
        );
        // Bad names never reach the command line.
        assert_eq!(
            upgrade_command_excluding(Manager::Dnf, UpgradeOp::Upgrade, &["-y; reboot".to_owned()]).unwrap(),
            "LANG=C dnf -y upgrade --refresh 2>&1"
        );
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
