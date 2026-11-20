# Add a Windows Server host

Manage a Windows Server (or a Windows 10/11 PC) from Nightgrid over its built-in OpenSSH server, with nothing installed on it: monitoring, Docker, Windows services, event logs, Windows Update, the firewall and more. This article also says what doesn't work on Windows yet, and why.

## What you end up with

- A Windows host that logs in with your SSH key, or your Windows password when no key is set up yet
- A terminal in cmd or PowerShell, its files in [Files](view:files), and live numbers on the [Overview](view:overview)
- [System](view:system) tabs that manage Windows services, event logs, scheduled tasks, Windows Update, disks and Windows Defender Firewall
- Knowing which views don't work on Windows, and why

## Before you start

- Windows Server 2019 or newer, or Windows 10 (1809) / 11. These ship OpenSSH Server as an optional feature.
- An account on it that may log in, usually a local or domain administrator.
- An SSH key on this computer. No key yet? **Settings → Connections → SSH keys → NEW KEY…** (see [Set up and rotate SSH keys](kb:howto/ssh-keys)).

## Part 1 · Turn on OpenSSH Server

In an elevated PowerShell on the Windows machine (or its console, RDP or Hyper-V/Proxmox screen):

- [ ] Install the server feature and start it on every boot:

```powershell
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Start-Service sshd
Set-Service -Name sshd -StartupType Automatic
```

- [ ] Check that the firewall rule exists (Windows creates `OpenSSH-Server-In-TCP` for port 22 during the install):

```powershell
Get-NetFirewallRule -Name OpenSSH-Server-In-TCP | Select-Object Name, Enabled
```

- [ ] Optional: make PowerShell the login shell instead of cmd:

```powershell
New-ItemProperty -Path "HKLM:\SOFTWARE\OpenSSH" -Name DefaultShell -Value "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -PropertyType String -Force
```

## Part 2 · Put your key in the right file

Windows OpenSSH reads keys for **administrator** accounts from one shared file, not from the user's own folder. This is the step that most often goes wrong.

- [ ] Copy your public key's line (the contents of `~/.ssh/id_ed25519.pub` on this computer).
- [ ] Administrator account: in an elevated PowerShell on the server, add it to `administrators_authorized_keys` and lock the file down to Administrators and SYSTEM. sshd ignores the file if anyone else can read it.

```powershell
Add-Content -Path C:\ProgramData\ssh\administrators_authorized_keys -Value 'ssh-ed25519 AAAA... you@laptop'
icacls.exe C:\ProgramData\ssh\administrators_authorized_keys /inheritance:r /grant "Administrators:F" /grant "SYSTEM:F"
```

- [ ] Standard (non-admin) account: the key goes in `C:\Users\<user>\.ssh\authorized_keys` instead, like on Linux.
- [ ] Or let Nightgrid do it: save the host with the Windows password first (Part 3), then **Settings → Connections → SSH keys → CHECK HOSTS** and **+** on its row. Nightgrid writes the key to the right file for the account, with the permissions sshd requires.

## Part 3 · Save the host

- [ ] Press **+** in the sidebar: name, address, port 22 and the Windows user name. Domain accounts can use `user@domain` or `DOMAIN\user`, the same as with `ssh`.
- [ ] Login: **SSH agent** or **Key file**. With **SSH agent** and no agent running (the Windows default when Nightgrid itself runs on Windows), Nightgrid tries `~/.ssh/id_ed25519`, `id_ecdsa` and `id_rsa`, then asks for the password, like OpenSSH does.
- [ ] A Windows 10/11 desktop or laptop: tick **Client, not a server**, so views never open on it by themselves and it shows as away when it's off.
- [ ] Click the host and accept its fingerprint. If your key isn't in place yet, Nightgrid asks for the Windows password instead. A refused key falls back to the password too, so you can log in and then fix the key.

#### Check

- [ ] A terminal opens at a `C:\Users\<user>>` or `PS C:\Users\<user>>` prompt.
- [ ] Reconnecting doesn't ask for a password (the key works).

## What works the same as on Linux

Nightgrid learns that a host runs Windows the first time it connects and remembers it. Commands run through Windows PowerShell 5.1, which every supported Windows has.

- **Terminal**: the host's own shell (cmd or PowerShell, whichever sshd is set to).
- **Files**: browse and edit over SFTP. Paths look like `/C:/Users/<user>`.
- **Power**: reboot and shut down use Windows' own `shutdown`; Wake-on-LAN works as for any host, also as a relay. Suspend asks Windows to sleep, which Windows Server and most VMs refuse; Nightgrid says so when they do.
- **SSH keys**: push and remove keys from Settings, as in Part 2.
- **Network tools** from this host: ping, path (`tracert`), DNS, ports, HTTP and certificates, neighbours and connections, all with Windows' own tools.
- **System → SECURITY**: Defender, firewall profiles, SSH password logins, Remote Desktop and NLA, SMBv1, update age, UAC, the Guest account, databases open to the network and who is an administrator. Some checks need an administrator account to read.

