//! What works on Windows hosts, feature by feature, and why the rest
//! doesn't. One table for the whole app: the UI shows these notes in place
//! of a view or tab, and commands for POSIX-only features fail early with
//! the same words instead of a shell error from cmd.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SupportLevel {
    /// Works the same as on Linux.
    Full,
    /// Works, with differences explained in `why`.
    Partial,
    /// Not yet: a Windows version is planned (`why` says what it will use).
    Planned,
    /// Can't work on Windows (`why` says why, and what to do instead).
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Support {
    /// Feature id (`system.units`, `docker`, ...).
    pub feature: &'static str,
    /// What the user calls it.
    pub label: &'static str,
    pub level: SupportLevel,
    /// One or two sentences for the user.
    pub why: &'static str,
}

const fn s(feature: &'static str, label: &'static str, level: SupportLevel, why: &'static str) -> Support {
    Support { feature, label, level, why }
}

use SupportLevel::{Full, None as No, Partial, Planned};

/// Every feature that runs something on the host, as it stands on Windows.
pub const WINDOWS: &[Support] = &[
    s("terminal", "Terminal", Full, "Opens the host's own shell (cmd or PowerShell, whichever sshd is set to)."),
    s(
        "terminal.hooks",
        "Key hints and nano opens the editor",
        No,
        "Both come from a small POSIX shell hook, and cmd and PowerShell can't run it. The programs the hints cover (nano, vim, less, htop) aren't part of Windows either.",
    ),
    s("files", "Files", Full, "Browses and edits files over SFTP. Paths look like /C:/Users/you."),
    s("metrics", "Overview, monitoring and history", Partial, "CPU, memory, page file, disks, network, uptime and NVIDIA GPUs come from Windows' performance counters. Windows has no load average, and temperatures show only where the firmware reports ACPI thermal zones (most servers and VMs don't)."),
    s("power", "Reboot and shut down", Full, "Uses Windows' own shutdown. Wake-on-LAN works as for any host."),
    s(
        "power.suspend",
        "Suspend",
        Partial,
        "Asks Windows to sleep. Windows Server and most VMs don't support sleep; Nightgrid says so when the host refuses.",
    ),
    s("keys", "SSH keys", Full, "Pushes keys the Windows way: to administrators_authorized_keys for administrator accounts (with the permissions sshd requires), else to the user's .ssh\\authorized_keys."),
    s("tunnels", "Tunnels and web UIs", Partial, "TCP forwards work as on Linux. Forwards to Unix sockets can't: Windows sshd doesn't support them."),
    s("automation", "Automation", Partial, "Commands run in the host's shell, so write them for cmd or PowerShell. The risky-command check only knows Linux commands."),
    s("system.units", "System → UNITS", Planned, "This tab reads systemd. A Windows version listing Windows services (start, stop, restart, startup type) is coming."),
    s("system.journal", "System → JOURNAL", Planned, "This tab reads journald. A Windows version reading the Event Log is coming."),
    s("system.schedules", "System → SCHEDULES", Planned, "This tab reads cron and systemd timers. A Windows version for Task Scheduler is coming."),
    s("system.config", "System → CONFIG", Planned, "Config drift reads files under /etc and Linux package lists. A Windows version comparing files you pick and installed programs is coming."),
    s("system.security", "System → SECURITY", Planned, "These checks read Linux settings (sshd, sudo, ufw). Windows checks (Defender, firewall profiles, RDP, SMBv1, updates, administrators) are coming."),
    s("system.firewall", "System → FIREWALL", Planned, "This tab edits ufw or firewalld. A Windows Defender Firewall version is coming."),
    s("system.updates", "System → UPDATES", Planned, "This tab runs apt, dnf or pacman. A Windows Update version is coming."),
    s("system.disks", "System → DISKS", Planned, "This tab reads SMART with smartctl and lsblk. A Windows version using Storage health (Get-PhysicalDisk) is coming."),
    s("system.vms", "System → VMS", Planned, "This tab reads libvirt. A Hyper-V version is coming."),
    s("power.schedules", "Power schedules", Planned, "Schedules are systemd timers on the host. A Task Scheduler version is coming."),
    s(
        "docker",
        "Docker",
        Partial,
        "Containers, images, logs, stats and events work through docker.exe on the host (Docker Engine or Docker Desktop). Container shells start sh, so they work in Linux containers only.",
    ),
    s(
        "docker.compose",
        "Compose stacks",
        Planned,
        "Stacks are listed from their containers, but starting, updating and stopping them runs docker compose through a POSIX shell. A PowerShell version is coming.",
    ),
    s("fleet.backups", "Fleet → BACKUPS", Planned, "Backup checks run restic, borg and rsync through a POSIX shell. Windows support is coming."),
    s("fleet.logs", "Fleet → LOGS", Planned, "Log search reads journald and /var/log. A Windows version searching the Event Log is coming."),
    s("fleet.access", "Fleet → ACCESS", Planned, "The access audit reads Linux login records. A Windows version reading logon events is coming."),
    s("fleet.speed", "Fleet → SPEED", Planned, "Speed tests start iperf3 through a POSIX shell. Windows support (with iperf3.exe) is coming."),
    s(
        "fleet.ups",
        "Fleet → UPS",
        No,
        "UPS support talks to Network UPS Tools (upsc) on the host. NUT doesn't run as a service on Windows: UPSes there use the vendor's software. Watch the UPS from a Linux host that NUT can reach.",
    ),
    s("security.posture", "Security posture", Planned, "Posture scores Linux settings. Windows scoring (Defender, firewall, RDP, SMBv1, updates) is coming."),
    s("network.scan", "Network scans from this host", Planned, "Scans use ip neigh, ping and nmap through a POSIX shell. A PowerShell version (Get-NetNeighbor, ping sweep) is coming."),
    s("nettools", "Network tools from this host", Planned, "The tools run Linux commands. PowerShell versions (Test-Connection, Resolve-DnsName, Test-NetConnection) are coming."),
    s(
        "proxmox",
        "Proxmox",
        No,
        "Proxmox VE is its own Linux system: a Windows host can't be a Proxmox node. Add the Proxmox node itself as a host.",
    ),
    s("desktops", "Desktops", Planned, "Desktops reach VM consoles through libvirt or Proxmox. Hyper-V guests (through RDP) are coming."),
    s(
        "node",
        "Nightgrid Node",
        No,
        "Nightgrid Node runs on Linux only, so it can't be installed here. Nightgrid reads this host over SSH instead.",
    ),
    s(
        "blueprints",
        "Blueprints and starters",
        No,
        "Blueprints and starters write Docker Compose stacks for Linux, systemd timers and ufw or firewalld rules, none of which exist on Windows.",
    ),
    s(
        "drift.history",
        "DRIFT reading shell history",
        Planned,
        "DRIFT reads bash and zsh history. Reading PowerShell's history file is coming.",
    ),
    s(
        "drift.lookups",
        "DRIFT checks on this host",
        Planned,
        "DRIFT's checks are Linux commands. PowerShell versions are coming.",
    ),
    s(
        "cmdwidgets",
        "Command tiles",
        Partial,
        "Your own commands run in the host's shell (cmd or PowerShell). The ready-made tiles are Linux commands.",
    ),
    s(
        "quests",
        "Quest checks",
        No,
        "Quest steps check a Linux guide's work on the host (files, services, packages). Windows hosts have no Linux guides to check.",
    ),
    s("system.cron", "Cron and timer editor", Planned, "The editor writes crontab and systemd timers. A Task Scheduler version is coming."),
    s("system.expiry", "Certificate expiry on the host", Planned, "Reads certificates with openssl through a POSIX shell. A Windows certificate store version is coming."),
];

/// The Windows entry for `feature`, if it has one.
pub fn windows(feature: &str) -> Option<&'static Support> {
    WINDOWS.iter().find(|s| s.feature == feature)
}

/// Why `feature` can't run on a Windows host, or `None` when it can.
pub fn unavailable(feature: &str) -> Option<String> {
    let s = windows(feature)?;
    match s.level {
        Full | Partial => Option::None,
        Planned => Some(format!("{} isn't available on Windows hosts yet. {}", s.label, s.why)),
        No => Some(format!("{} can't work on Windows hosts. {}", s.label, s.why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_texts_end_well() {
        let mut ids: Vec<_> = WINDOWS.iter().map(|s| s.feature).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate feature id");
        for s in WINDOWS {
            assert!(s.why.ends_with('.'), "{}: why should be full sentences", s.feature);
            assert!(!s.why.contains("—"), "{}: no em dashes", s.feature);
        }
    }

    #[test]
    fn messages() {
        assert_eq!(unavailable("terminal"), Option::None);
        assert!(unavailable("proxmox").unwrap().starts_with("Proxmox can't work on Windows hosts."));
        assert!(unavailable("system.units").unwrap().contains("yet"));
        assert_eq!(unavailable("no.such"), Option::None);
    }
}
