# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [SemVer](https://semver.org/).

## [Unreleased]

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