## What works with differences

- **Overview and monitoring**: CPU, memory, page file, disks, network, uptime and NVIDIA GPUs from Windows' performance counters. There's no load average, and temperatures show only where the firmware reports them (most servers and VMs don't).
- **Docker**: containers, images, logs, stats and events through `docker.exe` on the host (Docker Engine or Docker Desktop). Container shells start `sh`, so they work in Linux containers only. Compose stacks show from their containers, and up, pull, restart, update and down run through PowerShell.
- **System → UNITS**: Windows services, with start, stop, restart and automatic or disabled startup. Changing one needs an administrator account (Windows has no sudo).
- **System → JOURNAL**: the System and Application event logs (tick **Security log** for the Security log, administrators only). The unit filter matches an event source such as `Service Control Manager`.
- **System → SCHEDULES**: your Task Scheduler tasks (not the ones under `\Microsoft\`) with triggers and last results; run, enable or disable them. Creating tasks from Nightgrid isn't there yet.
- **System → UPDATES**: checks Windows Update and installs everything it offers. Windows won't install updates for an SSH login, so Nightgrid runs the install as a one-off SYSTEM task (under `\Nightgrid\` in Task Scheduler) and streams its progress. Needs an administrator account; picking single updates isn't there yet.
- **System → FIREWALL**: Windows Defender Firewall profiles, the inbound default and your own inbound rules, with the same automatic rollback as on Linux: the old rules are exported first, and a SYSTEM task puts them back when the countdown ends unless you press **Keep** after a new login worked (see [Change a firewall safely](kb:howto/firewall)). Needs an administrator account.
- **System → DISKS**: disk health from Windows' Storage module and Storage Spaces pools. Temperature, wear and error counts show only for administrator accounts.
- **Fleet → LOGS** searches the event logs; **ACCESS** reads logins from the OpenSSH event log; **BACKUPS** watches Task Scheduler tasks (Windows Server Backup included) and folders such as `D:\Backups`; **SPEED** needs `iperf3.exe` on the PATH or in `C:\iperf3`, because Windows has none and Nightgrid installs nothing.
- **Network scans** from a Windows host use its interfaces, routes, a ping sweep, the neighbour table, reverse DNS and Tailscale peers; mDNS, NetBIOS, SSDP and UPnP aren't read.
- **Tunnels**: TCP forwards work; forwards to Unix sockets don't.
- **Automation and command tiles** run in the host's shell, so write them for cmd or PowerShell.
- **DRIFT** reads PowerShell's history (cmd keeps none).

## Not on Windows yet

- Config drift (System → CONFIG), the cron and timer editor, power schedules, Hyper-V VMs and desktops, certificate expiry on the host, DRIFT's checks on the host, backup test restores, and the deploy wizard, stacks from git and the `.env` vault (put compose files on the host yourself and run them from **STACKS**).

## Can't work on Windows

- **Proxmox** and **Nightgrid Node**: both are Linux only. Add the Proxmox node itself as a host.
- **UPS through NUT**: NUT doesn't run as a service on Windows. Watch the UPS from a Linux host NUT can reach.
- **Blueprints and starters**: they write Linux compose stacks, systemd timers and firewall rules.
- **Quest checks**, and the terminal's key hints and `nano` hook, which are Linux shell code.

Where a view or tab can't run on the selected Windows host, it says why in place instead of showing an error. For Windows guests on Proxmox, the screen and RDP are in [Open VM screens and RDP](kb:howto/vm-desktops).

## Limits and gotchas

- **Permission denied (publickey)** for an admin account: the key is in `~\.ssh\authorized_keys` instead of `administrators_authorized_keys`, or that file's permissions let other users read it. Run the `icacls` line again, or push the key from Settings as in Part 2.
- Most changes (services, updates, firewall, other users' event logs) need an administrator account: Windows has no sudo, so log in as one.
- `C:\ProgramData` is hidden in Explorer; type the path in the address bar.
- Logs are in Event Viewer → Applications and Services Logs → OpenSSH → Operational.
- The Windows password asked at login isn't saved unless you tick **Remember secret in OS keychain**.

## Related

- [Add your first host](kb:howto/first-host)
- [When a host won't connect](kb:howto/troubleshoot-connection)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
- [SSH keys and agents](kb:basics/ssh-keys)
