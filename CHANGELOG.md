# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [SemVer](https://semver.org/). Changes go under **[Unreleased]**; the release train names the version when it ships them.

## [Unreleased]

### Added
- **Windows: pick PowerShell, Command Prompt or a WSL distro for the local shell.** On Windows the terminal's **+ NEW** menu lists each shell on this computer: PowerShell 7 (when installed), Windows PowerShell, Command Prompt and every installed WSL distro. WSL shells start through `wsl.exe` in the distro's Linux home (`~`), not in the Windows folder. **Settings → Terminal → Local shell** picks what `Ctrl+Shift+T` and **Local shell** open. Shown only when WSL is installed; macOS and Linux are unchanged.
- **Transmissions from the developer.** When something needs fixing on your end (a provider changed something, a setting needs a nudge, a known issue has a workaround), a notice can now reach every installed Nightgrid without an app update. It opens as an **INCOMING TRANSMISSION**: the channel tunes in, the signal locks and the message decodes, with its own sound, the steps to take and a button to the right article, view or page. **Understood** hides it for good, **Later** until the next start. Notices are signed with the release key and checked on this computer, so nobody else can send one; they can be aimed at a system, a range of versions or a feature, and expire by themselves. Settings → Updates has the switch, **Check now** and **Show all**.

## [1.21.1] - 2026-11-21

### Fixed
- **Keys reach `sudo` programs in SSH terminals again.** With key hints or "edit in Nightgrid" on, a program run with `sudo` that switches terminal modes (menus, `[y/N]` prompts, like `sudo nightgrid-node fix`) could stop receiving keys: no letters, no Enter, not even Ctrl+C, while the cursor kept blinking. The session hook now hands the shell the terminal's own device instead of `/dev/tty`, which `sudo` didn't recognise as the user's terminal.
- **Busy logs no longer flicker.** A container's log restarted every time the Docker list refreshed, so it cleared and reloaded every few seconds; it now keeps streaming. Bursts of output no longer paint a frame of empty rows while following the end, and when you scroll up to read, the lines you're reading stay put even as the oldest drop off.
- **System → Journal keeps up with fast hosts.** New entries are drawn once per frame instead of on every batch, and only new rows are added, so a chatty journal scrolls smoothly instead of stalling.

## [1.21.0] - 2026-11-20

### Added
- **What's growing.** Every mount in Fleet → DISKS has **What's growing**: the biggest folders and files on that filesystem, what grew since the last scan, and files changed today. VM disk images (`.vhdx`, `.qcow2`, `.vmdk`, `.vdi`, Hyper-V checkpoints…) are flagged with what to do, since their growth is inside the VM and they don't shrink by themselves; Docker and the journal get hints too. Click a folder to look inside, **Read as root** for folders the login can't open. Works on Linux, macOS and Windows hosts, installs nothing, and the previous scan stays on the host (`~/.nightgrid/growth`), not on this computer.
- **Storage links: "disks stored on".** On a drive's What's growing panel, link the VMs whose disk images live there (a VM image named after a saved host is suggested). It's not a "depends on": a linked VM's alerts fold under that drive's critical DISK FULL and under its hypervisor going offline, while a share mounted from a NAS never makes a host depend on it. Links are in the profile export.

### Changed
- **Disk forecasts understand shares and nested storage.** Fleet → DISKS tells a host's own disks from NFS/SMB/SSHFS shares, Windows drives passed into WSL, VM shared folders and ZFS/APFS datasets, with a chip on each. A share from a saved host (say `/mnt/plex` from the NAS) is counted once, on the NAS, and shown on the client as **counted on nas** with no FILLING UP or DISK FULL alert; the same share mounted on several hosts with no owner saved counts on the first. Datasets of one pool are forecast together (their free space is shared), and the same storage mounted twice on one host shows once. WSL's `/mnt/c`-style drives say the disk belongs to Windows, and SMART no longer reads as missing inside WSL or a VM guest. DRIFT's disk explanations get all of this.

### Fixed
- **Hub member says "connection refused" for its hub.** A member that is only on the LAN pings its hub on the hub host's LAN address, but the hub's collector only listened on Tailscale, so the link failed while pulls (which go the other way) worked. Joining a hub now also makes the collector answer on that LAN address, and a member whose hub link fails gets **Fix on <hub>** under LINKED: it updates the hub if needed and adds the address in one go.
- **The hub's version.** The hub card said "Node 0.26.0" while showing the hub program's own version; it now says **Hub 0.26.0**, and when the node next to it is newer (the hub isn't updated with the node) it says so and offers **Update hub** (config and history stay).
- **"Make a hub…" greyed out with no reason.** The card now says why (no host with Node 0.26+ connected, or which hosts to update), and a host marked as a client device that already runs the node can be picked as the hub too.
- **Link card says what to do.** When the node-to-node link is off or has no tailnet address, **Turn on the link** runs `allow peers` as root and, on a network without Tailscale, makes the link listen on the host's LAN address (`[peers] listen = "lan"`, Node 0.28+). Hubs work on LAN-only networks.

## [1.20.0] - 2026-11-20

### Added
- **Synology with 2-step verification on.** The Synology DSM card no longer needs an account without 2-step: enter one current code in the integration's settings and DSM trusts Nightgrid from then on. The device token stays in the OS keychain (not in profile exports, so another computer asks once too), **Forget** removes it, and a card whose token DSM dropped shows **Enter 2-step code**. A regular (non-administrator) account works too: the card shows what DSM shares with it and names what needs an administrator instead of failing.
- **SNMP cards (read-only).** **Add integration → SNMP** reads any SNMP agent with no app account: on a Synology NAS, system, power and fan status, temperature, storage pools (degraded in red), every disk with its health, volumes, CPU, memory and uptime; other agents show volumes, CPU and memory. SNMPv3 with SHA or MD5 and AES or DES, or v2c. Built in, nothing to install.
- **Library: A Synology NAS alongside Nightgrid** now covers the two doors into DSM (web sign-ins with 2-step, SSH with keys), a key-only administrator for SSH, SNMP with no DSM account, and Docker as its own user.

## [1.19.0] - 2026-11-20

### Added
- **Acknowledge alerts.** **✓ Ack** on an alert keeps it in the list but takes it off the badge and tray and stops every notification for it (desktop, inbox, ntfy, webhooks). Pick **Until it clears**, a time (1 hour to 30 days, or custom) or **Permanently**. Whatever you pick, it comes back if it gets worse, such as a disk crossing from WARN into CRITICAL.
- **Mute alerts.** **Mute…** silences this alert, every alert of its kind on its host, everything on its host, or its kind on every host, for a time or forever. **ACKNOWLEDGED & MUTED** in the Alerts panel lists them with the time left and an **Undo**. Silenced alerts stay on the host's timeline, tagged **ACKNOWLEDGED** or **MUTED**, and aren't counted as problems.
- **An alert that gets worse notifies again** (WARN to CRITICAL), and a timed acknowledgement or mute that runs out while its alert is still active notifies once.

### Changed
- **DISK FULL skips system mounts** by default (Settings → Alerts → **Skip system mounts**): read-only mounts and ones full by design, such as WSL's and Docker Desktop's `/mnt/wsl/…` (for example `/mnt/wsl/docker-desktop/cli-tools`), snaps and CD/DVD images.

## [1.18.0] - 2026-11-20

### Added
- **Run Docker as a separate user.** A host can now have a **Docker user**: Nightgrid keeps logging in over SSH as your normal user and runs every Docker action on that host (containers, logs, shells, stacks, compose, routes) as that account through `sudo -u`. Handy for a dedicated docker user with its own UID and GID, as on many Synology NAS setups. Set it in the host's settings or with **Use a separate Docker user instead** on the NO ACCESS TO DOCKER card. Only the name is saved (it's in the profile export); no second login or password.
  - The first time, **DOCKER USER NEEDS SUDO** shows the one sudoers rule it needs (your login may run docker and docker-compose as that user, nothing else) and **Fix it for me** adds it in its own `/etc/sudoers.d` file after checking it with `visudo`. If the Docker user can't open the socket, the usual card shows the group steps for that user. **Stop using <user>** goes back to your login.

## [1.17.0] - 2026-11-20

