# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [SemVer](https://semver.org/).

## [Unreleased]

## [0.10.10] - 2026-11-15

### Added
- **Terminal key hints**: a strip under the terminal shows the keys of the program in front (nano, vim, less/man, htop, top, btop, tmux, screen, watch, mc, nested ssh, and `journalctl -f` / `tail -f` / `docker logs -f`). SSH shells report the command through the session-only hook (bash, zsh, fish); local terminals ask the terminal which program is in front. Hide hints per program, or turn the strip off in **Settings → Terminal**. DRIFT knows the same keys and sees what's running.
- **Network → Scan from → This computer** (macOS and Linux): discovery, including the ping sweep, runs on the machine Nightgrid runs on, which appears on the map as its own node.

### Fixed
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
