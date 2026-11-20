# Add a Windows Server host

Manage a Windows Server (or a Windows 10/11 PC) from Nightgrid over its built-in OpenSSH server: monitoring, Docker, services, the Event Log, Windows Update, the firewall and more, with nothing installed on the host. This article also says what Nightgrid can't do on Windows yet, and why.

## What you end up with

- A Windows host that logs in with your SSH key, or your Windows password when no key is set up yet
- A terminal on it in cmd or PowerShell, and its disks in the [Files](view:files) view
- Overview numbers, System tabs and Fleet checks for it, read through Windows PowerShell
- Knowing which views can't run on it, so a note in place of a panel isn't a surprise

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
- [ ] Or let Nightgrid do it: log in once with the password, then **Settings → Connections → SSH keys** pushes the key to the right file with the right permissions.

## Part 3 · Save the host

- [ ] Press **+** in the sidebar: name, address, port 22 and the Windows user name. Domain accounts can use `user@domain` or `DOMAIN\user`, the same as with `ssh`.
- [ ] Login: **SSH agent** or **Key file**. With **SSH agent** and no agent running (the Windows default when Nightgrid itself runs on Windows), Nightgrid tries `~/.ssh/id_ed25519`, `id_ecdsa` and `id_rsa`, then asks for the password, like OpenSSH does.
- [ ] A Windows 10/11 desktop or laptop: tick **Client, not a server**, so views never open on it by themselves and it shows as away when it's off.
- [ ] Click the host and accept its fingerprint. If your key isn't in place yet, Nightgrid asks for the Windows password instead. A refused key falls back to the password too, so you can log in and then fix the key.

#### Check

- [ ] A terminal opens at a `C:\Users\<user>>` or `PS C:\Users\<user>>` prompt.
- [ ] Reconnecting doesn't ask for a password (the key works).

## What works and what doesn't

Nightgrid learns that a host runs Windows the first time it connects and runs everything through Windows PowerShell 5.1, which every supported Windows has. A view or tab that can't run on the host shows a note saying why, instead of an error.

- **Terminal, Files, Tunnels**: work. The terminal is a plain cmd or PowerShell session (no key-hint strip or `nano` hook, which are Linux shell code). File paths start with the drive letter, like `/C:/Users/<user>`.
- **Overview and history**: CPU, memory, disks, network, uptime, NVIDIA GPUs and temperatures where the firmware reports them, from performance counters. There is no load average on Windows.
- **Docker**: containers, images, logs and stats through `docker.exe`; Compose stacks start, update and stop through PowerShell. Container shells start `sh`, so they work in Linux containers only.
- **System**: services (UNITS), the Event Log (JOURNAL), Task Scheduler (SCHEDULES), disk health and Storage Spaces (DISKS), Windows Update (UPDATES), Windows Defender Firewall with the same automatic rollback as on Linux (FIREWALL), and security checks (SECURITY).
- **Fleet**: log search over the event logs, the access audit from the OpenSSH log, backups from scheduled tasks or folders, and speed tests if `iperf3.exe` is on the host (put it on the PATH or in `C:\iperf3`; Nightgrid installs nothing).
- **Reboot, shut down, Wake-on-LAN**: work. Suspend only where Windows supports sleep, which most servers and VMs don't.
- **Not yet on Windows**: config drift, the cron and timer editor, power schedules, Hyper-V VMs, certificate expiry on the host, DRIFT's host checks, backup test restores, and the Compose deploy wizard, stacks from git and the .env vault.
- **Can't work on Windows**: Proxmox, Nightgrid Node, UPS through NUT, blueprints and starters, which are Linux-only.
- For Windows guests on Proxmox, the screen and RDP are in [Open VM screens and RDP](kb:howto/vm-desktops).

### Windows Update and the firewall

Windows Update refuses to install over an SSH login, so Nightgrid starts the install as a one-off task that runs as SYSTEM (under `\Nightgrid\` in Task Scheduler) and streams its log back. A firewall change exports the old rules first and arms a SYSTEM task that imports them back when the countdown ends, unless you press **Keep** after a fresh login worked. Both need an administrator account.

## Limits and gotchas

- **Permission denied (publickey)** for an admin account: the key is in `~\.ssh\authorized_keys` instead of `administrators_authorized_keys`, or that file's permissions let other users read it. Run the `icacls` line again.
- `C:\ProgramData` is hidden in Explorer; type the path in the address bar.
- Logs are in Event Viewer → Applications and Services Logs → OpenSSH → Operational.
- The Windows password asked at login isn't saved unless you tick **Remember secret in OS keychain**.

## Related

- [Add your first host](kb:howto/first-host)
- [When a host won't connect](kb:howto/troubleshoot-connection)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
- [SSH keys and agents](kb:basics/ssh-keys)