### Added
- **Hold updates back.** Keep anything you don't want touched out of updates, and silence its warnings. Nothing is held by default, held things are marked HELD, and **Fleet → UPDATES → HOLDS** lists everything held with **Release**.
  - **Hosts**: **Hold…** on a host in Fleet → UPDATES, for 1 day, 1 week, 30 days or until released, with a note. A held host isn't counted as pending (Fleet badge, Overview widget, DRIFT) and can't be picked for **Upgrade selected…**.
  - **Packages**: **Hold** / **Release** on any package in Fleet → UPDATES or System → UPDATES. On apt, dnf with versionlock and Windows the hold is set on the host (`apt-mark hold`, `dnf versionlock`, hiding the update in Windows Update), so unattended upgrades and other tools respect it too; holds made outside Nightgrid (including pacman's `IgnorePkg`) show as held. On pacman and dnf without versionlock, Nightgrid's upgrades leave held packages out. Held packages aren't counted and the upgrade confirmation names them.
  - **Container images and stacks**: **Hold** an image, or **Hold stack**, in a stack's **Updates** dialog (or next to an image in **Check images**). Pull and Update then skip its services, so they stay on the image they run; checks still say when a newer version exists, without counting it.

### Fixed
- **Enter confirms type-to-confirm dialogs.** After typing the name (host, group, VM, key or folder), pressing Enter, including keypad Enter, now runs the red button in every type-to-confirm dialog: Upgrade hosts, power and group power, Docker, VMs and Proxmox actions, firewall changes, SSH key removal and rotation, file delete and a changed host key. Enter does nothing until the name matches, and it waits while an input method (IME) is still composing.

## [1.16.1] - 2026-11-20

### Fixed
- **macOS stops asking for keychain access after every update.** Releases are now signed with one stable certificate instead of a new ad-hoc signature each time, so macOS keeps recognising the updated app and remembers "Always Allow" for saved passwords and tokens. The first update with the new signature asks one last time per saved secret; choose **Always Allow** and later updates stay quiet. No Apple developer account or notarization is involved, and opening a freshly downloaded build still needs the quarantine step in the README.
- **Web UI windows no longer stay black on macOS.** macOS (14 and later) refused plain http pages on LAN and tailnet addresses inside Nightgrid's web windows without saying so. Web windows may now load them; the app's own connections are unchanged.
- **A web window that can't load its page says so** after 15 seconds instead of staying black, with the address, **Try again** and **Open in browser**.

## [1.16.0] - 2026-11-20

Windows Server hosts get close to everything Nightgrid does on Linux, using only what Windows ships with. Every view says plainly what can't run on a Windows host and why.

### Added: Windows hosts
- **Nightgrid knows which hosts run Windows.** It learns it on connect (OpenSSH for Windows says so in its version line) and remembers it per host. Nothing is installed on the host: commands run through Windows PowerShell 5.1.
- **Overview, monitoring and history** read Windows performance counters through one long-running PowerShell sampler: CPU, memory and page file, disks and volumes, network, uptime, NVIDIA GPUs and ACPI temperatures. Alerts and history work as for Linux hosts.
- **Docker** talks to `docker.exe` on the host (Docker Engine or Docker Desktop): containers, images, logs, stats and events.
- **Reboot, shut down and suspend** use Windows' own `shutdown.exe` and sleep. Suspend checks what the host supports first and says so when it can't sleep.
- **Wake-on-LAN relays** work from Windows hosts.
- **SSH keys** are pushed, listed and removed the Windows way, including `administrators_authorized_keys` with the permissions sshd requires.
- System shows whether the account is an administrator.
- **System → UNITS** lists Windows services with their state and startup type; start, stop, restart, and set them to automatic, manual or disabled.
- **System → JOURNAL** reads the System and Application event logs (and Security, for administrators) with the same source, severity, time and follow filters.
- **System → SCHEDULES** lists Task Scheduler tasks (Microsoft's own are hidden): run one now, enable or disable it.
- **System → DISKS** shows physical disk health, wear, temperature and error counters, and Storage Spaces pools.
- **System → UPDATES** checks Windows Update and installs updates. Windows Update refuses to install over an SSH login, so the install runs as a one-off SYSTEM scheduled task and its log streams back live.
- **System → FIREWALL** manages Windows Defender Firewall (profiles, default policy, allow and delete rules, enable and disable rules) behind the same rollback as Linux: the old rules are exported first and imported back by a SYSTEM task when the countdown ends, unless you keep the change.
- **System → SECURITY and the security posture** check Defender, firewall profiles, RDP and NLA, SMB1, UAC, patch age, the Guest account, administrators, sshd password logins and exposed listeners.
- **Fleet → LOGS** searches the event logs; **Fleet → ACCESS** reads SSH logins from the OpenSSH event log, local accounts and administrators; **Fleet → BACKUPS** watches Task Scheduler tasks (Windows Server Backup included) and folders; **Fleet → SPEED** runs iperf3.exe when it's on the host.
- **Compose stacks** run up, pull, update, restart and down through PowerShell.
- **Network tools and the network scan** run in PowerShell. DRIFT can read PowerShell's history.

### Changed: Windows hosts
- **Views and tabs that can't run on a Windows host say why** in place of the view, with what the Windows version will use when one is planned. Commands behind them stop early with the same words instead of a shell error from cmd. Background checks (daily posture, SMART, backups, log watches, incident timeline) skip Windows hosts quietly.
- Backup jobs accept Windows paths (`D:\Backups`, `\\nas\share`) and scheduled task paths.
- The deploy wizard, stacks from git and the .env vault say plainly that they need a POSIX shell on Windows hosts.

## [1.15.0] - 2026-11-20

### Added: Docker on Synology NAS
- **NO ACCESS TO DOCKER knows Synology DSM.** DSM has no `usermod`, so the old hint failed with "command not found". On a Synology the card now shows the DSM way: `sudo synogroup --add docker <you>` (or `--memberadd` if the group exists) plus `sudo chown root:docker /var/run/docker.sock`, because Container Manager creates the socket owned by root. Plain Linux hosts get `usermod -aG docker` (and `groupadd docker` if the group is missing). Steps that are already done are left out.
- **Fix it for me**: one button runs those steps as root on the host (it asks for the sudo password like System does), then reconnects so the new group applies. If you're already in the group and only the SSH login is older, the card says so and offers **Reconnect**.
- **Passwordless sudo works as a fallback**: when your user can't open the socket but may run docker through sudo without a password, Nightgrid talks to Docker through `sudo docker system dial-stdio` instead of showing the card.

### Fixed
- Docker on hosts that keep the `docker` binary in `/usr/local/bin` (Synology DSM, some NAS systems): non-interactive SSH sessions don't have it on `PATH`, so Docker views, stacks, container shells and DRIFT lookups couldn't find it. Nightgrid now adds `/usr/local/bin` itself.

## [1.14.1] - 2026-11-20

### Fixed
- Type-to-confirm dialogs (upgrade hosts, power, removing containers, deleting files, SSH keys, firewall, VMs, Proxmox, UPS, automation runs) showed the name to type in uppercase, so a host called `braincage` read as BRAINCAGE and typing that kept the button disabled. The name is now shown exactly as stored, and the typed name is matched ignoring case.

## [1.14.0] - 2026-11-19

### Added: Edit on every Expiry Watch row
- **Rows Nightgrid finds by itself** (SERVICE TLS from Services, PROXY TLS from Docker → Routes) now have an **Edit** button too. It shows the certificate (subject, issuer, end date, SHA-256 fingerprint) and lets you:
  - **Trust this certificate**: hides the "unknown issuer or self-signed" warning for that exact certificate. If the server ever sends a different one, the row warns again ("certificate changed since you trusted it"). The service card shows it as trusted too.
  - **Mute expiry alerts**: the row stays, greyed out at the bottom, and never raises a certificate alert.
  - Your own **expiry date** and **warn days** instead of the certificate's date and Settings → Alerts.
  - **Reset to found** removes your edits.
- Items you add can be muted the same way (**Mute alerts for this item**).
- All of it is included in profile exports.

### Fixed
- DRIFT described an Expiry Watch edit button that didn't exist yet. The guide now describes the real controls, and DRIFT is told never to describe buttons or options its guide doesn't name.

## [1.13.0] - 2026-11-19

### Added: storage, network and monitoring integrations
Nine more integration types, 34 in all.
- **TrueNAS**: pools with health and fill level, alerts, uptime, load and memory.
- **Synology DSM**: volumes, disks with SMART problems, CPU, memory and temperature.
- **Unraid** (7.2+): array state and capacity, disks with temperature and errors, containers and VMs.
- **Proxmox Backup Server**: datastores with fill level and full-by estimate, failed tasks.
- **OPNsense**: gateways with delay and loss, stopped services with **Start**, firmware updates.
- **UniFi Network** (9.0+): devices, offline first, clients and **Restart** for a device.
- **Nginx Proxy Manager**: proxy hosts, nginx errors, expiring certificates, **Enable** and **Disable**.
- **Grafana**: dashboards and firing or pending alert rules.
- **Prometheus**: targets down with their errors, firing alerts.

### Changed: layout sweep
- **One tab bar.** View tabs (System, Fleet, Docker, Proxmox, Security, Settings, Services), sub-tabs inside panels and every segmented picker (unit filter, time ranges, map style, webhook kind, auth method …) now share one style and one height. Primary tabs line up with the host picker, filter box and buttons beside them; pickers had five different "selected" looks and now all use the magenta underline (form pickers in dialogs keep the tinted fill).
- **Small buttons** are one size (22 px) everywhere. About seventy panels used the small-button class without defining it and showed full-height buttons in table rows.
- **Filter chips and status tags** share two sizes, so a row of chips or tags no longer mixes 16, 18, 20 and 24 px heights.
- Dimmed text that asked for an undefined colour now uses the muted text colour, as intended.

### Fixed: layout at narrow widths
- Narrow windows: container rows, proxy routes, systemd timers, speed tests, power, Nightgrid Node and expiry rows fit their pane instead of pushing action buttons off the edge; the Files panes keep the file name column (MODE hides when a pane is narrow); the Network map keeps at least 360 px of height and the view scrolls instead of squashing it to nothing; Settings tabs wrap instead of running off the page.
- The top bar keeps its breadcrumb readable in a narrow window: the search box drops its placeholder words (the shortcut stays) and the indicators no longer break onto two lines.
- Service cards wrap their buttons instead of clipping EDIT at the card edge; the MCP port field no longer overflows its box; disk usage text no longer splits "61.0 / GB"; the Network ping sweep option and the saved-network "on map" boxes line up.

## [1.12.1] - 2026-11-19

### Fixed: false downtime in Fleet → UPTIME
- **A down has to be confirmed.** A host or service now counts as down only after staying down for 2½ minutes; a single failed check or a few failed polls used to count as a minute or more of downtime each. Unconfirmed blips are unknown time (grey), not up or down.
- **Waking up isn't an outage.** For 3 minutes after Nightgrid starts or the computer wakes, failures don't count, and the gap before isn't filled in with whatever was seen last.
- **Your own network dropping isn't an outage.** A round where every host (or everything watched) fails at once is unknown time.
- **Client devices** (laptops, handhelds) never count as down, even with "Alert when it goes offline" on.
- **Self-signed services**: a service that answers but whose certificate this computer doesn't trust counts as up in uptime history (the Services view still shows the certificate problem).
- **Nightgrid Node** hours where the node saw its host up now cancel downtime the app recorded for that time.
- **History already recorded is cleaned up** once: client downtime becomes away time, and outages shorter than 2½ minutes are removed.

## [1.12.0] - 2026-11-19

Faster updates: interface fixes between releases, batched releases and quicker release builds.

### Added: interface fixes between releases
- **Fixes that only touch the interface now arrive without a full update.** They're a small signed download built for exactly the version you run, applied the next time Nightgrid starts (a card offers **Restart now**). Settings → Updates has the switch (**Interface fixes between releases**, on), what's in use, **Check fixes** and **Use built-in**.
- A fix can only replace the interface's files: the app's permissions and content security policy stay as built, and the fix is checked against the release key on every start. If a fix doesn't start within a minute, Nightgrid goes back to its built-in interface by itself and never uses that fix again.

### Changed: releases
- **Fixes merged close together now ship as one release** instead of one after another, and the release picks its own version number.
- **Release builds take about half as long**: the platform builds run beside the checks instead of after them, and the Linux build tools install in a fraction of the time.

## [1.11.0] - 2026-11-19

Nine more integrations: download clients, files and photos, notifications and cameras.

### Added
- **qBittorrent** and **Transmission**: speeds, torrents with progress and errors, pause or resume one or all (login optional).
- **SABnzbd** and **NZBGet**: speed, queue with progress, free disk, recent failures; pause, resume or delete.
- **Immich**: photos, videos, storage and background jobs.
- **Nextcloud**: users, files, active users, shares, free space, load and app updates.
- **Syncthing**: folder sync state, devices online, errors; rescan, pause and resume.
- **Gotify**: health, apps and latest messages; delete one or all.
- **Frigate**: cameras with frame rates, detector speed, recording storage and latest detections.

## [1.10.0] - 2026-11-19

### Added: back and forward
- **Mouse side buttons go back and forward** through where you've been: views, the tabs beside a view's title (One host / Fleet, Map / Tools…), tabs inside a view (Fleet → DISKS, Docker → STACKS, Proxmox, Security, Files editors, Settings tabs), the host a view shows and Library articles. On macOS the buttons are caught natively, because the system webview doesn't pass them to the page.
- **Keys**: `Cmd+[` / `Cmd+]` on macOS, `Alt+Left` / `Alt+Right` on Windows and Linux, plus the Back/Forward keys some keyboards and mice send. They stay with text fields and editors, and with the terminal on Windows and Linux, where shells move by word with `Alt+Left` / `Alt+Right`.
- **‹ ›** buttons at the top left, and a **two-finger sideways swipe** on a trackpad (Settings → General → Swipe to go back; areas that scroll sideways keep the swipe).

## [1.8.1] - 2026-11-19

### Fixed
- **Library shelf filters** are all the same size again. The ◇ QUESTS chip had picked up the Objectives panel's frame (notched corner, thick left edge, extra margin), which stretched ALL and USING NIGHTGRID into tall boxes. QUESTS keeps its amber look at the normal chip size, and the chips wrap evenly at any width.
- **Sidebar collapse button** no longer gets clipped or overlaps Settings when the sidebar is narrow. The decorative barcode beside Settings shrinks first and hides below about 216 px.

## [1.8.0] - 2026-11-19

Ten more integrations for the media stack, on a shared integration card.

### Added
- **Jellyfin** and **Emby**: what's playing, transcodes, library counts and **Scan libraries**.
- **Tautulli**: Plex activity with quality, transcode decisions and bandwidth.
- **Sonarr**, **Radarr** and **Lidarr**: download queue with progress and import problems, missing items, the app's health warnings, **RSS sync**, **Search missing** and **Remove** on queue items.
- **Prowlarr**: indexers, failing and disabled ones, **Test indexers**.
- **Bazarr**: missing subtitles and throttled providers.
- **Overseerr** and **Jellyseerr**: request counts and pending requests with **Approve** and **Decline**.
- The integration type picker is grouped (Media, Media automation, Network, Monitoring, Docker, Games) and has a filter.

### Changed
- Integration types are checked by the app instead of a database constraint (migration 0050), so new types never need a table change again (D-182).

## [1.7.2] - 2026-11-19

### Fixed
- **Adding a Pi-hole or AdGuard Home integration failed** with "CHECK constraint failed: kind IN ('plex','amp','kuma','portainer')". The integrations table never learned the two DNS kinds from 0.10.36; migration 0049 rebuilds it with every kind and keeps your existing integrations (same ids, so saved passwords and tokens still match).

## [1.7.1] - 2026-11-19

### Fixed: logging in to Windows hosts (and any host without an agent key)
- **SSH agent hosts no longer fail when there's no agent.** With no agent running (the Windows default) or an empty one, and no key file set, Nightgrid now tries `~/.ssh/id_ed25519`, `id_ecdsa` and `id_rsa`, then asks for the password when the server takes one, like OpenSSH. Before, the login stopped at the agent's error (often "SSH agent: early eof") and never offered a password.
- **A refused key now falls back to the password** on key-file and agent hosts. Before, the password prompt came back again and again because the password was never used.
- **Terminals on OpenSSH for Windows** (Windows Server, Windows 10/11) open a plain shell. The key-hints and edit hook are POSIX shell, so on cmd or PowerShell they ended the session at once.
- Failed connections are written to the log, so exported diagnostics say why a host won't connect.

## [1.7.0] - 2026-11-19

Ten more starter shelves: apps, scheduled jobs, firewall profiles, alert rules, Overview tiles and dashboards, log searches, proxy routes, Proxmox shapes, restore drills and security posture.

### Added: starter shelves (Fleet → BLUEPRINTS → STARTERS)
- **APPS**: twelve single-app stacks: Vaultwarden, Immich, Nextcloud, Uptime Kuma, Paperless-ngx, Homepage, Forgejo, Navidrome, Audiobookshelf, Mealie, FreshRSS and Actual Budget. Secrets go into the `.env` vault and are generated when left empty.
- **JOBS**: six scheduled jobs installed as Nightgrid timers: Docker cleanup, unattended upgrades, weekly SMART self-tests, a nightly backup kick-off, weekly TRIM and monthly ZFS scrubs. Jobs that need a disk, pool or unit ask for it as one plain word.
- **FIREWALL**: four allow-rule profiles: SSH only, web server, LAN and tailnet only, and Docker host. On firewalld, trusting a network becomes a step for you.
- **ALERTS**: four alert rule sets (quiet nights, strict storage watch, calm log alerts, every alert on), shown as a table of each setting now and after. Where alerts are sent stays your choice.
- **TILES** and **DASHBOARDS**: four sets of read-only command tiles (host basics, Docker, storage health, network) and three dashboards (media box, NAS watch, network ops) that bring their own tiles for a host you pick. Dashboards are added beside yours, never over them.
- **LOG SEARCHES**: five saved searches across every host: SSH logins, out-of-memory kills, disk errors, container errors and failed services. Disk errors and OOM kills alert on new matches.
- **PROXY ROUTES**: four route sets for the app starters, saved to a Caddy or Traefik proxy Nightgrid found on a host under your own domain. You see the Caddyfile diff (or the Traefik routes) and confirm; Caddy validates before loading and keeps the old file.
- **PROXMOX**: five VM and container shapes (Docker LXC, small service LXC, Linux VM, Home Assistant OS VM, Windows 11 VM) that open Proxmox → Create filled in, picking the newest matching container template.
- **DRILLS**: weekly or monthly restore drills added for each backup job that has none.
- **POSTURE**: two sets of Security view and network watch settings (watchful, relaxed).

## [1.6.0] - 2026-11-18

### Added: UI sounds
- **Short cyberpunk-style UI sounds**, all synthesized by Nightgrid itself (no sound files or packs): alerts firing (a sharper one for critical) and clearing, new inbox notices, actions you started working or failing (container actions, firewall applies, Proxmox actions, package runs, power), confirm dialogs (a darker one for destructive ones), DRIFT finishing a reply, quest steps and completed quests, and scans, test restores, drills and speed tests finishing. Never for terminal output or live numbers.
- **Settings → SOUND**: master switch, volume, a switch per kind of sound with a ▶ test button for every sound, quiet hours, and mute while the window isn't focused. On at 40 % to start; the view-switch tick is off until you turn it on. Included in profile exports, and in Settings → Features as **UI sounds**.

## [1.5.0] - 2026-11-18

Starters: ready-made server setups and alert runbooks you can read, preview and use as they are.

### Added: starter blueprints (Fleet → BLUEPRINTS → STARTERS)
- **Nine starters** for common jobs: media server (Jellyfin with Sonarr, Radarr and Prowlarr), reverse proxy with HTTPS (Caddy), network-wide ad blocking (Pi-hole), file sync and sharing (Syncthing and File Browser), monitoring basics (Uptime Kuma and Dozzle), home automation (Home Assistant and Mosquitto), a Minecraft server with automatic world backups, a hardened Docker host and a backup target (restic REST server, append-only).
- Each one explains in plain words what it's for, lists what it sets up (stacks, ports open on the LAN and ports kept on the host, firewall rules, timers, steps left to you), shows the exact files it writes and links its Library guides.
- Pick a host and a folder, answer a few questions, and **Preview what it would do** shows the same step-by-step plan as a blueprint. **Apply ticked steps…** asks you to confirm before anything is written. Passwords go into the stack's `.env` vault and are generated when left empty; nothing on the host is deleted, and denying traffic or turning a firewall on stays a step for you in System → FIREWALL.
- Your own blueprint files moved to **YOUR BLUEPRINTS** next to it, unchanged.

### Added: starter runbooks (Knowledge → Starters)
- **Seven runbooks** for disk full, a container down or crash-looping, a failed systemd unit, a certificate expiring, a host unreachable, high memory and a failed or stale backup: what it means, steps to tick and commands to send to a terminal, with Library links.
- **Add to my runbooks…** makes one your note, shown as **▤ Runbook** on matching alerts. An alert with no runbook of yours offers **▤ Starter runbook** when one fits.
- Library links in notes (`kb:`) now open the article.
- **New how-to**: *Start from a ready-made setup*.

## [1.4.0] - 2026-11-18

### Added: quest log
- **Start a quest** from any Library article with steps: the strip under the title says **◇ QUEST AVAILABLE**; pick the host it's for (or none) and press **▶ START QUEST**. Articles that can be quests are marked **◇ QUEST** in the list and on the shelves, and the **◇ QUESTS** chip shows only those. The same guide can run as separate quests on different hosts.
- **QUESTS** in the top bar, beside INBOX: every active quest with its host, objective, progress bar and the step you're on. Click one to open the guide on that host; **Check now** runs its read-only checks; **Abandon** stops tracking it; finished quests are kept under **Completed**.
- **Quest updates**: finishing a step shows **QUEST UPDATED** in the top right with what's next; finishing a part shows **OBJECTIVE COMPLETE** and the last step **QUEST COMPLETE**. The text decodes in, and the motion is off under reduce effects.
- On a quest, steps that pass their read-only check complete by themselves, after **Check** and every few minutes while Nightgrid is already connected to the host.
- Quests are kept in the database and travel in profile exports.

## [1.3.0] - 2026-11-18

### Fixed: installing Nightgrid Node
- **Install… failed with "reading the response failed: length limit exceeded".** The download was cut at 4 MB and the node binary is 5.4 MB; release downloads may now be up to 64 MB, and a response over a limit says so in words.
- A host whose CPU has no Node build (32-bit ARM on many NAS boxes) shows **no build for armv7l** instead of an Install button that can't work.
- Client devices (laptops, handhelds) are left out of the NODE list unless they already run the node; **Show client devices** brings them back.

### Added: hub setup in Fleet → NODE
- **HUB card**: make one always-on server the network's hub (Nightgrid Node 0.26+). It pulls the other servers' history read-only and notices a host going offline while Nightgrid is closed. **Make a hub…** installs it next to the node, **Add members…** links the other servers (including LAN-only ones behind a jump host), and the card shows each member as the hub sees it, with **Leave**, the hub's events, config, logs and **Remove…**.
- **ROLE** column: hub, reports to hub, or direct.
- **Per-host tabs**: STATUS (services and `doctor`), HUB & LINK (role, join or leave, the link's address and peers), CONFIG (edit the node's or hub's config: checked first, the old file kept, services restarted, rolled back if they don't come up) and LOGS (the services' journal).

## [1.2.0] - 2026-11-18

### Added: Library content updates
- **Library articles now update between app releases.** When only Library content changes (articles, guides, figures, the error reference), it's published as a small signed download, and Nightgrid picks it up after start-up and every 6 hours without an app update or restart. The Library's home page shows the content date and whether it's built in or a downloaded update.
- **Settings → Updates → Library content updates** (on by default) turns it off; **Check content** looks right away, and **Use built-in copy** removes downloaded content.
- Downloads come from this project's GitHub Releases and are checked against the same signing key as app updates; anything that doesn't verify, is older than the built-in copy or is meant for a newer app version is ignored. The Library still works fully offline, and DRIFT reads the newest content.

## [1.1.0] - 2026-11-18

Nightgrid 1.1: a Security view, an inbox for every notice, and four new Fleet tabs for knowing what runs where and proving it can come back.

### Added: local notifications
- **INBOX** in the top bar: every notice Nightgrid sent in the last 30 days (alerts firing and clearing, restore drills, rotations due, security score drops, capacity warnings) with an unread count. Click one to open what it's about.
- **Settings → Alerts → Notifications**: choose per notice type whether it reaches the desktop and ntfy.
- **ntfy (self-hosted)**: off by default; push notices to your phone through your own ntfy server, with an optional access token kept in the keychain, a level filter and a test. The public ntfy.sh is refused. An "ntfy" alert webhook from an older version moves here by itself.

### Added: Security view (Mod+Shift+Y)
- **Security → POSTURE**: a score from 0 to 100 and a grade for every host, from the same read-only checks as System → security plus waiting security updates and container images over six months old. Click a host for its checks, worst first, each with the fix and a Library guide. Every host keeps a trend line, and a score that drops by 10 or a grade sends a notice. **Check connected hosts once a day** keeps it current.
- **New quest guide**: *Raise your fleet's security score* takes every host to an A step by step and checks each fix on the real host (new checks: no exposed databases, no Docker API on TCP, security updates, image age).
- **Security → SECRETS**: every SSH key, saved host password, `.env` vault secret, integration token, Nightgrid token and certificate in one list with its age and rotation due date, overdue first. Set how often each should change, **Mark rotated today** for anything changed elsewhere, list secrets kept outside Nightgrid by name, and for an SSH key see which hosts still accept it. A notice comes 30 days before one is due. No secret's value is read or shown.
- **New how-to**: *Rotate secrets and certificates*.

### Added: Fleet → CATALOG, DRILLS, CAPACITY, REPLAY
- **Service catalog**: every container on every server with image, version, ports, the URLs that reach it and its data folders, and whether a backup job covers them. Filter to what has no backup; export as Markdown.
- **Restore drills**: run a backup job's test restore on a schedule, or restore a Proxmox guest's newest backup into an isolated sandbox, boot it, check it and destroy it. Each run keeps its steps, a failure sends a notice, and **Report** saves the history.
- **Capacity planner**: each host's size and busiest CPU and memory (p95 over 7, 30 or 90 days) with its fullest disk. Pick workloads to see which host can take them and what a new server needs. A notice when a host stays short of memory or CPU for a week.
- **Incident replay**: step through an outage or an alert burst minute by minute with metrics, alerts, restarts, OOM kills, deploys and notices on one timeline, plus the host's warnings from that window.
- **New how-tos**: *Prove your backups restore* and *Plan capacity and replay incidents*.

## [1.0.0] - 2026-11-17

Nightgrid 1.0: the road-to-1.0 batch (journal, blueprints, sandbox clones, power & cost, Library quests, Node pairing) ends with an MCP server.

### Added: MCP server for AI clients
- **Settings → Connections → MCP server** (off by default): lets Claude Desktop, Claude Code or any MCP client on this computer ask about your homelab through Nightgrid. It listens only on 127.0.0.1 and needs a token.
- **Read tools**: hosts and their state, a host's live metrics, metric history, active and past alerts, a host's timeline, service checks, the journal, disk fill-up forecasts and a host's containers. Read-only by default.
- **Changes are opt-in**: with **Allow changes** on, a client can also start, stop or restart a container and add a journal note, and each one waits for **Allow once** or **Deny** in Nightgrid (with a notification when the window isn't in front). Unanswered requests are denied after two minutes.
- **Connect a client**: copy a ready config as HTTP JSON, as a command (`nightgrid --mcp`, which forwards to the running app and reads the token itself) or as a `claude mcp add` line. **New token** makes old configs stop working. **Recent calls** lists what clients asked for.

## [0.10.88] - 2026-11-17

### Added: Nightgrid Node pairing
- **Fleet → NODE**: where the optional Nightgrid Node agent runs. Nightgrid looks for it whenever it connects to a host (one read-only check) and uses it where it's running.
- **No gaps**: after the app was closed, the node's history fills each host's graphs, long-range charts, uptime bars, disk forecasts and timeline for the time it was away.
- **While you were away**: alerts the node recorded meanwhile land in the alerts panel, with one notification per host for those still active and none for ones that already cleared. Your alert thresholds are sent to the node, so it judges disks the way the app does.
- **Live from the node**: events arrive as they happen, and a host's metric tiles take the node's samples (every 2 s while you look at it) instead of running a script over SSH.
- **Install…, Update…, Remove…** per host: the latest release for the host's architecture is downloaded from GitHub, checked against the release's checksums, uploaded to a private temp folder and installed as root through the usual sudo prompt; or pick a binary file yourself. Remove can also delete the node's history and account.
- Per host **Use on this host**, and Settings → Features → Fleet → Nightgrid Node turns it all off. Hosts without the node work exactly as before.
- What the node project could add to make this smoother is written up in `docs/NODE_PAIRING.md`.

## [0.10.87] - 2026-11-17

### Added: Library quests
- **Verify on a host**: step-by-step guides and how-tos now have a **VERIFY ON [host] Check** row in their Objectives box. Nightgrid runs fixed, read-only checks on that host (no sudo, nothing changed) and marks each step it can verify with ✓ done, ✗ not yet (with what it found) or ? couldn't tell (for example a file only root can read).
- **Tick verified** ticks every step the host passed, so a guide's progress matches the real server instead of what you remembered to tick.
- Checks cover key-only SSH, root logins, firewall, automatic updates, Docker and the docker group, time sync, Tailscale, backup timers, swap, fail2ban, pending reboots, and named containers, services, listening ports and installed commands. Fifteen articles carry them, including *First homelab weekend*, sshd hardening, unattended upgrades, restic, Caddy, Vaultwarden, Pi-hole and Tailscale.
- DRIFT knows the last result for the open guide ("Lock it down: 3 of 5 verified on server01").

## [0.10.86] - 2026-11-17

### Added: power & cost
- **Fleet → POWER**: what each host draws and costs. Give a host its watts at idle and at full CPU (**Set watts…**, or a preset from Raspberry Pi to rack server) and Nightgrid works out its average and peak draw from the CPU history it already keeps, plus the GPU watts it measures. Totals per month and per year, over the last 7, 30 or 90 days.
- **Price per kWh** and currency at the top turn kWh into cost.
- **Idle hours**: hours of the day a host is reliably idle (under 5% CPU and almost no traffic on 85% of days) are listed with what switching it off then would save, and **Schedule…** opens System → SCHEDULES to set up a shutdown and a Wake-on-LAN.
- **UPS check**: a UPS that reports its output (`ups.realpower`, or load × rated power) is compared with the estimates for the hosts on it, so you can tell whether the profiles are close.
- Nothing new is polled. Settings → Features → Fleet → Power & cost turns it off.

## [0.10.85] - 2026-11-17

### Added: sandbox clones
- **Proxmox → SANDBOX**: make a throwaway copy of a VM or container to try an upgrade or a risky change on, from its current state or from a snapshot. **+ Sandbox** on the tab, or **Sandbox copy…** in a guest's ⋯ menu.
- The copy starts **cut off from your network**: every network card's cable is pulled, or every card moves to a bridge you pick (one with no ports keeps sandboxes on their own little network). It can't clash with the original's IP or hostname.
- PCI and USB passthrough is removed from the copy, and so are a container's bind mounts of the node's folders, so a sandbox never grabs the original's GPU or writes to its data. It never starts at boot.
- **It expires**: pick 2 hours to a week (or never). A timer on the node stops and destroys the copy on time, even while Nightgrid is closed, and only if it's still that sandbox. **Expiry** sets a new time, **Keep…** turns it into an ordinary guest, **Destroy…** removes it now.
- Sandboxes are marked with Proxmox tags, so they show up from any Nightgrid and in Proxmox's own UI. Making, extending, keeping and destroying them lands in the journal.
- Settings → Features → Proxmox → Sandboxes turns the tab off.

## [0.10.84] - 2026-11-17

### Added: lab blueprints
- **Fleet → BLUEPRINTS**: save a host's setup as one file encrypted with a passphrase: its compose stacks (compose files and `.env`, or the `.env` vault's values for them), crontab, Nightgrid timers, firewall rules and Caddy/Traefik routes. Pick every stack or some, and leave out what you don't want.
- **Rebuild from a blueprint**: open the file, pick a host (a fresh machine or any other) and **Plan**. Nightgrid reads that host and lists every step as new, already there, different or by hand; tick what to apply. Stacks are written (vault values go into the target's vault) and started, missing crontab lines are added, timers installed, allow rules added and proxy routes written, each through the same code the editors use.
- Nothing on the target is ever deleted. Changing files that already exist is off until ticked. Deny rules, default policies and turning a firewall on stay manual, so they go through System → FIREWALL's rollback.
- Saving and applying a blueprint lands in the journal.

## [0.10.83] - 2026-11-17

### Added: the homelab journal
- **Fleet → JOURNAL**: a changelog of everything changed from Nightgrid across all hosts, newest first and grouped by day: deploys, compose runs, snapshots and rollbacks, container actions, files saved in the editor, crontab and timer edits, firewall changes (and whether they were kept), Caddy and Traefik routes, `.env` vault changes (names only), package upgrades and unit actions, power actions, Proxmox changes, SSH keys pushed or removed, config baselines and integrations.
- **Line diffs** for text edits (files, crontabs, Caddyfiles), with lines that look like they hold a secret hidden. Failed changes are marked.
- **Search** across titles, details, hosts and diffs; filter by host, area, time (24 hours to everything) or failed only. **Export Markdown** saves what's shown as a changelog.
- **Your own entries** (**+ Entry**) for changes made outside Nightgrid, with a host and a time; they can be edited and deleted. Kept two years.
- **DRIFT reads the journal**: questions about a host include its changes from the last 14 days, and "what changed before it broke?" without a host gets the whole lab's. **Ask DRIFT** on an entry asks what it could have broken and how to undo it.
- Settings → Features → Fleet → Journal turns it off; nothing is recorded while it's off.

## [0.10.82] - 2026-11-17

### Added: saved networks on the network map
- **Scans know which network they're on**: each scan is matched to a saved network by its subnet and its router's MAC address. A laptop on home Wi-Fi and servers on another LAN are separate networks, and so are two places that both use `192.168.1.x`.
- **Nothing is "missing" just for being elsewhere**: devices remember the network they were last seen on, so a scan of one network (and the network watch's DEVICE MISSING alerts, and the expected map) never counts another network's devices as gone.
- **NETWORKS bar** on the Network view: rename networks, show a network's last scan without scanning, **merge** a network back after a router swap, or forget one.
- **Several networks on one map**: tick **on map** to draw another network's last scan behind its own router, in its own region. One **Internet** node sits above every router and the **tailnet** links its devices across networks, showing how the networks reach each other.

## [0.10.81] - 2026-11-17

### Changed: client devices are away, not offline
- A host marked **Client, not a server** (laptop, desktop, handheld) that stops answering now shows as **away**: no HOST OFFLINE alert, and it doesn't count as down in uptime bars, SLA reports or the status page. Tick **Alert when it goes offline** in the host editor to watch a client anyway. D-153.

## [0.10.80] - 2026-11-17

### Added: DRIFT reads your servers, an error reference and Nightgrid Node guides
- **DRIFT read-only server checks**: turn on "DRIFT may run read-only checks" per host and DRIFT looks around itself (disk, storage/RAID/ZFS, mounts, memory, processes, sensors, ports, network, services and their journals, error/kernel logs, timers, cron, logins, updates, containers and their logs, Docker stats/events/disk, allow-listed config files). Fixed read-only commands only: never sudo, secrets masked, every check is listed under "What I checked". DRIFT can ask for two more checks before answering. Host facts also include what other views know (service checks, updates, disks, backups, containers).
- **Error reference**: a new Library shelf with 1,124 Linux server errors across 20 areas (shell, disks, kernel, systemd, apt, dnf, SSH, network, firewall, TLS, Docker, Compose, web servers, databases, git, storage, NFS/SMB, users, virtualization, runtimes), each with what it means and what to do. Paste an error into DRIFT and its exact entry comes up in the checks card.
- **Nightgrid Node guides**: what Node is, installing it, choosing its grants and the CLI, plus six scenario guides (watching with the laptop closed, self-healing, UPS shutdown, guarding an internet-facing server, watching the LAN, linking several servers), each with a diagram.

### Fixed
- **DRIFT "too many tokens"** on the first question: the prompt now fits the loaded model's window (only the relevant point targets, long sections split, the conversation budgeted and trimmed in a fixed order), and the window is 12k or 16k where memory allows.

## [0.10.79] - 2026-11-17

### Added: a diagram for most of the Library
- **148 more diagrams**. Nearly every Nightgrid how-to now has one, as do the Linux skills (boot and storage stacks, SELinux, DKMS, sysctl, static IPs and more) and most app guides (game servers, media and NAS layouts, monitoring agents, home automation, backups, CI, password and secret stores).
- 267 of 321 articles now have a picture. That is 220 figures, about 1.3 MB in all.

## [0.10.78] - 2026-11-17

### Added: diagrams across the Library
- **42 new diagrams**: Nightgrid how-tos (firewall rollback, tunnels, web UI windows, VM consoles, stacks from git, the alert path, the phone status page, update and rollback, UPS shutdown), Linux skills (SSH forwards, the boot chain, LUKS, mdadm, keepalived, iSCSI, nftables hooks, zram, Quadlets, ACME, fstab, chrony, IPMI, Btrfs snapshots) and apps (Authelia, authentik, Zigbee2MQTT and Home Assistant, Frigate, Immich, Nextcloud, Jellyfin, Paperless-ngx, Syncthing, restic/borgmatic/Kopia, PBS, rclone crypt, k3s, Gluetun, Headscale, Loki, Healthchecks, mergerfs + SnapRAID, NFS).
- Existing diagrams now also appear where they fit: reverse proxies (Traefik, Caddy, NPM, nginx, proxy routes), the DNS chain (Pi-hole, AdGuard Home), the tailnet (subnet router, remote access) and more.
- 89 articles now have a picture; 72 figures, about 465 KB in all.

## [0.10.77] - 2026-11-17

### Added: more Library diagrams
- **Diagrams** for ZFS pools, a WireGuard hub, the Pi-hole → Unbound DNS chain, VLAN trunks, Cloudflare tunnels, GPU passthrough, LVM layers, NUT UPS shutdown and Prometheus scraping.
- 30 bundled figures now, about 190 KB in all.

## [0.10.76] - 2026-11-17

### Added: pictures in the Library
- **Quest briefings**: every guide opens with a briefing banner above its objectives, quest-log style.
- **Diagrams** in the articles where a picture says it faster: reverse proxies, the 3-2-1 backup rule, a tailnet with a subnet router, SSH key pairs, what a compose stack becomes, Docker networks, arr-stack hardlinks and jump hosts.
- Figures are framed like intel cards; click one to see it full size (Esc closes).
- All drawn for Nightgrid as small bundled SVGs (about 125 KB for all 21), nothing fetched. DRIFT replies and notes still never show images.

## [0.10.75] - 2026-11-17

### Added: 2 more Library articles
- **Linux skills**: move a server to a new SSD, by cloning or by copying files and reinstalling the bootloader.
- **Apps**: Dockge alongside Nightgrid, sharing the same compose folders.
- The library now holds 345 articles.

## [0.10.74] - 2026-11-17

### Added: 6 more Library articles
- **Apps**: LanCache for game downloads, netboot.xyz network boot menu, SFTPGo file transfer server, Mailpit test mail catcher, and Verdaccio private npm registry.
- **Linux skills**: reset a forgotten root or sudo password from the console.
- The library now holds 343 articles.

## [0.10.73] - 2026-11-17

### Added: 7 more Library articles
- **Guide**: a small office server (files, passwords, calendars, job board, paperwork and signing, workstation backups) for a shop of 2–15 people.
- **Linux skills**: capture traffic with tcpdump, encrypt files with age, and reach servers by `name.local` with Avahi.
- **Apps**: VictoriaMetrics long-term metrics, Radicale calendars and contacts, and the LanguageTool grammar checker.
- The library now holds 337 articles.

## [0.10.72] - 2026-11-17

### Added: 8 more Library articles
- **Linux skills**: memory limits and OOM kills (systemd and compose limits, finding what got killed, protecting key services), and changing kernel boot parameters on GRUB, grubby, systemd-boot and rpm-ostree hosts.
- **Apps**: Wallabag read-it-later, Kanboard project boards, DocuSeal document signing, OpenBao secrets manager, Penpot design and prototyping, and Kasm Workspaces browser desktops.
- The library now holds 330 articles.

## [0.10.71] - 2026-11-17

### Added: 6 more Library articles
- **Alongside Nightgrid**: a Synology NAS (SSH, SFTP, Container Manager's Docker) and an Unraid server (keys that survive reboots, compose stacks, what doesn't apply), plus an OpenWrt router with SQM.
- **Linux skills**: install Proxmox VE on a new server, and nginx as a web server and reverse proxy.
- **Apps**: Jitsi Meet video calls.
- The library now holds 322 articles.

## [0.10.70] - 2026-11-17

### Added: 9 more Library articles
- **Linux skills**: fix a full disk (including deleted-but-open files, inodes and snapshots), write nftables rules by hand with a rollback armed, keep DKMS modules (ZFS, NVIDIA) working across kernel updates, and HBA cards in IT mode for a NAS.
- **Apps**: Recyclarr, Pinchflat, Joplin Server, NocoDB and Ghost.
- The library now holds 316 articles.

## [0.10.69] - 2026-11-17

### Added: 11 more Library articles
- **Apps**: game streaming with Sunshine and Moonlight, PiKVM, UpSnap, MeTube, LibreNMS, evcc solar charging, TeslaMate, Trilium notes and WordPress.
- **Linux skills**: a floating IP with keepalived (two DNS servers behind one address), and lightweight containers with systemd-nspawn.
- The library now holds 307 articles.

## [0.10.68] - 2026-11-17

### Added: 12 more Library articles
- **Linux skills**: two-factor codes for SSH (without breaking key logins or Nightgrid), SFTP-only accounts jailed to one folder, iSCSI block storage, tracking who changed what with auditd, and the kernel settings worth changing with sysctl.
- **Apps**: Technitium DNS Server, Homarr, Matrix with Synapse, Mattermost, GoToSocial, Adminer and pgAdmin opened through Nightgrid's tunnel, and Proxmox VMs as code with OpenTofu.
- The library now holds 296 articles.

## [0.10.67] - 2026-11-17

### Added: 17 more Library articles
- **Apps**: Wazuh security monitoring, Zabbix, Graylog, Keycloak, NetBird, Seafile, Outline, HedgeDoc, Semaphore UI for Ansible, ntopng, PairDrop, Webtop (a Linux desktop opened through Nightgrid's SSH tunnel), RomM, and Valheim and Satisfactory servers.
- **Guide**: a mini PC homelab, from choosing an N100 or used business mini to BIOS settings, install, Wake-on-LAN and power draw.
- **Linux skills**: share a printer with CUPS (AirPrint for phones, Windows by address).
- The library now holds 284 articles.

## [0.10.66] - 2026-11-17

### Added: 12 more Library articles
- **Linux skills**: free disk space Docker is using (safely, volumes last), debug DNS on a server, mount Windows or NAS shares with credentials kept out of fstab, VLANs and bonded NICs, cut a home server's power draw, NVIDIA drivers for containers, organise `~/.ssh/config`, and admin scripts that fail loudly and never run twice.
- **Guide**: upgrade the homelab to 10 GbE, from measuring first to SFP+ vs RJ45, jumbo frames and SMB multichannel.
- **Apps**: Duplicati, SABnzbd and Jellystat.
- The library now holds 267 articles.

## [0.10.65] - 2026-11-17

### Added: 54 more Library articles
- **Linux skills** for the repair bench and the server room: test a drive before you trust it (SMART, long tests, badblocks), burn in a server (Memtest86+, stress-ng, MCE logs), wipe a drive properly (NVMe sanitize, ATA secure erase), recover data from a failing drive (ddrescue, testdisk, photorec), IPMI, fans and sensors, hardware inventory, LUKS encryption, fixing a server that won't boot, SSH tunnels, rsync, log rotation, finding what holds a port or file, apt and dnf, and ACME certificates.
- **Self-hosted apps**: Snipe-IT, Homebox, Grocy, PhotoPrism, Netdata, Glances, SmokePing, LibreSpeed, apt-cacher-ng, Tautulli, Komga, Tube Archivist, linkding, Miniflux, changedetection.io, Wiki.js, LLDAP, Umami, IT-Tools, Excalidraw, UrBackup, openmediavault, TrueNAS alongside Nightgrid, Incus, OPNsense in a VM, UniFi Network Application, ZeroTier, NetBox, NetAlertX, InfluxDB and Telegraf, JupyterLab, Factorio, Palworld, Mumble, a local voice assistant for Home Assistant, Music Assistant, Homebridge and Scrypted.
- **How-to**: open app web UIs inside Nightgrid (the 0.10.63 windows, the `nightgrid.webui` label, loopback-only ports).
- The library now holds 255 articles in about 0.6 MB of plain text (about 200 KB compressed).

## [0.10.64] - 2026-11-17

### Added: a much bigger Library, with topics
- **161 new articles**, all offline, on three new shelves. **How-tos** (44): one job in Nightgrid from start to finish, such as adding your first host, jump hosts, reaching servers away from home, deploying a stack, proxy routes, alerts and runbooks, backups, disks, updates, the firewall, schedules, power and UPS, the network map and tools, Proxmox, VM desktops and profiles. **Self-hosted apps** (96): install and run an app on a Linux server and watch it from Nightgrid, from Docker hosts, ZFS, Samba and restic to Caddy, Traefik, Pi-hole, Authentik, Vaultwarden, Nextcloud, Immich, Jellyfin, the *arr stack, Home Assistant, Frigate, Prometheus and Grafana, Forgejo, k3s, game servers, Ollama and more. **Linux skills** (19): systemd units, journald, fstab, LVM, static IPs, users and sudo, swap and zram, RAID, Btrfs snapshots, release upgrades, permissions, SELinux, Podman quadlets, rpm-ostree, tmux and more. Two more **Guides**: a first homelab weekend and a DIY NAS.
- **Topics** across the shelves (Getting started, Docker & containers, Networking & DNS, Security & access, Storage & backups, Monitoring & alerts, Media, Smart home, Productivity, Dev & automation, Gaming, Virtualization, Power & hardware, Linux skills): pick one on the Library's home page or in the topic list beside the search, with a count for each. Shelf chips show counts too, and both filters also narrow search results.
- **Objectives and ticks** now work in every step-by-step article (how-tos, apps, Linux skills and guides), not only the guides.
- DRIFT reads every new article and links the ones it used.
- The whole library is about 0.4 MB of plain text (about 120 KB compressed) and still loads only with the Library view or a DRIFT question.

### Fixed
- A markdown line starting with four backticks (or a fence with words after the language) no longer freezes notes, DRIFT answers or the Library.

## [0.10.63] - 2026-11-17

### Added: open a service's web UI inside Nightgrid
- **⧉** on a running container's row in Docker (and **Web UI** in its detail panel) opens the container's web interface (AMP, Portainer, Homepage, anything with a published web port) in its own Nightgrid window. Ports that aren't web pages (databases, game servers, DNS) are skipped; the `nightgrid.webui` label picks the port, scheme and path yourself.
- Clicking a service's name in Services opens it the same way; **↗** still opens it in your browser. **Browse** on a tunnel opens in a window too.
- It connects straight when this computer can reach the port, otherwise through a temporary SSH tunnel to the host, so ports bound to the host's loopback and hosts reached only through the built-in tailnet work too. It checks the page answers first and says why when it can't open (nothing listening, or an HTTPS certificate the window can't accept, with **Open in browser**).
- The window shows only that site: it has no access to Nightgrid or your hosts, and pop-ups open in your browser.

## [0.10.62] - 2026-11-16

### Added: the Library, a built-in knowledge base (M11)
- **Knowledge → Library** (`Mod+Shift+L`): every article is bundled and works offline, on three shelves. **Using Nightgrid** has one article per feature (the user guide, so it always matches the app). **Homelab basics** covers SSH keys and agents, hardening sshd, Docker and compose, the docker group, reverse proxies, Tailscale and WireGuard, backups and reading logs. **Guides** walks through whole setups: a Proxmox media server, a smart home hub, a local AI GPU server, family cloud and photos, a game server host, a dev and git deploy lab, remote family IT and a repair bench server.
- **Search** covers the full text of every article. Code blocks have **Copy** and **Send to terminal**, view links open that view, and articles link to each other, with **← Back**.
- **Guides read like a quest log**: each part is an objective with its own progress, the steps are checkboxes, and your ticks are kept on this computer (**Reset** starts over).
- **?** beside every view's title opens that view's article. The command palette lists every article as **Help: …**.
- **DRIFT reads the library** too, and its answers link the articles they used.

## [0.10.61] - 2026-11-16

### Added: security checks and a network watch (network map, part 5)
- **SECURITY** above the map lists what the scan shows: router port forwards (UPnP) to risky services such as RDP, SSH, SMB, databases or a Docker API (red), UPnP being on at all, a public address on an interface, and a second DHCP server on the LAN (a rogue or forgotten one).
- **Check web security** reads one page or certificate on the common web ports of every device found (up to 64): expired, nearly expired or self-signed certificates, admin pages of gear that ships with default passwords (routers, printers, cameras, NAS), and login forms or basic auth over plain HTTP. It never tries a password. Devices with findings get a ⚠ on the map; each finding opens in Tools.
- **Watch the network**: a scheduled scan (every 15 minutes to once a day) from the chosen scanner keeps the device list current and raises alerts: **NEW DEVICE** (an untrusted device first seen in the last day), **DEVICE MISSING** (on the expected map but silent in a subnet that was scanned) and **NETWORK RISK** (the findings above). Each has its own switch; DRIFT's triage knows all three.

## [0.10.60] - 2026-11-16

### Added: more of the real topology (network map, part 4)
- **Switches from LLDP.** When the scanning host runs `lldpd` (or systemd-networkd with LLDP on), the map shows the switch it's plugged into, with the port, between the router and the host. Drop other devices onto the switch to connect them through it.
- **Routers on the way to routed ranges.** A scan with routed ranges also traces the path to each range; the last router before the range appears on the map with the range's devices under it, and the range's region says which router it's reached through.
- **Tailscale direct or relayed.** Tailnet peers say whether the connection is direct (peer to peer) or goes through a Tailscale relay, in their details and in the tailnet list.

## [0.10.59] - 2026-11-16

### Added: service scan (network map, part 4)
- **Services** on a selected device or host on the map: **Scan services** connects to 46 common ports from the machine that scanned and reads what answers on each (SSH version, web server and status, mail, FTP, VNC, Redis and so on, over TLS when needed). Nothing is logged in to or changed.
- **Risky services are flagged**: Telnet, a Docker API without TLS and SSH protocol 1 in red; FTP, unauthenticated-by-default databases and caches (Redis, MongoDB, Elasticsearch, memcached), MQTT, VNC and Remote Desktop in amber; SMB and rpcbind as a reminder. The device gets a ⚠ on the map.
- **Add as service** on any web port opens the service editor with the name and address filled in (and "allow untrusted certificate" for HTTPS), so it's watched under Services.
- **Network → Tools → Services** runs the same scan on any host and port list.

## [0.10.58] - 2026-11-16

### Added: network tools (network map, part 3)
- **Network → Tools** (`Mod+Shift+N`): **Ping**, **Path** (tracepath or traceroute), **DNS** lookups (any common record type, optionally against a chosen server), **Ports** (which TCP ports accept a connection, up to 64 at once), **HTTP / TLS** (status, server, redirect, and the certificate's issuer, expiry, whether it's self-signed and whether that machine trusts it), **Wake** (Wake-on-LAN to any MAC), the **Neighbours** table with makers, and open **Connections**.
- **Run from anywhere you have**: every tool runs on the machine picked under **Run from**, a host over SSH or this computer, with that machine's own commands, so you test from where the problem is. Results read as facts with colours (answers, loss, open ports, certificate days left), a table where it fits, and the raw output.
- **Subnet** calculator and **MAC lookup** work offline.
- On the map, a selected device, router or host has **Tools** buttons (Ping, Ports, Web, Path, DNS, Wake) that open the tool with its address filled in, run from the machine that scanned it.
- DRIFT knows each tool and can point at it.

## [0.10.57] - 2026-11-16

### Added: the network map as a real topology (network map, part 2)
- **A graph, not columns.** The map now draws the internet above your router, the router above what connects through it, and the tailnet beside it, laid out automatically on first draw. Subnets are drawn as regions (**LAN**, each **ROUTED** range, **TAILNET**). Each node has a type icon; links are styled by kind (internet-facing, LAN, tailnet, routed range, the router's uplink) and nodes by state (internet-facing, new, offline, not seen, missing).
- **Pan, zoom and place.** Drag the background to pan, use the wheel or **+ / − / FIT** to zoom. Drag a node to put it where you want: the position is remembered, and devices you haven't placed move with what they connect through.
- **Say what connects through what.** Drop a device onto a router, access point, switch or host (or pick it under **Connected through** in the details) and the map links it there, e.g. a smart lamp to the access point it's on. Your choice wins over what the scan implies and never forms a loop. Label a device as an access point or switch to make it something others can connect through.
- **The expected map.** **Save as expected** remembers the map as it is now. Later scans show expected devices that didn't answer as dashed **MISSING** ghosts in their place, mark devices that aren't in it with **+**, and say in the scan line whether the map matches. **Add it** puts a new device in the picture; **Remove from expected** takes one out; **Clear expected** drops the picture.
- **List mode** keeps the old four columns; **Auto layout** forgets hand-placed positions (connections you set stay). Positions, connections and the expected picture are part of profile exports.

## [0.10.56] - 2026-11-16

### Fixed
- **macOS: no more crash when Nightgrid restarts** after installing an update or importing a profile, or when it quits, while DRIFT's model is loaded. The model is now freed first; llama.cpp's Metal backend aborted when it was still in memory at exit.
- **macOS: network scans with a sweep finish in seconds again.** `nc` on macOS ignored the one-second limit for addresses that never answer, so a sweep could take minutes.

### Added: who's on the network (network map, part 1)
- **Names from more places.** A scan now also reads reverse DNS for every address it saw, the scanning host's own DHCP leases (dnsmasq, Pi-hole, ISC dhcpd) when it hands out addresses, NetBIOS names (when Samba's `nmblookup` is there) and UPnP descriptions (friendly name, maker and model, read from the devices with `python3`).
- **What each device probably is.** Phones, laptops, TVs, speakers, printers, cameras, NAS boxes, smart-home gear, consoles, access points and the router are guessed from the maker, names, announced services and open ports. The guess shows with a "?" on the map and fills the label's type; a label you save always wins.
- **Inventory.** Every device a scan sees is remembered with when it was first and last seen. Devices seen for the first time show **NEW** on the map and under **New this scan**; known devices in the swept range that didn't answer are listed under **Missing**. **Trust** marks a device as expected; **Forget** removes it.
- **Routed ranges.** Type subnets the scanner can reach through a router, VPN or Tailscale subnet route (`10.20.0.0/24`, up to 4, /24 to /30) and the scan sweeps them too, from whichever host or computer you scan from. MAC addresses, makers and local names don't cross a router, so those devices carry less detail and say so.

## [0.10.55] - 2026-11-16

### Added: choose DRIFT's model
- Settings → DRIFT → **Model** offers three sizes: **Light** (Qwen3 4B Instruct 2507, 2.5 GB, the one so far), **Balanced** (Qwen3 8B, 5.0 GB) and **Best** (Qwen3 30B-A3B Instruct 2507, 18.6 GB, a mixture-of-experts model that answers about as fast as a small one).
- Nightgrid reads this computer's memory, CPU and GPU (locally: `sysctl`, `/proc`, PowerShell and `nvidia-smi`), marks each model **FITS**, **TIGHT** or **WON'T FIT** with the memory it needs, and recommends the biggest one that fits comfortably. 16 GB Macs get Balanced; Best needs a 32 GB Mac (or about 48 GB of RAM on Windows and Linux, where this build runs the model on the CPU).
- Models download on demand from pinned files checked against their SHA-256. The model in use keeps answering while another downloads, and DRIFT switches when it's ready. **Use this model** switches between downloaded ones; **Delete…** frees the space.
- Qwen3 8B is asked to answer directly and any thinking block it writes is dropped.

## [0.10.54] - 2026-11-16

### Changed: Settings in tabs
- Settings is split into tabs by topic instead of one long GENERAL page: **GENERAL** (tray and window, app updates, diagnostics), **APPEARANCE** (theme, custom colours, scale and effects), **TERMINAL** (terminal options, keyboard shortcuts), **MONITORING** (metrics, service checks), **ALERTS** (alert rules, log rules and mutes, dependencies, notifications), **CONNECTIONS** (built-in Tailscale, SSH keys), **DRIFT** (the helper, its model, tours), **DATA** (profile export and import, phone status page) and **FEATURES**.
- DRIFT's spotlights and tours open the tab that holds the control they point at. **Alert rules…** in the Alerts panel opens the ALERTS tab.

## [0.10.53] - 2026-11-16

### Fixed: log alerts that weren't errors
- Log levels decide. WARN, INFO and DEBUG lines are never errors, even when the logger's class is called `GlobalExceptionHandler`. Only ERROR, FATAL, CRITICAL and panic lines count, or wording like "uncaught exception" in lines without a level.
- A stack trace or multi-line error is one event. Indented lines, `at …`, `status:` and `code:` belong to the line above, so "(+1 more new)" from one trace is gone.
- HTTP 404 / NOT_FOUND is noise by default. It only alerts when it spikes well above that container's usual rate.
- "New" is remembered. Each pattern's fingerprint (times, thread ids, numbers, UUIDs and request ids blanked; stored as a hash) is kept in the database. A pattern seen before a restart isn't new after it, and one first seen over a day ago is never new again.
- New errors need to repeat. A pattern alerts at 3 times in 10 minutes by default; fewer become a **note** in the Alerts panel. FATAL/panic, or a container that's unhealthy or restarting, alerts at once.
- Grace after restarts. Errors in the first 5 minutes after a container starts, or after a reverse proxy on its host starts (Caddy, Traefik, nginx, HAProxy, Nginx Proxy Manager…), are notes, not alerts. FATAL still alerts.
- Mutes. **Mute pattern** on a log alert or note mutes that pattern for its container. Settings → Alerts lists mutes and takes regex mutes per container. Muted lines and 404s left out are counted in the Alerts panel.
- Each new pattern is its own alert, so muting one never hides another.
- Exited, crash-looping and unhealthy containers still alert immediately (unchanged, now covered by tests next to the new rules).
- New settings (Settings → Alerts, under Log anomalies): **Alert after** (1–10×), **Within** (10–120 min), **Grace after restart** (off–15 min), **404s are noise**, **Muted patterns**.

### Fixed: DRIFT guessing in triage
- **What broke?** now sends DRIFT the attached lines as the log rules judge them (one event per trace, likely harmless or not, and why), the container's real state (running, health, uptime, restart count), a verdict, and commands that work as written (`docker inspect …`, `docker logs --since 30m …`). The generic "usual causes" are gone for log alerts.
- DRIFT is told to answer only from those facts, never to invent paths, variables or settings, to say "unknown" instead, never to use placeholder commands, and not to call a running, healthy container failed.
- Numbered lists in DRIFT's replies count up again (1, 2, 3) when the model splits them with bullets or code.

### Added: Export diagnostics
- **Settings → Diagnostics → Export diagnostics…** (also in the command palette) saves one text file: version, platform, settings, active and recent alerts and the newest app logs. Passwords, tokens and secret-looking lines are removed.

## [0.10.52] - 2026-11-16

### Changed: Overview widgets say more
- **SERVICES** lists every service with a status dot and its latency, or why it's down (HTTP code or the first words of the error), problems first. Hover a row for the full reason.
- **UPDATES** shows one line per host: pending packages, security updates and a pending reboot.
- **DISKS** shows each host's worst finding (a degraded pool, a failing SMART disk, a mount about to fill) or a short all-clear, plus a bar for its fullest mount.
- **BACKUPS** lists every job with its host, tool, the age of its last good run and its state, problems first.
- **ALERTS**, with nothing active, lists the last week's resolved incidents: when, what, where and how long it lasted.
- UPDATES, DISKS and BACKUPS fetch on their own while they're on screen: updates every 3 hours, disks hourly, backups every 30 minutes. They only read servers that aren't known to be offline, never clients, and never ask for a password, sudo or a Tailscale approval. **Check all** still reads every host.
- Widgets whose feature is off in **Settings → Features** are hidden from the Overview and the add menu, and don't fetch. The rest close up over the gap; the layout itself is kept for when the feature comes back.

## [0.10.51] - 2026-11-16

### Added: Tailscale SSH
- Hosts with Tailscale SSH log in without a key. Nightgrid now asks the server with "none" first, like OpenSSH does, so the tailnet's policy decides.
- In the policy's **check** mode, a card shows **Approve in browser** with the login link while the host connects. The terminal says *Waiting for approval*, and the connection goes ahead once you approve. This works over a system Tailscale and the built-in one.
- Logging in is never left hanging: after 5 minutes without an answer it stops with a message. If the tailnet refuses, the error names the policy's `ssh` rules and `tailscale set --ssh`.
- The host editor says when an address is a tailnet one and that Tailscale SSH needs no key.

### Fixed: Services showing sites as down
- Checks connect like a browser (Happy Eyeballs). They try a name's IPv6 and IPv4 addresses side by side instead of one after another. On a network where IPv6 doesn't get through, public sites (login.tailscale.com, say) timed out before IPv4 was ever tried. SSH, tunnels and the rest of the app dial the same way.
- A check that can't connect tries once more 2 seconds later before it counts as down. One lost connection no longer turns a site red or starts an alert's grace period.
- Failures say more:
  - which server answered (`Server` header) and the address it was reached at, or that it went through the built-in tailnet
  - the start of the error page
  - for 502 and 504, that the proxy couldn't reach the app behind it
  - for timeouts, the addresses that were tried

## [0.10.50] - 2026-11-16

### Changed: a shorter sidebar
- Related views share one sidebar row and switch with tabs next to the view title: **System** holds One host and Fleet, **Network** holds the map and Tunnels, and **VMs** holds Proxmox and Desktops. A row with more than one view shows **+1**, and opens the view you used last in it.
- Rows are more compact, and the shortcut shows only when you point at a row, focus it, or it's the active one.
- Settings moved to the foot of the sidebar, next to the collapse button.
- **Notes is now Knowledge.** Same notes and runbooks, same `Mod+9`; the built-in guides (M11) will live here too.
- DRIFT knows the new layout: it names places the way the sidebar shows them ("System → Fleet"), its **Show me** and tours point at the right row and open the right tab, it can point at the tabs beside a title, and the Getting around tour shows Settings at the bottom and the Features tab.
- Every shortcut still works (`Mod+6` Tunnels, `Mod+Shift+U` Fleet, `Mod+Shift+P` Proxmox, `Mod+Shift+M` Desktops). Turning a view off in Settings → Features drops its tab, and a row with one view left shows that view's own name.

## [0.10.49] - 2026-11-16

### Changed: picking hosts
- Hosts have a role. Tick **Client, not a server** in the host editor for a laptop, desktop or handheld you also manage (an ROG Ally, say). Views never pick a client by default. You can still choose one, and if every host is a client one is used.
- Views remember the host you last picked in them, on this machine. A view you haven't picked a host in yet starts on the one you picked last anywhere, else the first server (favorites first). This covers Docker, System, Files and the host fields in new tunnels, snippets, UPS watches, backup jobs, command widgets, speed tests and imports.
- DRIFT's host facts say when a host is a client.

### Changed: network map scans from the map
- The **Scan from** dropdown is gone. Select a node on the map and the scan runs from it; the button says **Scan from <name>**. Nothing is preselected.
- Nodes that can run a scan are marked **◉**: this computer, and saved hosts that answer. Other devices, and hosts that are offline, can't scan. **This computer** is now always on the map, so you can scan from it without a dropdown.

## [0.10.48] - 2026-11-16

### Added: Settings → Features
- Settings has two tabs now: **GENERAL** (everything that was there) and **FEATURES**.
- **FEATURES** turns each view on or off: Terminal, Files, Docker, Services, Tunnels, System, Fleet, Desktops, Proxmox, Network map, Automation, Notes and the DRIFT helper. It also covers the parts inside them: Expiry watch under Services, and each Fleet tab (Updates, Disks, Backups, UPS, Uptime, Log search, Access audit, Speed tests).
- Presets: **Simple** (terminal, files, Docker, services and System), **Standard** (everything but Proxmox, VM screens, UPS, log search, access audit and speed tests) and **Everything**. Any other mix shows as custom.
- A feature that's off is gone from the sidebar, the command palette, its shortcut, the controller's view switching and DRIFT's answers. A shortcut or link to it opens Features with **Turn on and open**.
- Off also means no work in the background: no container-down polling (Docker), no SMART or pool scans (Disks), no backup checks, no UPS reads, no saved-search or log anomaly scans, no uptime recording, no service checks, no expiry re-reads, no auto-start tunnels, and the DRIFT model is never loaded.
- Nothing is deleted, and turning a feature back on takes effect at once, without a restart. The choice travels in profile exports.

### Changed
- Views load on first use. The app's startup JavaScript went from 1.8 MB to 1.2 MB (548 KB → 378 KB gzipped), and views that are off are never loaded. With Simple, opening every view it has loads 1.5 MB, against 1.8 MB with Everything.

## [0.10.47] - 2026-11-16

### Added: Proxmox VE, part 6: firewall (M10r)
- New **FIREWALL** tab in Proxmox with a scope picker: **Datacenter**, the connected **node**, or a **security group**. Each guest's window gains its own **FIREWALL** tab.
- Turn the firewall on or off and set the in/out policies (datacenter and guests). Rules are listed in order, and the first match wins. You can add a rule (on top), edit it, turn it on or off, move it up or down, or delete it. Each rule has a direction, action or security group, macro, source and destination (with aliases and `+ipsets` suggested), protocol, ports, interface, log level and comment.
- At datacenter level you can add and delete security groups (and edit their rules) and aliases. IP sets are listed for use in rules.
- Datacenter, node and group changes can lock you out, so they run under a rollback armed on the node. The first change saves the firewall files, and further changes join the same session. Press **Keep** within 90 seconds or the node restores its firewall by itself. Turning the firewall on asks first and explains Proxmox's management exception (local network only).
- Every edit carries Proxmox's digest, so a list changed elsewhere in the meantime is refused instead of overwritten.

## [0.10.46] - 2026-11-16

### Added: Proxmox VE, part 5: node network (M10r)
- New **NETWORK** tab in Proxmox: the connected node's bridges, bonds, VLANs and NICs with state, ports or slaves, addresses, gateway and comments.
- **+ Bridge**, **+ Bond** and **+ VLAN** add interfaces, and **Edit…** and **Remove…** change them. Ports already used by another bridge or bond are marked. As in Proxmox, every change is only staged: the tab shows the staged diff of `/etc/network/interfaces`, with **Revert** and **Apply…**.
- **Apply…** first arms a rollback on the node, then reloads the network. Unless you press **Keep** within 90 seconds, the node restores the previous config by itself. A change that cuts off this computer, or the node's own address, undoes itself. **Roll back now** restores the old config at once.

## [0.10.45] - 2026-11-16

### Added: Proxmox VE, part 4: hardware and options (M10r)
- A guest's window gains **HARDWARE**. Change cores, sockets, CPU type, memory and the balloon minimum (VMs) or swap (containers). Changes the running guest can't take are shown as waiting for the next restart, with a **Reboot…** button.
- **Disks**: **Grow…** any disk by some GB, **Add** a SCSI, VirtIO or SATA disk (or a mount point with a path in a container), **Detach…** a disk (it stays on the storage as unused), **Attach** an unused one again, or **Delete…** it (type its key). Change the disc in a CD/DVD drive from the node's ISO list.
- **Network**: add, edit and remove network devices: model, bridge, VLAN and firewall, plus address and gateway for containers. Editing keeps the MAC address.
- **Passthrough** (VMs): list the node's PCI and USB devices, pass one through (all functions and PCIe by default on q35), and see which other devices share its IOMMU group. A warning shows when IOMMU looks off.
- **Options**: start at boot, boot order and delay, protection, tags and notes.
- Every change is sent with the configuration's digest, so if someone changed the guest in the meantime Proxmox refuses it and the tab rereads it. Each change lands on the host's timeline.

## [0.10.44] - 2026-11-16

### Added: Proxmox VE, part 3: create, clone, templates and ISOs (M10r)
- **Proxmox → GUESTS** gains **+ VM** and **+ Container**. A new VM takes an id (the next free one is filled in), name, cores, memory, disk size and storage, an install ISO, a bridge with optional VLAN, OS type and UEFI. It gets a VirtIO SCSI disk with discard and IO thread, VirtIO networking and the QEMU guest agent, and Windows 11 also gets UEFI and a TPM.
- A new container takes a template, cores, memory, swap, disk, bridge and VLAN, DHCP or a static address with gateway, and **your SSH public keys**, which are required: Nightgrid never sets a root password. Unprivileged and nesting are on by default.
- **⋯** on a guest adds **Clone…** (full, or linked from a template), **Convert to template…** and **Destroy…**. Destroy needs the guest's name typed and also removes its disks; backups stay.
- **STORAGE** lists the node's ISO images and container templates. **Download ISO…** has the node fetch an installer from a URL itself, with the file name taken from the URL and an optional SHA-256 check. **Get CT template…** lists Proxmox's template catalogue with a filter and downloads one.

## [0.10.43] - 2026-11-16

### Added: Proxmox VE, part 2: snapshots and backups (M10r)
- Click a guest's name in **Proxmox → GUESTS** to open it. **SNAPSHOTS** shows its snapshots as a tree, marks the one it currently runs from, and shows which ones hold RAM. **Take snapshot** suggests a name, takes an optional note and can save RAM for running VMs. **Roll back…** and **Delete…** ask first, naming the guest, node and snapshot.
- The guest's **BACKUPS** tab: **Back up now** to any backup storage on its node, in snapshot, suspend or stop mode, with a notes template. It also lists every backup of the guest with age, size, notes and verify state. From there you can **Protect** or unprotect a backup, **Restore as new…** (the next free id, disks on a storage of your choice, optional start), **Restore over…** (you type the guest's id first) and **Delete…**.
- New **BACKUPS** tab: the cluster's backup jobs (schedule, guests, storage, mode, next run, disabled jobs) and the guests no job covers. Each guest's latest backup is listed worst first: red when it has none or the newest is older than 7 days, orange after 2 days.
- Snapshot, rollback, delete, backup and restore steps follow their Proxmox task to the end. Closing the task window keeps following it in the background, and they're recorded on the host's timeline.

## [0.10.42] - 2026-11-16

### Added: Proxmox VE, part 1: cluster, guests, storage and tasks (M10r)
- New **Proxmox** view (`Mod+Shift+P`). Pick the host that runs Proxmox VE and Nightgrid reads its whole cluster through `pvesh` over SSH as root (the sudo prompt asks when needed). No API token, open port or web UI needed.
- **SUMMARY**: every node with CPU, memory and root disk, uptime and running guests, plus cluster name and quorum. The node you're connected to also shows its CPU model, kernel, load and boot mode.
- **GUESTS**: every VM and container with state, CPU, memory, uptime, tags, locks, HA state and templates. Filter by text, `#tag` or node. **Start**, **Shut down** and **Resume** are one click. Reboot, Pause and Force stop sit under **⋯** and ask first, naming the guest and node. **Screen** opens a VM's display in Desktops and **Console** opens `qm terminal` / `pct enter` in a terminal tab, for guests on the connected node.
- **STORAGE**: each storage with type, content, nodes and usage. Shared storage is listed once.
- **TASKS**: recent cluster tasks (backups, starts, clones…) with their result. Click one for its log. A task Nightgrid starts opens its log and follows it until it ends. Power actions go to the host's timeline.

## [0.10.41] - 2026-11-16

### Added: more signals, part 3: custom command widgets (M10x)
- **Overview → Edit → + Widget → COMMAND…**: any read-only command on one of your hosts becomes a dashboard tile. Pick how to read the value (first number, a regex group, line count, exit code or the first line of text), an optional unit, warn and critical thresholds (or "low is bad" for things like free space) and a refresh interval from 10 s to 24 h.
- **Try it** runs the command once in the editor and shows the parsed value and the raw output before you save. Ready-made starting points: root disk use, load, logged-in users, failed units, running containers, ZFS pool state, a port listening.
- Tiles colour by their thresholds and show the host, when it last ran and the refresh interval, with **Run now**, **Output** and **Edit…**. They only run while a dashboard shows them, and pause while the window is hidden.
- Widgets refresh on their own, so commands that change something (rm, kill, prune, down, reboot, overwrite with `>`…) and sudo are refused. Saved widgets travel in backups and profiles; removing a tile keeps the widget under **YOUR COMMANDS** in the add menu, deleting it removes it everywhere.

## [0.10.40] - 2026-11-16

### Added: more signals, part 2: network tests between hosts (M10x)
- **Fleet → SPEED**: pick two hosts and **Run test**. Nightgrid measures throughput both ways with `iperf3` and latency with `ping`, from one host straight to the other (5, 10 or 20 seconds each way).
- Results are kept per pair with a throughput history line, so you can compare a link with itself. About 100 Mbit/s on a link is flagged as a possible Fast Ethernet negotiation (a bad cable or port), and a run well below the pair's usual speed is called out. **Again** repeats a test, **Clear…** forgets a pair's results.
- The target runs an iperf3 server only during the test; it stops itself after two minutes at most. Both hosts need `iperf3` installed, and the target's firewall must allow port 5201 from the source.

## [0.10.39] - 2026-11-16

### Added: more signals, part 1: GPU history (M10x)
- The monitor's **GPU** chart now shows VRAM use (the fullest GPU) next to how busy it is, **TEMP** adds the hottest GPU next to the CPU, and a new **GPU W** chart shows the power all GPUs draw. All of it is kept like the other metrics: a year of hourly points, with zoom.
- New Overview widget **GPUS** (Edit → Add): every GPU across your hosts with busy %, VRAM, temperature and power; click one to open its host's monitor.

## [0.10.38] - 2026-11-16

### Added: deploy & secrets, part 2: stacks from git (M10w)
- **Docker → Stacks → From git…** clones a repository onto the host (into a folder you choose, default `~/stacks/<repo>`) and tracks one stack folder inside it, so one repository can hold several stacks. An existing checkout of the same repository is adopted as it is.
- Git runs on the host with the host's own credentials (a deploy key or a credential helper there); Nightgrid stores no token. Errors such as a missing deploy key or an unknown host key say what to do.
- **Check** fetches the branch and shows how many new commits touch the stack, their messages, and whether files were edited on the host.
- **Review & deploy…** shows the new commits and the diff for the stack's folder. **Deploy and up** saves a stack snapshot, moves the checkout to exactly that commit, then brings the stack up. **Roll back to …** returns to the previous deploy the same way.
- **Stop tracking…** forgets the repository; the files and the stack stay on the host.

## [0.10.37] - 2026-11-16

### Added: deploy & secrets, part 1: .env vault (M10w)
- **Docker → Stacks → Vault** keeps a stack's environment variables encrypted inside Nightgrid, with the key in the system keychain. Values that look like secrets (passwords, tokens, keys) are masked until you click **Reveal**.
- Add a variable by name and value, or paste `NAME=value` into the name field. **Edit** changes a value or the masked flag, the arrows set the order in the file, and **Delete…** asks first, naming the variable.
- **Import from host .env** reads the stack's current `.env` into the vault.
- **Deploy .env…** compares the vault with the host's file and lists which names are added, changed or removed (never the values), with a warning for names that would be dropped. **Write .env to <host>** saves a stack snapshot first, then writes the file readable only by its owner. **Up now** recreates the stack with the new values.
- The vault travels in encrypted profile exports when you include secrets; on another computer the import brings the key along. Variables written with a key this computer doesn't have show as LOCKED.

## [0.10.36] - 2026-11-16

### Added: host config editors, part 4: Pi-hole and AdGuard Home (M10v)
- New integrations: **Pi-hole** (v6) and **AdGuard Home** (Services → Add integration). The card shows queries, blocked count, clients and the top blocked domains; **Turn on** appears while blocking is paused.
- **Manage…** opens the dashboard. It shows queries, blocked share, active clients and the domains on lists. **Pause…** turns blocking off for 5 minutes, 30 minutes, 1 hour or until you turn it back on (after a confirmation naming the server). It also shows the top blocked domains.
- **Local DNS records** (AdGuard: rewrites): add a name → IP, or remove one. AdGuard also takes wildcards like `*.apps.lan` and a name as the answer.
- **Lists**: block and allow lists with entry counts and when they were last downloaded. Tick to turn a list on or off. **Remove…** a list, add one by URL, and **Update gravity** / **Update lists** to download them again.
- The password stays in the keychain. Pi-hole sessions are closed after every read, so the dashboard never uses up Pi-hole's session slots.

## [0.10.35] - 2026-11-16

### Added: host config editors, part 3: reverse proxy route editor (M10v)
- **Docker → Routes → Edit routes…**: change where your reverse proxy sends each domain, for Caddy (a Caddy container or Caddy installed on the host) and Traefik.
- **Caddy**: every site in the Caddyfile with its upstreams. **Add site…** (domains and an upstream such as `app:8080`), **Change…** a site's upstream, **Remove…** a site, or **Edit as text** for anything else. Every change shows a diff first (secrets masked until you reveal them). On save, Caddy checks the new file (`caddy validate`) and loads it (`caddy reload`); if it won't load, the old file is put back. A copy of the previous Caddyfile stays in `.nightgrid-backups` next to it. A change is refused if the Caddyfile was edited elsewhere since it was opened.
- **Traefik**: add, edit and remove routes (name, domains, upstream, entry points, TLS and certificate resolver from Traefik's own configuration). They are written to `nightgrid.yml` in Traefik's file provider directory, which Traefik picks up by itself; routes from container labels stay as they are. Without a file provider directory the editor says which flag to add.
- The Routes list now includes the Traefik routes Nightgrid keeps.

## [0.10.34] - 2026-11-16

### Added: host config editors, part 2: firewall manager (M10v)
- **System → FIREWALL**: see whether the host uses ufw, firewalld or plain nftables/iptables, whether it is on, and its rules. ufw shows its rules (with comments) and default incoming policy; firewalld shows each active zone with its services, ports and sources; the raw `nft list ruleset` (or `iptables-save`) is always one click away.
- Change ufw: add a rule (allow, limit, deny or reject, a port or range, tcp/udp/any, from an address or network, a comment), **Delete…** a rule, set the incoming default or turn ufw on or off. Change firewalld: add or remove a service, a port or an "accept everything from" source in a zone. nftables and iptables stay read-only.
- **Every change is applied behind a rollback**: the host saves the current rules and sets a timer (1, 2 or 5 minutes) that puts them back unless you press **Keep**, even if Nightgrid can't reach the host any more. After the change Nightgrid opens a fresh SSH connection to show you can still get in; **Test again** repeats it, **Undo now** restores the old rules at once.
- A change that could lock you out, such as deleting the rule that lets SSH in or turning on a firewall without it, says so and asks you to type the host name.

## [0.10.33] - 2026-11-16

### Added: host config editors, part 1: cron and timer editor (M10v)
- **System → Schedules → Crontab…**: edit your own crontab, root's or another user's (through sudo). Tick a job to turn it on or off (an off job stays in the file as a `#nightgrid-off` line), **Edit** or **Delete…** it, or add a new one with a schedule (common ones in a list) shown in plain words as you type. Every change is checked first: a broken schedule, a second line or an unescaped `%` (which cron turns into a line break) is refused with the reason. A change is refused if the crontab was edited elsewhere since it was opened, and the previous crontab is saved on the host in `~/.nightgrid/cron-backups` (the last 20).
- **+ Timer**: run a command on a schedule as a systemd timer: a name, the command, an `OnCalendar` schedule (common ones in a list; **Check** asks systemd on the host what it means and when it next runs), the user to run as and whether to catch up on runs missed while the host was off. It becomes a `nightgrid-job-…` service and timer on the host; **Edit** on its row changes it, **Remove…** deletes it.
- Every timer row now has **Run now** (starts its service once) and **Enable** / **Disable**.

## [0.10.32] - 2026-11-16

### Added: logs & access, part 2: login & access audit (M10u)
- **Fleet → ACCESS**: one row per host with SSH logins and failed attempts over the last 7 days, addresses banned right now, users with a shell, authorized keys and the most serious finding. **Audit all** reads every host three at a time; click a host for the full report.
- The report: **logins** (when, who, from where, and which key, named when it is one of your own keys from Settings → SSH keys), **failed attempts by address** with the user names tried, **fail2ban** jails and **CrowdSec** decisions, **users with a shell** (sudo and service accounts marked, keys per user), **sudo rules**, every user's **authorized keys**, the **sshd** settings that matter (effective values from `sshd -T` when possible) and recent sessions from `last`.
- **Findings** pick out what needs a look: another uid 0 account, root password logins allowed, empty passwords, password logins on while attempts fail and nothing bans them, password logins that happened, many failures without fail2ban or CrowdSec, and passwordless sudo for everything.
- Read as root when that needs no prompt (root login, passwordless sudo or a remembered sudo password); otherwise the report says what it couldn't see and **Read as root…** asks for the password. Logins come from the journal or `/var/log/auth.log` / `/var/log/secure`. Nothing is installed on hosts and nothing is stored: audit again to refresh. **Explain** sends the report to DRIFT.
- Fixed a test that could fail on a busy machine (a freed port reused by another test).

## [0.10.31] - 2026-11-16

### Added: logs & access, part 1: cross-host log search (M10u)
- **Fleet → LOGS**: search the systemd journal and container logs of several hosts at once and read the matches as one stream, newest first. Plain text or a regular expression (`.*`), match case (`Aa`), look-back from 15 minutes to 7 days, all hosts or chosen ones, journal, containers or both. **More filters** narrows to a unit, a container name or a journal priority. Matches are highlighted; chips filter by source, and every line names its host and where it came from.
- Hosts that aren't connected are connected for the search; a host that fails or times out is listed with its error while the others still show. Plain text is narrowed on the host with `grep -F`, so only matching lines come back; at most 2000 lines are shown.
- Lines that look like they carry a password, token or key are shown as a hidden-line notice instead of their text (the same filter as history search).
- **Saved searches**: save a search by name and load it with one click. Turn on **◉ alert** and the search runs every 5 minutes on connected hosts; new matching lines raise a **LOG MATCH** alert per host with the count and the newest line. Saved searches travel in profile exports.
- **Explain** sends the matches to DRIFT.

## [0.10.30] - 2026-11-16

### Added: long-term history, parts 2 and 3: a year of metrics and monthly reports (M10t)
- **A year of metrics**: hourly averages are now kept for 400 days next to the 1-minute points and 15-minute averages, and the host monitor offers **90D** and **1Y** ranges (hourly averages beyond a month). Existing 15-minute history is folded into hourly averages on update, so the long ranges start with what is already stored.
- **Zoom**: drag across a history chart to zoom into that span (finer points are loaded when the span is short enough), **Reset zoom** or a double-click to go back. The chart says which averages it shows.
- **Monthly uptime report** (Fleet → UPTIME → **Monthly report…**): pick a month (the last 13) and a target (99% to 99.99%) to see availability, downtime, outages and the longest one per host and service, and whether each met the target. **Export HTML…** saves a self-contained page with the summary, the table and every outage with its first error, styled for screen and print (save as PDF from the browser).

## [0.10.29] - 2026-11-16

### Added: long-term history, part 1: uptime (M10t)
- **Fleet → UPTIME**: one bar per day for every host and checked service over 30 or 90 days, coloured by availability (green from 99.9%, yellow from 99%, red below, grey with no data), with the availability and total downtime for the range. Hover a bar for that day; click a row for its outages (when, how long, why) with the count, total and longest.
- Uptime is recorded on every alerts round (15 s) while Nightgrid runs, also with the window closed and with alerts turned off: hosts from monitoring, services from their checks. Availability is up out of the time actually watched, and a host that is asleep, suspended or rebooting on purpose isn't counted as down. History is kept 400 days and stays on this computer (not in profile exports).
- The **phone status page** and static export show the 90-day bars under every host and service.

## [0.10.28] - 2026-11-16

### Added: smarter alerts, part 3: UPS shutdown (M10s)
- **Fleet → UPS**: watch a UPS through NUT. Pick the host it's plugged into (or any host that can reach a NUT server) and its NUT name, such as `ups@localhost`; **Test** reads it once. Each UPS shows its state, charge, runtime, load and input voltage, read with `upsc` every 30 s while the host is connected. Nothing is installed: the host needs NUT already (or a NAS's built-in NUT server).
- **ON BATTERY** alerts: running on battery (warning), battery low or runtime under the limit (critical), and the UPS's own "replace battery" flag.
- **Group shutdown** (optional): after a set time on battery, when the runtime drops under a limit, or when the UPS reports low battery, Nightgrid starts a countdown (2 minutes by default) and then shuts down every host in a chosen group, the UPS's own host last. Turning it on means typing the group's name. The countdown shows as a banner above every view with **Cancel shutdown**, sends a notification even in quiet hours, runs with the window closed, and stops by itself if the power comes back. After a cancel it won't start again until the power returns and fails again.
- UPS watches travel in profile exports.

## [0.10.27] - 2026-11-16

### Added: smarter alerts, part 2: runbooks (M10s)
- A note can be a **runbook for alerts**: pick the alert kinds it's for and optionally a host and a service. Matching alerts show **▤ Runbook** in the alerts list (also on alerts folded under a cause); others offer **+ Runbook**, which starts a note for that kind of alert with a step list to fill in.
- **Runbook steps** in notes: `- [ ] step` becomes a checklist to tick while you work, and `[label](view:fleet)` a link that opens a view. Code blocks keep **Send to terminal** (never pressed Enter).
- **✦ Triage** on an alert tells DRIFT about your runbook for it, so the answer starts from your own steps.
- Runbook settings travel with notes in profile exports.

## [0.10.26] - 2026-11-16

### Added: smarter alerts, part 1: dependencies (M10s)
- **Settings → Dependencies**: say what breaks when a host or service is down ("Media server depends on NAS", "NAS depends on router"). Loops are refused. Dependencies travel in profile exports and go away with the host or service.
- **Folding**: alerts explained by an outage upstream are listed under the alert for the cause ("explains 3 more alerts") and don't notify or reach the webhook on their own, including their "resolved" message. Everything about a host already depends on that host, so containers, disks, backups and checks on an offline host fold under it without any setup. In a chain, the furthest-up outage is the cause.
- A notification whose cause may still fire (a service down while its host is inside the offline grace period) waits up to 5 minutes, so an outage gives one notification. The cause's notification says how many alerts it explains.
- The status bar, the tray and the Overview's alert widget count causes, not every folded alert.

## [0.10.25] - 2026-11-16

### Added: DRIFT as a guide (M10q)
- **Guided tours**: step-by-step walkthroughs that put brackets around one control at a time, say what to do and wait for you: a click on the control, opening a view, a dialog opening or closing, or **Next** for steps that only explain. DRIFT never clicks, types or switches views itself. The callout has **Back**, **Skip step** and **End tour**, says when the control isn't on screen, and stays above open dialogs; Esc ends a tour.
- Seven tours: Getting around, Add your first host, Customize the Overview, Watch your services, Check fleet health, Back up your setup, Open a VM's screen. They work without the model.
- **What can I do here?** (**HERE?** in the DRIFT panel): the current view's controls that are on screen, each one click away from being pointed out, and the tours, those for this view first.
- DRIFT can offer a tour at the end of a reply when a question is a multi-step task.
- **First-run offer**: a one-time card offers the Getting around tour; either answer is remembered, and every tour can be replayed from **Settings → Helper**.

## [0.10.24] - 2026-11-16

### Added: DRIFT skills, part 2 (completes M10p)
- **Words → command**: a one-line command in a DRIFT reply has a **TERMINAL** button that types it at the prompt of the host you're asking about (or the focused shell). Nothing runs until you press Enter. Commands the command check marks as dangerous show a DANGER badge and need a second click (**SEND ANYWAY?**).
- **Alert triage**: every alert in the alerts list has **✦ Triage** (log alerts keep **What broke?** with their error lines). DRIFT gets the alert, the usual causes for that kind of alert, the first things to check and where to look in Nightgrid.
- **Daily summary**: ask for "a daily summary" or "what happened overnight" and DRIFT gets what Nightgrid knows about the last 24 hours: unreachable and restarted hosts, active and resolved alerts, services down, pending updates and reboots, disk and backup problems (from the checks that ran this session). Nothing extra is fetched.
- **Runbook notes**: **Save as runbook note** under a reply opens a new note (tagged `runbook`) with the question, what was attached, the checks and the answer, ready to edit and save.
- **Regex helper**: a `/pattern/` or a backticked pattern after "regex" is explained piece by piece and tried on the attached lines (how many match, with the groups), with notes on grep/sed/awk differences.
- **Deeper shell history**: for hosts that share their history, DRIFT now also gets up to 25 older lines that mention the question's words ("when did I last restart nginx?"), not just the newest 60.
- New skill chips: Regex, Draft a command, Daily summary.

## [0.10.23] - 2026-11-16

### Added: DRIFT skills, part 1 (M10p)
- Before DRIFT answers, Nightgrid runs exact checks on the question and the attached text. They appear in a **checks card** above the reply and are given to the model as facts it must not contradict, so the risky parts can't be glossed over even when the answer is wrong.
- **Command check**: put a command in backticks ("is it safe to run \`…\`?") or select one line in the terminal. It flags `rm -rf` (and worse with `/`, `~` or an unquoted `$VAR`), `chmod 777`, `chown -R` on system folders, `curl … | sh`, `dd of=/dev/…`, `mkfs`, partitioning, `> /dev/sdX`, `find -delete`, `crontab -r`, `docker system prune -a`/`--volumes`, `compose down -v`, `git push --force`/`reset --hard`/`clean -f`, firewall flushes and lockouts (`ufw enable`, stopping sshd), shutdowns, `kill -9`, `-y` and `--force`, ZFS/LVM/RAID destroys, Proxmox and libvirt destroys, and the fork bomb, each with why and what to do instead.
- **Compose review**: attach a compose file (Explain in the editor, or select it) and it checks every service for a missing restart policy, untagged or `:latest` images, ports published on every interface (databases and the Docker API called out), secrets written inline, `privileged`, `docker.sock` mounts, host networking and PID namespace, broad capabilities, and the obsolete `version:` key.
- **Error reference**, offline: exit codes (126, 127, 128+signal such as 137 = killed, often out of memory), systemd's `status=203/EXEC`-style codes, systemd results (start limit hit, auto-restart loops, timeouts, OOM kills), and common Docker, SSH, DNS, apt and TLS errors, each with a likely cause and the next command to run.
- **Cron helper**, both ways: a cron expression in a question becomes plain words, its next three run times and the matching systemd `OnCalendar=`; "make a cron schedule for weekdays at 6pm" becomes `0 18 * * 1-5` with notes on cron's quirks.
- Skill chips in an empty DRIFT panel start each kind of question.

## [0.10.22] - 2026-11-16

### Added: customizable Overview (M10o)
- The Overview is now a dashboard of widgets on a 12-column grid. **Edit** turns on edit mode: drag a widget by its title bar to move it, drag its corner to resize, **×** removes it, and **+ Widget** adds one into the first free spot. Widgets that are moved onto push the others down, and everything slides up to close gaps. With the keyboard, focus a widget's grip: arrows move it, Shift+arrows resize it, Delete removes it. **Undo** steps back through the session's changes and **Reset** brings back the built-in layout.
- Ten widgets: NODES (the host tiles), ALERTS (worst first, click to jump), SERVICES (up/down summary and what's failing), UPDATES, DISKS and BACKUPS (fleet health, with a Check button instead of starting SSH work on their own), TUNNELS, pinned NOTES, DESKTOPS (open VM screens and RDP tunnels) and SYS.STATUS.
- **Several dashboards**: tabs above the grid, each with its own layout (for example one for daily checks and one for maintenance). Make one empty or from a preset, rename it and delete it in edit mode.
- **Presets**: four built-in layouts (Overview, Operations, Minimal, Notes & links), and **Save this layout as a preset** for your own. Dashboards and presets are stored with the settings, so they travel in profile exports.
- On a narrow window the widgets stack in one column in reading order.
- Removed the old ROADMAP panel from the Overview.

## [0.10.21] - 2026-11-16

### Added: VM desktops (M10n)
- **Screen** on a running VM in System → VMS opens its graphical console in the new **Desktops** view (`Mod+Shift+M`), one tab per guest. It is a VNC viewer built into Nightgrid: the picture travels over the host's existing SSH connection, so no port is opened, no API token is needed and nothing gets installed. libvirt/KVM guests with a VNC display (the usual `listen='127.0.0.1'`) and Proxmox VE VMs work; a guest with a password on its VNC display is handled when the host shows it to root. A SPICE-only guest says how to add VNC.
- Click the screen to type into the guest; click outside to get the keyboard back. The toolbar has **Ctrl+Alt+Del**, **Type text…** (sends text key by key, for installers and login screens), **Send clipboard** / **Copy guest clipboard** (needs a clipboard agent in the guest), **1:1** / **Fit**, **Full**, **Redraw**, **Disconnect** and **Reconnect**.
- **RDP** on a running VM finds the guest's address (libvirt DHCP leases or the QEMU guest agent), lets you pick or type one, and opens a tunnel from a free port on 127.0.0.1 through the host to the guest's RDP port. Nightgrid then opens a `.rdp` file for it in this computer's RDP client (Windows App / Microsoft Remote Desktop on macOS, `mstsc` on Windows, whatever handles `.rdp` on Linux). Tunnels show in the Desktops view with **Stop**. The file holds only the local address; you sign in inside the RDP client.

## [0.10.20] - 2026-11-15

### Added: backup monitor with test restores (completes M10m)
- The backup board moved from System → BACKUPS to **Fleet → BACKUPS**, next to updates and disks, with a host picker for **Find jobs**. A failed or stale alert now opens it there.
- **Test restore** on a restic or borg job restores one random file under 20 MB from the latest snapshot or archive into a new folder under `/tmp` on the host. It checks the file's size against the backup's listing, shows its SHA-256 and deletes the folder. It runs as root with the backup unit's own `EnvironmentFile=`, `Environment=` and `User=`, so the repository location and password stay on the host. For rsync and other copies to a folder (path jobs), it copies a random file out and compares checksums. You confirm first, and sudo asks for a password when the host needs one.
- Each job shows when its restore was last tested and whether that passed. The result is kept with the job.

## [0.10.19] - 2026-11-15

### Added: disk health and fill-up forecast (M10m, part 2)
- **Fleet → DISKS** shows every host's storage in one list, worst first. It covers **ZFS pools** (state, size, scrub or resilver progress, devices with errors), **software RAID** (`/proc/mdstat`: degraded arrays, failed or spare members, resync and recovery progress) and **btrfs** filesystems (missing devices, error counters, degraded mounts). None of these need root. **SMART** is read too where sudo needs no prompt, and **SMART details** opens System → DISKS.
- Each mount shows its usage, its growth per day over the last two weeks, a 30-day sparkline and **full in about N days**. Usage is recorded on this computer while a host is monitored, one point an hour, kept 90 days. It isn't part of exports.
- New alerts: **FILLING UP** when a mount is forecast to be full within 14 days (3, 7, 30 or 60 in Settings → Alerts; critical inside 3 days), and **POOL DEGRADED** for a ZFS pool, RAID array or btrfs filesystem that lost a device or has errors (checked every 30 minutes on connected hosts). Clicking either opens Fleet → DISKS. **✦ Explain** asks DRIFT what to fix first.

## [0.10.18] - 2026-11-15

### Added: update center (M10m, part 1)
- New **Fleet** view (`Mod+Shift+U`) for health across every host. Its **UPDATES** tab checks all hosts at once (three at a time) and lists pending packages per host, with security packages and **reboot required** flags first. Click a host for its package list and the reason a reboot is needed. Opening the view re-checks when the last check is older than 30 minutes.
- **Upgrade selected…** installs every pending update on the hosts you tick, three at a time, with each host's output streaming under its row. You type the host's name, the group's name when you picked exactly one group, or `upgrade N hosts`. Hosts whose sudo needs a password that Nightgrid doesn't remember are skipped and listed, not prompted one by one. Nothing reboots by itself: **Reboot…** on a flagged host opens its usual typed confirmation.
- **Check images** asks each Docker stack's registry whether an image tag moved, on request only, and lists the newer versions with release notes and **Open stack**. Updating still goes through Docker → STACKS, so a snapshot is taken first.
- **rpm-ostree** systems (Fedora Atomic, Bazzite and similar) now show a pending system image with its version, and **Upgrade** stages it for the next reboot (`rpm-ostree upgrade`). A staged image counts as reboot required. This works both in Fleet and in System → UPDATES.
- **✦ Explain** asks DRIFT which updates matter most and which hosts to do first.

## [0.10.17] - 2026-11-15

### Added: expiry watch and config drift (completes M10l)
- **Services → Expiry watch** lists everything that runs out on a date, soonest first: HTTPS service certificates, proxy route certificates (once Docker → Routes has been read) and items you add.
- **Add…** watches a **certificate file on a host** (end date read with `openssl` over SSH, re-read twice a day while the host is connected), a **domain** (type the date, or **Look up** with `whois` on a host you pick, only when you click), an **API token or key**, a **licence** or anything else with a date. Only names and dates are stored, never the token itself.
- Each item has its own warning days and raises an **EXPIRING** alert inside that window (critical in the last 3 days). Items are included in profile exports. **✦ Explain** asks DRIFT what to renew first.
- **System → CONFIG** compares a host's `/etc`, the compose files in its stack folders and its installed packages (apt, dnf/rpm, pacman, apk) with a **baseline** saved on the host, or with **another host**. Changed, added and removed files are grouped by folder; click a changed file for a line diff (values that look like secrets are masked until you reveal them). Packages show as upgraded, downgraded, added or removed. Host-to-host comparisons hide files that are meant to differ (hostname, machine-id, SSH host keys, network config) unless you turn that off. **✦ Explain** asks DRIFT which changes look risky.
- The baseline lives on the host in `~/.nightgrid/baselines/default` (readable only by that login), so file contents never come to this computer's database; `.env` files are compared by checksum only. Only files the login can read are included, and nothing else on the host is changed.

## [0.10.16] - 2026-11-15

### Added: incident timeline (M10l, part 1)
- **System → TIMELINE** shows what happened on a host, newest first and grouped by day, for the last 24 hours, 7, 30 or 90 days. Nightgrid now records alerts firing and clearing, power actions and wake-ups, compose runs, deploys and rollbacks, container actions, unit actions, package upgrades and automation runs as they happen (kept 90 days on this computer). Opening the tab also reads the host's own history without installing anything: boots (with how long it was down), failed systemd units, kernel out-of-memory kills, apt, pacman and dnf transactions, and Docker container crashes, restarts, OOMs and health changes.
- Filter chips (alerts, boots and power, containers and stacks, units and OOM, packages, automation, problems only) and a text filter. **✦ Summarize** asks DRIFT to explain the shown period: the incidents, likely causes, how they connect and what still needs attention.
- **Clear recorded history…** forgets what Nightgrid recorded for a host; the host's own logs aren't touched.

## [0.10.15] - 2026-11-15

### Added: group and scheduled power (completes M10k)
- **System → GROUP…** suspends, shuts down or reboots every host in a group, one after another, after you type the group's name. Hosts whose sudo needs a password Nightgrid doesn't remember are skipped and listed, not prompted one by one.
- **System → Schedules → POWER SCHEDULE**: shut down, suspend or reboot a host at a set time on chosen days (or every day), or wake it up from its **Wake from** host. Each schedule is a systemd timer on the host itself (`nightgrid-power-…`, `nightgrid-wake-…` on the relay), so it runs whether or not Nightgrid is open, and a scheduled shutdown shows as planned, not offline. Schedules are listed with their next run and can be replaced or removed.

## [0.10.14] - 2026-11-15

### Added: SSH key manager (M10k, part 2)
- **Settings → SSH keys** lists your key pairs (`~/.ssh`) and SSH agent keys with fingerprints, passphrase and agent badges, and which saved hosts use each one. **Copy public key** copies the `.pub` line.
- **NEW KEY…** makes an ed25519 key pair in `~/.ssh`, optionally passphrase protected. Existing files are never overwritten, and the passphrase isn't stored.
- **CHECK HOSTS** reads every host's `authorized_keys` and shows which of your keys each host accepts. You can add a key to a host in one click (skipped if it's already there) or remove one (you confirm by typing the host's name). Keys on a host that aren't yours are listed too.
- **ROTATE A KEY** across hosts, one host at a time: add the new key, log in with only the new key, switch the host's saved settings to it, then remove the old key. A host where a step fails keeps its old key. Removing the old key needs its name typed to confirm.

## [0.10.13] - 2026-11-15

### Fixed
- **Built-in Tailscale: public names that point at tailnet addresses** (for example a DNS record for `app.example.com` that points at a 100.x tailnet node) timed out in service checks, SSH and tunnels when the Tailscale app wasn't running. While the built-in client runs, Nightgrid now looks such names up first and connects through the tailnet when every address is a tailnet address. TLS and the Host header still use the name. With the built-in client off nothing changes.

## [0.10.12] - 2026-11-15

### Added: power states and Wake-on-LAN (M10k, part 1)
- **Suspended isn't offline**: a host that stops answering after Nightgrid suspended, shut down or rebooted it, or after the host itself announced sleep or shutdown (systemd-logind, followed over the monitoring connection), shows **SUSPENDED**, **SHUT DOWN** or **REBOOTING** with the reason and when it was last seen, in a calm colour, and raises no offline alert. It clears as soon as the host answers again; a reboot that takes longer than 15 minutes turns into a real outage. The sidebar, tray, phone status page and DRIFT say the same.
- **Sleep window** per host (host editor, e.g. `23:00-07:00`): inside it, a host that doesn't answer shows **ASLEEP**, not offline.
- **System → SUSPEND / SHUT DOWN / REBOOT**, each confirmed by typing the host name, with sudo as for other system actions.
- **Wake-on-LAN**: set a host's MAC address and **WAKE** (System toolbar, or ⏻ on its overview tile) sends the magic packet from this computer, or from another saved host on the same LAN (**Wake from**), which also works away from home.

### Fixed: network map
- Containers on Docker and Podman bridges (`docker0`, `br-…`) are no longer listed as network devices; the scan notes how many were left out.
- Devices show their maker from the MAC address (an offline copy of the IEEE list is bundled); private random MACs say so.
- A device's IPv6 address joins its IPv4 entry (same MAC) instead of appearing as a second device.
- The router is found from an IPv6 default route too (matched to its IPv4 address by MAC), and from routing tables other than the main one.
- Network shares the scanned host mounts (NFS, SMB) and LAN peers it has open connections to are listed even when they ignore ping, with the share and mount point. **Ping sweep** also tries a few TCP ports on each address for devices that drop ping.

## [0.10.11] - 2026-11-15

### Added: built-in Tailscale and DRIFT knows your hosts (M10j)
- **Settings → Tailnet → Built-in Tailscale** (off by default): Nightgrid joins your tailnet as its own device, without the Tailscale app and without admin rights. **LOG IN** opens the login page once; the node's keys are stored encrypted with a key kept in the OS keychain. SSH, files, Docker, metrics, service checks and tunnels all reach tailnet addresses, MagicDNS names and accepted subnet routes through it. **Use subnet routes** reaches LAN addresses another device advertises; Nightgrid tries the LAN first and the tailnet when the LAN doesn't answer. **DEVICES** lists your tailnet with copyable addresses; **Advanced** sets the device name and a self-hosted control server (Headscale). Nothing is sent to Tailscale's log servers. With it off, tailnet addresses go through the system Tailscale app as before.
- **Tailnet address** per host (host editor): a second address (100.x or a MagicDNS name) tried when the main one doesn't answer. Nightgrid remembers which one worked and tries it first next time, so a host works on the home LAN and away from it. The host key is always checked against the main address.
- **DRIFT knows your hosts**: when a question names one of your saved hosts (loosely: one word of its name, a typo, a follow-up like "check its history"), DRIFT gets that host's live facts first: reachability, monitoring state and last error, last seen and uptime, and active and recent alerts. It answers from those instead of the guide.
- **DRIFT may read shell history** per host (host editor, off by default): DRIFT also gets the host's last 60 commands from bash, zsh or fish history, read over SSH when you ask. Lines that look like passwords or tokens are left out, and nothing is stored.

## [0.10.10] - 2026-11-15

### Added
- **Terminal key hints**: a strip under the terminal shows the keys of the program in front (nano, vim, less/man, htop, top, btop, tmux, screen, watch, mc, nested ssh, and `journalctl -f` / `tail -f` / `docker logs -f`). SSH shells report the command through the session-only hook (bash, zsh, fish); local terminals ask the terminal which program is in front. Hide hints per program, or turn the strip off in **Settings → Terminal**. DRIFT knows the same keys and sees what's running.
- **Network map labels**: name, type, tags and a note for devices that aren't hosts (TVs, phones, printers …), kept by MAC address. Labels show on the map and in a **Labelled devices** list, travel with profile exports, and DRIFT can read them.
- **Network → Scan from → This computer** (macOS and Linux): discovery, including the ping sweep, runs on the machine Nightgrid runs on, which appears on the map as its own node.

### Changed
- **Clicking a host in the sidebar** (or the tray, or **SSH:** in the palette) jumps to its open terminal; it only opens a new one when the host has none. Alt-click (or Cmd-click) always opens a new one.
- **+ NEW** in the terminal tab bar (was **+ LOCAL**) opens a picker: this computer or any saved host (type to filter by name, address or #tag), as a new tab, split right or split down.
- **Terminal background**: the app's backdrop now shows through behind terminal text. **Settings → Terminal → Solid background** brings back the solid panel.
- **Find in terminal** has previous, next and **×** buttons; Esc closes it and returns to the terminal, and the toolbar button and `Mod+Shift+F` toggle it.
- The rolling CRT band of the scanline overlay is much fainter and softer; the lines, flicker and vignette are unchanged.

### Fixed
- **Terminal panes going black after closing a split**: closing a pane crashed the hand-off of the terminal to the pane that stays, so that pane was left empty until the tab was reloaded. Panes now keep what they were opened with while they close.
- **Terminal text drifting out of its columns** (garbled banners, misaligned `htop`): terminals no longer use the GPU renderer, which WebKit could blank when a pane moved; the regular renderer is used everywhere. Cells are measured again when a font finishes loading or a pane comes back on screen, fits wait for the layout to settle, and bold and accented text use bundled faces instead of synthesized ones.
- Network discovery no longer lists loopback interfaces.
- Network discovery works on macOS and BSD hosts: the ping sweep used a Linux-only timeout flag, and tools were wrapped in `timeout`, which macOS doesn't have.

## [0.10.9] - 2026-11-15

### Added: network map, a custom colour scheme and DRIFT reads your notes (M10i)
- **Network** view (`Mod+0`): a map of the internet, your router and tailnet, your hosts and the other devices on the network. **Scan** runs from a host you pick and reads its neighbour table, mDNS announcements, Tailscale peers, router port forwards (UPnP), Tailscale Funnel and cloudflared tunnels; **Ping sweep** also pings each address in its subnet. Nothing is scanned until you click, and nothing outside your network is contacted.
- **Internet-facing** paths are drawn in red and counted on each host, separately from LAN and tailnet links. Selecting a host lists how the internet reaches it and its listening ports (from **Audit ports**), with exposed ports marked. Selecting an unknown device shows its address, MAC, names, services and how it was seen; **Add as host** opens the host editor with them filled in.
- **Settings → Theme → CUSTOM** replaces PLAIN DARK: pick surfaces, text, accent, info, active, danger, OK, warning and the neutral highlight from curated swatches that all stay readable together. **Start from** any built-in scheme, or **Reset**. Saved with your settings and included in profiles.
- **DRIFT reads your notes**: questions search your notes alongside the guide, and replies show which notes they used (click to open). Tick **Hide from DRIFT** on a note to keep it out.
- **Terminal icons**: Nerd Font symbols (powerline prompts, fastfetch, MOTD icons) are bundled and show without installing a font.

### Fixed
- **Terminal splits**: a pane split or resized while its session was still connecting kept the old size on the server, so output (a login banner, `htop`, `vim`) was laid out for the wrong width. The final size is now sent as soon as the session opens, and fits happen once per frame after the layout settles.
- **Terminal panes going blank** after splits or with many panes open: when macOS drops a pane's GPU context, the pane now switches to the regular renderer and repaints instead of staying empty, and moved or re-shown panes repaint themselves.

## [0.10.8] - 2026-11-15

### Fixed
- **Notes** fills the window: the list and the note scroll on their own, so **Save** stays in view however many notes you have. **Mod+S** saves a note while editing.
- **Files**: dragging files from one pane to the other works on macOS (and Windows) again. The window's native file-drop handler was swallowing in-app drags.

## [0.10.7] - 2026-11-15

### Added: VMs, a phone status page and dashboard snapshots (M10h)
- **System → VMs**: libvirt/KVM domains (`virsh`) and Proxmox VE VMs and LXC containers (`qm`, `pct`) on the host, with state, vCPUs, memory and autostart. **Start**, **Shut down** (clean, via ACPI), **Reboot** and **Force stop**. Every action except start asks first, naming the guest and the host, and force stop needs the guest's name typed. **Console** opens the guest's serial console (`virsh console`, `qm terminal`) and **Shell** opens `pct enter` for containers, in a terminal tab. When this login may not use the tools, **Read as root** goes through the usual sudo prompt.
- **Settings → Status page · Snapshot → Serve the phone status page** (off by default): a read-only page with every host's state, CPU, memory, fullest disk, temperature and uptime, plus active alerts and service checks. It refreshes every 30 seconds and is served by the app on this machine's LAN address, its Tailscale address or every interface, behind a secret link (masked until **Reveal link**, **New link** invalidates the old one). The page has no buttons, scripts or commands.
- **Save snapshot…** writes the same page as one static HTML file. Host addresses are left out unless you tick them. Both are also in the command palette.

### Fixed
- **CRT scanlines** now show on macOS (the overlay used a blend mode WebKit drops) and feel alive everywhere: lines that slowly crawl, a faint rolling refresh band, a rare subtle flicker and a vignette. Motion is GPU-only; reduced motion keeps the static lines, and Reduce effects turns it off.

## [0.10.6] - 2026-11-15

### Added: backup board, proxy routes and log anomaly alerts (M10g)
- **System → Backups**: a status board for restic, borg, rsync and rclone jobs on every host, with last run, last good run, size (and what the last run added), result and a **STALE** flag after an age you pick per job. A job points at a **systemd timer or service** (last run, exit status and the totals the tool printed in the journal) or a **path** such as a repository or rsync target (newest file and size on disk). **Find jobs** looks through the host's timers and cron tables and pre-fills them. Nightgrid never opens a repository, needs its password or runs a backup.
- **Docker → Routes**: every domain Traefik (router labels), Caddy (Caddyfile, in a container or on the host, and caddy-docker-proxy labels) or Nginx Proxy Manager serves on the host, where it forwards, the container behind it, and when its certificate expires (yellow inside your alert window, red when expired or missing).
- **Alerts**: *Backup failed or stale* (on by default, checked every 30 minutes) and *Log anomalies* (off by default): every 10 minutes, new error lines from running containers and the journal are grouped into patterns, and an error never seen before or a burst of several times the usual count raises an alert. Log alerts have **✦ What broke?**, which hands the recent error lines to DRIFT on this computer.

### Added: profiles and a weekly backup reminder
- **Settings → Backup · Profile**: the encrypted export is now called a profile: one file, a passphrase you choose, everything from hosts to settings (and saved secrets if ticked), importable on macOS, Windows and Linux. **Export profile…** and **Import profile…** are in the command palette.
- **Backup reminder**: a small card reminds you to export when the last profile export from this computer is older than a week (or 2 weeks, a month, never). **Tomorrow** snoozes it; any export resets it.

## [0.10.5] - 2026-11-15

### Added: stack snapshots, image changelogs and a deploy wizard (M10f)
- **Updates** on each Compose stack asks every image's registry (anonymously, only when you click) whether its tag moved, and shows the version you run, the new version and when it was built. When the image says where its source lives, **What changed** opens the GitHub compare page between the two versions (or the release notes), and **Project** opens the repository. Images pinned by digest, built locally or behind a login are labelled instead of guessed.
- **Snapshots** record each service's exact image and copy the stack's compose files and `.env` on the host (`~/.local/state/nightgrid/snapshots`, mode 700). One is saved automatically before every **Update** and **Rollback**; **Snapshot now** saves one by hand; the newest 10 per stack are kept.
- **Roll back…** restores the saved files and images (pulled again by digest if they were removed) and recreates the services that changed. The confirmation names the stack, the host, every file it overwrites and every service's target image; the state before the rollback is saved first, so it can be undone.
- **+ New stack** deploys from an image or from pasted compose YAML. From an image it reads the image's declared ports, volumes and environment variables to pre-fill a form, then shows the generated `compose.yaml` to edit before writing it. Environment values go into the stack's `.env` (mode 600), never into `compose.yaml`, and are masked until you click **Show values**. Existing files are never replaced without a second click under a warning that names them. The file is checked with `docker compose config` and the stack can be started from the last step.

## [0.10.4] - 2026-11-15

### Added: notes, runbooks and history search (M10e)
- **Notes** (`Mod+9`): markdown notes and runbooks, optionally tied to a host, with tags, pins, search and host/tag filters. Each code block in a note has **Send to terminal** (types it at the prompt of a shell on that host, opening one if needed; you press Enter) and **Copy**. **✦ Explain** asks DRIFT to walk through a runbook.
- Notes never hold secrets: text that looks like a password, token, private key or a password in a URL is refused on save, and **Mask them** replaces those values with `••••`. Notes are included in encrypted backups.
- **Shell history search** (off by default, **Settings → Terminal**): a **HIST** button on SSH terminals searches the host's bash, zsh and fish history. Picking a result types it at the prompt; **+ Note** starts a runbook from it. Lines that look like they hold a secret are left out (and counted), and nothing is saved.

## [0.10.3] - 2026-11-15

### Added: security hygiene and port map (M10d)
- **System → Security** grades a host from A to F with checks that need no root: root and password logins over SSH, an active firewall (ufw, firewalld or nftables), databases, caches or the Docker API listening on every interface, Docker-published ports that bypass ufw, brute-force protection, automatic security updates, a pending reboot and extra uid-0 accounts. Findings come with the fix and copy buttons for its commands; nothing is changed on the host. **✦ Explain** asks DRIFT what to fix first.
- **Listening ports** per host: port, service, who can reach it (local, VPN, one address, all interfaces), and the process or Docker container behind it.
- **Port map · all hosts** scans every host (three at a time, no prompts) and shows the ports each one exposes beyond localhost, with its grade.

## [0.10.2] - 2026-11-15

### Added: forecasts, GPUs and scheduled jobs (M10c)
- **System → Schedules** lists systemd timers and cron jobs on the host: the schedule in plain words ("every 15 minutes", "weekdays at 09:00"), when each runs next and ran last, and whether the last run failed (marked red with its exit code). **Journal** opens the last 7 days of the service a timer starts. Cron jobs come from your crontab, `/etc/crontab` and `/etc/cron.d`; the `cron.daily`/`weekly`/… scripts are listed too. Commands that look like they carry a token are masked until you click **Reveal**.
- **Disk and memory forecasts** in the host monitor: a trend over the last 14 days says when the fullest filesystem fills up, and warns when memory keeps climbing.
- **GPU monitoring**: NVIDIA (`nvidia-smi`) and AMD (amdgpu sysfs) cards show busy %, VRAM, temperature and power in the monitor, and the busiest GPU is kept in history with its own chart.

## [0.10.1] - 2026-11-15

### Added: DRIFT explains things where you are (M10b)
- **Explain** buttons hand what you're looking at to DRIFT with a label saying what it is: container logs (Docker → a container → Logs), the journal (System → Journal), compose output after a stack operation, and each host's result in Automation. Select some lines first to send only those; otherwise the newest 150–200 lines go.
- **Ask DRIFT** in the file editor explains the open file (or the selected part): what it does and anything that looks wrong or risky. Long files are cut to the first part.
- The question is asked straight away when the model is ready; otherwise DRIFT opens with the text attached and the question waiting in the box. Each question shows what was attached under it, and the chip can still be removed before asking. Secret-looking values are masked as before.
- The buttons only appear while DRIFT is turned on.

## [0.10.0] - 2026-11-15

### Added: DRIFT, the local helper (M10)
- **DRIFT** (`Cmd+J`, the DRIFT button in the top bar, or *Ask DRIFT* in the palette) answers questions about Nightgrid and your servers in a side panel. Select log lines, an error or a compose file first and ask what's wrong: the selection goes along with the current view and host. You can remove either before asking.
- **Runs on this computer.** Turn it on under **Settings → Helper** and download the model once (Qwen3 4B Instruct, 2.5 GB, Apache-2.0; resumable and checked against its SHA-256). It uses the GPU (Metal) on macOS and the CPU on Windows and Linux. Nothing you ask leaves the machine.
- **Show me**: when DRIFT explains where something is, *Show me* switches to the view and outlines the control with magenta brackets and a note. It only points; it never clicks.
- Answers come from the bundled user guide. Each reply lists the guide sections it used and can show exactly what the model was given. Values that look like passwords or tokens are masked first.
- Code blocks have a copy button; DRIFT never runs commands itself.

## [0.9.2] - 2026-11-15

### Changed
- **Self-updates are on.** Releases are now signed, and Nightgrid checks GitHub Releases after start-up and offers **Install & restart** when a newer version is out (Settings → Updates). 0.9.2 is the first version that can update itself, so install it by hand once.

## [0.9.1] - 2026-11-15

### Added: handhelds and docs (M9)
- **Settings → Display → Handheld layout** for the ROG Ally, Steam Deck and touch screens: finger-sized buttons and fields, 130 % scale and an icon-only sidebar in one switch.
- **Controller navigation**: with a gamepad connected, the D-pad or left stick moves focus (hold to repeat), A selects, B closes dialogs, LB/RB switch views, Start opens the command palette and Select toggles the sidebar. The focused item gets a clear outline while you use the controller.
- A **user guide** (`docs/USER_GUIDE.md`) covering every view.

## [0.9.0] - 2026-11-15

### Added: encrypted backups (M9)
- **Settings → Backup** exports your setup (hosts, groups, tags, snippets, playbooks, services, tunnels, integrations, bookmarks, Docker stack folders and settings) to one file encrypted with a passphrase. It's a standard [age](https://age-encryption.org) file, so `age -d` can open it too.
- **Include saved passwords and tokens** (off by default) adds the keychain entries to the encrypted file, so the other machine doesn't ask for them again.
- **Import** asks for the passphrase, shows what the file holds and lets you **Merge** (keep everything here, add what's new, skip what already exists) or **Replace** (delete the current setup and load the backup; you type `replace` to confirm). Nightgrid restarts afterwards.
- **Sync folder**: pick a folder that syncs between your machines and export/import use `nightgrid-backup.age` in it with one click. The passphrase is never stored.

## [0.8.0] - 2026-11-15

### Added: automation (M8)
- New **Automation** view (`Cmd+8`): type a command or pick a saved one, choose hosts (click a group to select all of its hosts) and **Run**. Hosts run in parallel, each with its own status, exit code, duration and live output (errors in red). **Stop** ends a run; **Select failed** re-targets the hosts that failed.
- **Snippets**: saved commands with variables. `{{container}}` asks for a value when it runs, `{{lines=100}}` pre-fills one, and `{{host}}` / `{{hostname}}` / `{{user}}` are filled per host. A preview shows the exact command for the first host. Values are safely quoted. Snippets can be shown for all hosts, one host or a group, and can run as root.
- **Playbooks**: steps in order (run a command, upload a file from this computer, wait until a port answers, start/stop/restart a container or pull an image), with live progress per step and stop-on-error.
- **Root**: snippets and steps marked "run as root" use sudo like the System view. Root and passwordless hosts just run; others use their remembered sudo password or the one you type for the run (sent on stdin, never stored).
- **Safety**: commands that delete, kill, stop, restart, reboot, upgrade or overwrite (`rm`, `docker prune`, `compose down`, `systemctl restart`, `> file`, ...) ask first, listing what was found and the hosts. On several hosts you type the group name or the number of hosts.
- **Palette**: `Snippet: …` (aimed at the host of the terminal in front), **Type into terminal** for plain snippets, `Playbook: …`, **Run a command on several hosts**, **New snippet** and **New playbook**.

## [0.7.1] - 2026-11-15

### Added: integrations (M7)
- **Services → Add integration** connects Nightgrid to apps you run. Each gets a card in a new **INTEGRATIONS** panel at the top of Services, refreshed every 30 seconds while the view is open:
  - **Plex**: server version and what's playing (user, title, player, progress, quality) with a transcode count. Needs a Plex token.
  - **AMP** (CubeCoders): game server instances with state, players, CPU and memory, plus **Start**, **Stop** and **Restart**. Stop and restart confirm with the instance named.
  - **Uptime Kuma**: monitors from a public status page (up/down, ping, 24 h uptime). No API key: enter the status page's slug.
  - **Portainer**: link it to a host and containers on that host get an **Open in Portainer** button in Docker.
- Tokens and passwords are kept in the OS keychain only, masked in the editor with **Show** to reveal. **Test** tries the settings before you save.

## [0.7.0] - 2026-11-15

### Added: alerts and notifications (M7)
- Nightgrid now raises **alerts** from what it already watches: a host unreachable for 2 minutes, a disk over 90 % full, a container that was running and exits with an error (or restarts in a loop, or turns unhealthy), a service failing its check for about 90 seconds, a certificate expiring within 14 days, and a disk with a SMART warning.
- **Desktop notifications** for new alerts (a burst becomes one summary). Alerts resolve on their own when the problem goes away.
- The status strip's **ALERTS** counter is live (yellow for warnings, red for critical); click it for the list of active and resolved alerts. Each alert has a button that jumps to the right place: the host monitor, Docker, Services or the host's Disks tab.
- **Settings → Alerts**: turn each rule on or off, set the disk threshold and certificate warning window. **Settings → Notifications**: desktop notifications, **quiet hours** (alerts still show in the app, nothing pings you), and an optional **webhook** to ntfy, Discord or any URL that takes JSON, with "resolved" messages and a **Send test** button. The webhook URL is kept in the OS keychain.
- Docker and SMART are only read on hosts that are already connected. SMART runs every 6 hours, and only where sudo needs no prompt (root, passwordless sudo or a remembered sudo password): a background check never asks for a password.

### Added: tray / menu bar icon (M7)
- A tray icon (menu bar on macOS) with the alert count, **Connect** (every host with its status; click to open a terminal), **Tunnels** (click to start or stop a saved tunnel), **Show** and **Quit**. Turn it off in **Settings → Desktop**.
- **Keep running when the window is closed** (off by default): closing hides the window while alerts, tunnels and checks keep running. Click the Dock icon or **Show** to bring it back.

## [0.6.0] - 2026-11-15

### Added: system management (M6)
- New **System** view (`Cmd+7`) per host with four tabs:
  - **Units**: every systemd service with its state and whether it starts at boot. Filter by active, failed or inactive. Select one for its `systemctl status` and **Start / Stop / Restart / Reload / Enable / Disable**; **Logs** jumps to its journal.
  - **Journal**: `journalctl` with unit, level (e.g. "WARN and worse"), time range and backlog size, following new lines live, with colour by priority and a find box. If you can only see your own entries, **Read with sudo** shows everything.
  - **Updates**: upgradable packages for apt, dnf or pacman with installed and new versions (security updates highlighted), a **REBOOT REQUIRED** flag with the reason, **Refresh lists** (apt), **Upgrade all** with live output, and **Reboot**.
  - **Disks**: SMART health from `smartctl` for every disk (SATA, SAS, NVMe): model, size, temperature, power-on time, NVMe wear and data written, plus a WARNING/FAILING badge with the reasons (reallocated or pending sectors, failed attributes, worn-out flash, overheating). **Details** shows the full attribute table.
- **sudo prompt**: actions that need root ask for your sudo password each time. It's sent to sudo on its standard input, never on a command line. Tick **Remember for this host** to keep it in the OS keychain (off by default); **Forget sudo password** removes it. Hosts where you're root or have passwordless sudo never ask.
- Stop, restart and disable confirm with the unit and host named, upgrades list the packages first, and a reboot needs the host name typed.
- The palette has **<host>: systemd units / journal / package updates / disk health** for every host.

### Changed
- Shortcuts: **Automation** moved to `Cmd+8` to make room for **System** on `Cmd+7`.

## [0.5.0] - 2026-11-15

### Added: services board (M5)
- **Services** view: every web app you run as a card with up/down state, HTTP status, latency with a 24-hour trend, 24 h uptime and, for https, certificate trust and days to expiry (warns under 14 days). Group by your own groups or by host, filter, check one or all now. Click a name to open it in your browser.
- Health checks run in the background from this computer (default every 60 s; **Settings → Services**). Healthy status codes are editable per service (e.g. `200-399,401`), and **Allow untrusted certificate** turns a self-signed or internal-CA cert into a warning instead of "down".
- **Import Homepage**: read a Homepage `services.yaml` from a host (or this computer), preview the groups and skipped entries, and import. **Detect** finds a Homepage container's config folder. Re-running the import updates services from the file and keeps your check settings; removing entries that left the file is opt-in.

### Added: tunnels (M5)
- **Tunnels** view: save port forwards per host: local (`-L`), remote (`-R`) and SOCKS5 (`-D`). Each shows its state, where it listens, live traffic and open connections. Tunnels reconnect on their own after a dropped connection; **Auto-start** brings them up when Nightgrid opens and when the host reconnects.
- **Open web UI through a tunnel**: type a port or URL as the host sees it and press Open, or use **Tunnel** on a service card linked to a host. Nightgrid forwards it to a free local port and opens it in your browser.
- The top bar's TUN counter shows running tunnels (click to open Tunnels). The palette has **Add service**, **Import Homepage**, **Open <service>**, **Add tunnel** and **Start/Stop tunnel <name>**.

## [0.4.2] - 2026-11-15

### Added: app updates (M4.5)
- Nightgrid checks GitHub Releases for a newer version shortly after launch (one request; turn it off in **Settings → Updates**). When there is one, a dialog shows the version and its changelog with **Install & restart**, **Later** and **Skip this version**. **Check now** is in Settings.
- Updates are downloaded, verified against the project's signing key (including the version they were signed for) and installed in place: the .app on macOS, the NSIS installer on Windows (passive mode), the AppImage on Linux. .deb/.rpm installs keep updating through the package files.
- Self-update switches on once the release signing key is configured; until then Settings says so and new versions come from the Releases page as before.
- Releases are now regular GitHub releases (not prereleases), since the updater follows "latest".

### Changed
- The glitch/scramble text decode-in is used more widely: panel and dialog titles, view switches, notices and errors, and host state changes on the Overview tiles. It stays quick and is off with **Reduce effects** or the system's reduced-motion setting.

## [0.4.1] - 2026-11-14 (prerelease)

### Changed
- New app icon (and matching sidebar mark): a terminal window in the app's colours (mint frame, a magenta/mint title-bar stepper, cyan prompt, magenta cursor) on a near-black tile sized to the macOS icon grid. The vector source is `app/icons/source.svg`.

### Added: resizable layout
- Drag the borders to resize the sidebar, the split between the two file panes, the transfer queue, the Docker container panel and the compose output console. Sizes are remembered on this machine. Double-click a border (or press Enter on it) to reset it, arrow keys nudge it, and the palette has **Reset layout**.

### Fixed
- Command palette: the prompt and the search text sit on one left-aligned line again. A global form-field style was stacking them and centering the text.

## [0.4.0] - 2026-11-14 (prerelease: M0–M4)

### Added: monitoring (M4)
- **Overview tiles are live**: each host shows CPU % with a sparkline, memory and fullest-disk meters, network rates, CPU temperature and uptime. The state line says when a host is offline (with the retry countdown) or needs a login first.
- **Host monitor** (click a tile, or `Monitor: <host>` in the palette): CPU with per-core bars, load, memory and swap, every real mount with usage, per-disk read/write, per-interface network rates, temperatures from hwmon/thermal sensors, and a **history chart** (CPU, memory, network, disk I/O, temperature) over 1 h, 24 h, 7 days or 30 days.
- Agentless: one short read-only `sh` command per sample over the host's existing SSH connection (`/proc`, `df`, `/sys` sensors). Nothing is installed and sudo is never used. Works on Debian/Ubuntu/Fedora/Arch servers, Raspberry Pi OS and BusyBox systems; VMs and containers just show no temperatures.
- Hosts on screen are sampled every 2 s, the rest every minute (and when the window is hidden). Hosts that need a password or a host-key decision never pop up a prompt in the background; they wait until you connect once from a terminal or the file browser.
- History is stored as minute averages (kept 7 days by default) plus 15-minute averages (kept 90 days).
- **Settings → Monitoring**: on/off, sample interval, background interval and how long minute detail is kept.

### Changed
- ASCII host icons size to their content, so they also fit inside the new tiles.

## [0.3.0] - 2026-11-14 (prerelease: M0–M3)

### Added: release builds
- Pushing a `v*` tag, or running CI manually with a `release_tag`, publishes the macOS (.dmg), Windows (.exe, portable .zip) and Linux (.deb, .rpm, .AppImage) installers as a GitHub prerelease. Builds are unsigned.

### Added: ASCII host icons
- Each saved host can have a small ASCII icon (up to 8 lines × 32 characters) shown above its name on the overview tiles and the terminal's connect tiles. It's drawn in mono in the host's color (cyan if none), with the app's glow, which turns off with "reduce effects".
- Pick one in the host editor from presets (server, NAS, Pi/SBC, router, desktop, tower, cloud/VPS, terminal) or draw your own, with a live preview. ASCII, box-drawing, block and braille characters are allowed; emoji and control characters are refused. Indentation and blank edge lines are cleaned up on save.

### Added: compose stacks (M3 P1)
- **Docker → STACKS** tab lists compose stacks on the host: every project with containers (from the compose labels), plus stacks found in **stack folders** you add per host (e.g. `/opt/stacks`, `~/stacks`; Nightgrid looks for `compose.yaml`/`docker-compose.yml` in the folder and one level below). Each row shows the folder and how many services are up.
- Per stack: **Up** (`up -d`), **Pull**, **Update** (pull, then `up -d`, with a summary of which services got a new image, old → new id), **Restart** and **Down**. Down/Restart/Update confirm with the stack and host named. Output streams live into a console under the list, and **Cancel** sends Ctrl-C.
- Edit a stack's compose file or `.env` straight from its row; they open in the editor over SFTP, and a missing `.env` is created on save.
- Uses `docker compose`, falling back to `docker-compose`, and works with rootless Docker.
- Palette: `Stacks: <host>`.

### Added: edit server files in Nightgrid instead of nano
- New setting, **Settings → Terminal → Edit server files in Nightgrid** (off by default). In new SSH terminals, `nano FILE` and anything that runs `$EDITOR`/`$VISUAL` (git commit, `crontab -e`, `sudoedit`) open the file in an editor tab with normal desktop shortcuts: `Ctrl/Cmd+S` save, `Z`/`Shift+Z`/`Y` undo/redo, `C/X/V`, `F` find, `A` select all, Shift+arrows and mouse selection. The terminal waits. **Save & return** (or closing the tab) hands control back, and Enter in the terminal stops waiting. A missing file opens empty and is created on save.
- Nothing is installed on the server. The terminal starts your normal shell (bash, zsh or fish, as a login shell) with a session-only helper in a private temp folder, which is deleted when the shell exits. `nano` with no file still runs the real nano.

### Added — M3 Docker
- Docker view per host, agentless: the Docker API runs over the host's existing SSH connection by forwarding `/var/run/docker.sock` (direct-streamlocal). If sshd forbids socket forwarding it falls back to `docker system dial-stdio`. Rootless sockets (`$XDG_RUNTIME_DIR/docker.sock`) are found too.
- Containers grouped by compose project (from the `com.docker.compose.*` labels), with the project folder, then the containers outside compose. Each row has name/service, image, state, health, uptime, published ports, CPU and memory (like `docker stats`). The list refreshes every 5 s while the view is open. You can filter it and hide stopped containers.
- Actions: start, stop, restart, kill, pause/unpause and remove, per row, per compose project, or for a multi-selection. Stop/restart/kill/remove confirm with the exact containers and host. Removing a running container needs "force", and removing several needs the host name typed.
- Live logs in a side panel: follow, search with highlight (and "only matches"), timestamps on/off, tail 100/500/2000/10000/all, stdout/stderr colouring, ANSI colours stripped, copy, and "jump to end". Up to 20,000 lines are kept, and the view is virtualised so long logs stay smooth.
- Shell into a container: opens a terminal tab running `docker exec -it` (bash if present, else sh). It reconnects like any SSH tab.
- Container detail: state, start/finish times, exit code, restart policy and count, command, ports, mounts, networks, labels, and the raw inspect JSON. Environment values are hidden until you click them (or "reveal all"), and are masked in the raw JSON.
- A clear message with the fix when the SSH user can't use the Docker socket (`sudo usermod -aG docker <user>`), or when Docker isn't installed.
- Palette: `Docker: <host>`, plus `Logs: <container>` and `Shell: <container>` for the host on screen.

### Fixed
- Debug builds no longer log Docker API payloads (bollard's debug output would have included container env values).

### Known gaps (M3)
- Images, volumes and networks lists come later.

### Added — M2 SFTP + editor
- Files view with two panes; each pane is this machine or any saved host, so you can copy local ↔ remote and host ↔ host (through the app). SFTP runs on the host's shared SSH connection, so the terminal, files and the coming features share one login.
- Browser: path bar (with `~`), back/up/refresh, filter, hidden-files toggle, multi-select (click, Ctrl/Cmd-click, Shift-click, arrows, Space, Ctrl/Cmd+A), drag & drop between panes or onto a folder, new folder (F7), rename (F2), copy to the other pane (F5) and delete (Del). Delete always lists the exact targets, and deleting several items on a host also needs the host name typed.
- Transfer queue: files and whole folders, progress, speed and ETA, up to 3 jobs at once, pause/resume, cancel (removes the partial file), retry that continues partial files, and conflict handling (overwrite / merge folder, skip, keep both as `name (1).ext`) with a side-by-side size/date compare and "apply to all". Permissions are carried over. Panes refresh when a transfer lands in them. The status strip shows active transfers and conflicts.
- Editor (CodeMirror 6, loaded on demand) in tabs next to the browser: syntax highlighting for YAML/compose, `.env`, JSON, shell, nginx, Dockerfile, TOML, INI/systemd units, Python, JS/TS, Markdown, HTML/CSS/XML; search/replace, folding, bracket matching, `Ctrl/Cmd+S`. Files over 5 MiB, binary or non-UTF-8 files are refused instead of being corrupted. Line endings (LF/CRLF) are kept.
- Atomic saves: write a temp file next to the original, give it the original's mode and owner, then rename over it. Symlinks are saved through (the link stays a link). If the folder isn't writable or the owner can't be kept, the file is rewritten in place and the editor says so. If the file changed on the server since you opened it, the save stops and offers "reload theirs" or a confirmed overwrite.
- Bookmarks per host (and for this machine): ★ saves the current folder, chips jump back to it.
- Palette: `Files: <host>` opens a host in the right pane.

### Known gaps (M2)
- `sudo` edit for root-owned files (P1) and folder sync (P2) are not in yet. Dropping files from Finder/Explorer into a pane isn't supported yet; use the local pane.

### Added — M1 hosts + terminal
- Hosts: name, hostname, port, user, group, tags, colour, favourite, notes, auth method (SSH agent / key file / password), identity file, ProxyJump and "remember secret". Host list in the sidebar with groups, favourites, fuzzy and `#tag` filter, and TCP reachability every 30 s; host editor; host tiles on the overview.
- `~/.ssh/config` import (with `Include`, `Host` patterns, `%h/%r/%p` tokens); re-sync keeps your names, groups, tags and colours. The file is only read, never written.
- SSH client (russh): agent first (OpenSSH agent, 1Password, Secretive, Pageant / Windows OpenSSH agent), key files with passphrase prompt, password and keyboard-interactive, ProxyJump chains, keepalive. One shared connection per host; every pane on that host multiplexes over it.
- Host key verification against NIGHTGRID's own `known_hosts` (hashed entries and wildcards understood; `~/.ssh/known_hosts` is read as a second source and can be imported). First connection shows the fingerprint; a **changed key blocks the connection** with a red screen, a fingerprint diff and "type the host name to accept".
- Password/passphrase prompts with an opt-in "remember in OS keychain". Without it, secrets stay in memory until the app quits.
- Terminal: xterm.js 6 with WebGL renderer, tabs, resizable split panes (right/down), find, clickable links (opened in the system browser), copy on select, right-click paste, `Ctrl+Shift+C/V` on Windows/Linux, truecolor. Local shells (PowerShell/pwsh on Windows, login zsh/`$SHELL` on macOS, `$SHELL` on Linux) via a PTY.
- Auto-reconnect with backoff (1, 2, 4, 8, 16, 30 s) and a "Retry now" button; prompts (host key changed, password) stop the retries until you answer.
- Terminal settings: font size, scrollback, copy on select, right-click paste, cursor blink. Terminal shortcuts (`Mod+Shift+T/W/D/S/F`, `Ctrl+Tab`); on Windows/Linux only Shift chords are captured while a terminal has focus.
- Live counts in the status strip (sessions) and top bar (SSH), and the breadcrumb shows the focused host.
- Tests against a real OpenSSH `sshd` (key auth, exec, ProxyJump, interactive shell exit code, changed host key), run in CI on Linux and macOS.

### Fixed
- The app window stayed blank in Tauri when a dependency assigned `toString` on a plain object (xterm.js) because `freezePrototype` is on. A build step now rewrites those assignments; a test guards future xterm upgrades.

### Added — M0 skeleton
- Cargo workspace with `nightgrid-core` (library) and `nightgrid` (Tauri 2 app) crates.
- SQLite store (`rusqlite`, bundled SQLite) with forward-only migrations; initial schema for settings, host groups, hosts and tags. A test guards against secret-looking columns.
- OS keychain wrapper (`keyring` v4) behind a `SecretStore` trait, with an in-memory store for tests. Secrets are redacted in `Debug`/`Display` and zeroized on drop.
- App settings (theme, reduce effects, scanlines, UI scale 90–150 %, collapsed sidebar) persisted in SQLite and validated in Rust.
- Typed IPC: Rust commands exported to `ui/src/lib/bindings.ts` with tauri-specta; CI fails on drift.
- Rotating `tracing` log files (daily, 7 kept) in the app log dir.
- Linux WebKitGTK DMA-BUF auto-workaround for NVIDIA, with env overrides.
- Svelte 5 frontend: design tokens, theme system with NIGHTGRID (default), ICE, BLOODLINE and PLAIN DARK themes, bundled JetBrains Mono + Rajdhani.
- App shell: left rail (nodes + modules), top bar with segmented breadcrumb and palette trigger, HUD status strip with clock and keychain/DB status.
- Command palette (`Ctrl/Cmd+K`) with fuzzy search, keyboard navigation and theme/view/navigation commands.
- Settings view: theme tiles, segmented UI-scale slider, effect toggles, keybinding list, diagnostics.
- Effects: grid backdrop, optional scanlines, glow, scramble text on titles. All off with "reduce effects", the plain theme, or OS `prefers-reduced-motion`.
- Strict CSP (no inline scripts or styles, no remote content) and a minimal capability set.
- CI: lint + tests (svelte-check, eslint, vitest, rustfmt, clippy `-D warnings`, cargo test) and installer builds on Windows, macOS (arm64) and Linux.
