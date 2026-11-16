# Nightgrid user guide

Nightgrid manages your servers over plain SSH. Nothing is installed on them: terminals, files, Docker, monitoring, services, tunnels, system tasks and automation all use the SSH access you already have. Everything is stored on this computer, and passwords live only in the OS keychain.

`Mod` below means **Cmd** on macOS and **Ctrl** on Windows and Linux.

## Getting around

- **Sidebar**: your hosts (with favorites and groups) at the top, the views below. `Mod+B` collapses it to icons.
- **Views**: Overview `Mod+1`, Terminal `Mod+2`, Files `Mod+3`, Docker `Mod+4`, Services `Mod+5`, Tunnels `Mod+6`, System `Mod+7`, Automation `Mod+8`, Notes `Mod+9`, Network `Mod+0`, Fleet `Mod+Shift+U`, Settings `Mod+,`.
- **Command palette** `Mod+K` (also `Ctrl+Shift+K` inside a terminal): jump to any host, view, snippet, playbook or action by typing.
- **DRIFT helper** `Mod+J`: ask a question in the side panel (see below).
- **Settings → Keybindings** lists every shortcut.

## Hosts

- **+** in the sidebar adds a host: name, address, port, user, and how to log in.
  - **SSH agent** (recommended): uses the keys your agent holds, including 1Password or Secretive if they act as your agent.
  - **Key file**: a private key, with its passphrase asked when needed.
  - **Password**: asked when needed.
  - **Remember** saves the password or passphrase in the OS keychain. It's off unless you tick it.
- **IMP** imports hosts from `~/.ssh/config`, including jump hosts (`ProxyJump`). Your SSH config is only read, never written.
- **Host keys**: the first connection shows the server's fingerprint for you to accept. If a key changes later, the connection is blocked until you type the host name to accept the new key. A diff of the fingerprints is shown.
- Groups, tags, colors, icons and favorites keep long lists tidy. Filter the list with text or `#tag`.

## Terminal

- Click a host to go to its terminal (a new one opens if it has none; Alt-click or Cmd-click always opens a new one), or press `Mod+2`. Use `Ctrl+Shift+T` for a local shell.
- **+ NEW** in the tab bar opens this computer or any saved host, as a new tab or split next to the focused pane. Type to filter, arrows and Enter to pick.
- Panes and tabs:
  - Split right with `Ctrl+Shift+D` and split down with `Ctrl+Shift+S`.
  - Close a pane with `Ctrl+Shift+W`.
  - Switch tabs with `Ctrl+Tab`.
  - Find with `Ctrl+Shift+F` (again, Esc or **×** closes it).
- Terminal text sits on the app's backdrop; **Settings → Terminal → Solid background** gives it a solid panel instead.
- Dropped connections reconnect on request. Links in the output open in your browser.
- **History search** (turn on under **Settings → Terminal**): **HIST** on an SSH terminal searches the host's bash, zsh and fish history. Enter or a click types the command at the prompt; **+ Note** starts a runbook with it. Lines that look like they hold a password or token are left out, and nothing is saved.
- **Edit server files in Nightgrid** (Settings → Terminal): `nano FILE` and `$EDITOR` open the file in Nightgrid's editor instead.
- **Key hints**: a strip under the terminal shows the keys of the program in front of the active pane: nano, vim, less and `man`, htop, top, btop, tmux, screen, watch, Midnight Commander, a nested `ssh`, and anything that follows output (`journalctl -f`, `tail -f`, `docker logs -f`). On SSH terminals the shell reports the command through the same session-only hook as editing in Nightgrid (bash, zsh and fish; nothing is installed and it's gone when the shell exits). On local terminals Nightgrid asks the terminal which program is in front. **×** hides the hints for that program; **Show hidden** brings them back. Turn the strip off under **Settings → Terminal → Key hints**. DRIFT knows the same keys ("how do I quit vim?").

## Files

- Two-pane SFTP browser. Bookmarks, upload and download with a transfer queue, rename, new folder and delete.
- A delete always lists exactly what will go.
- Open text files in the built-in editor. If the file changed on the server since you opened it, saving asks first.

## Docker

- Containers per host with live CPU and memory, logs (follow and search), inspect, and a shell inside a container.
- Start, stop, restart and remove are available. Stop, restart and remove confirm the containers by name.
- **Stacks**: Compose projects, plus folders you add, with up, down, pull and restart.
- **Routes**: the domains Traefik, Caddy or Nginx Proxy Manager serve on the host, where each one forwards, the container behind it and how many days its certificate has left.
- **Updates** asks each image's registry whether its tag moved (only when you click). It shows the version you run and the new one, and **What changed** opens the project's compare page or release notes in your browser.
- **Snapshots** save each service's exact image plus copies of the compose files and `.env` on the host. One is taken before every Update and Rollback, and the newest 10 are kept. **Roll back…** names every file and service it changes and saves the current state first.
- **+ New stack** builds a `compose.yaml` from an image (ports, volumes and variables pre-filled from what the image declares) or from pasted YAML. Variable values go into `.env` with mode 600, never into `compose.yaml`.
- Environment values are masked until you click them.

## Overview and monitoring

- The Overview is a dashboard of widgets. Press **Edit** to change it: drag a title bar to move a widget, drag the bottom-right corner to resize it, **×** removes it and **+ Widget** adds one. Others make room by moving down. From the keyboard, Tab to a widget's **⠿** grip and use the arrows (Shift+arrows resize, Delete removes). **Undo** steps back and **Reset** restores the built-in layout. Press **Done** when finished; every change is saved as you go.
- Widgets: NODES, ALERTS, SERVICES, UPDATES, DISKS, BACKUPS, TUNNELS, NOTES (pinned ones), DESKTOPS and SYS.STATUS. UPDATES, DISKS and BACKUPS show the last check and have a **Check** button; they don't contact your hosts by themselves.
- **Dashboards**: in edit mode, **+ New** makes another dashboard (empty or from a preset) shown as a tab; **Rename** and **Delete** act on the one shown. **Presets** applies a built-in layout or one you saved with **Save this layout as a preset**. Dashboards and presets are part of the settings, so profile exports carry them.
- The Overview shows every host with CPU, memory, disk, network and temperatures, read over SSH with one short command per sample.
- Click a host for history charts, from 1 hour to 1 year. Minute detail is kept for the days set in Settings → Monitoring, 15-minute averages for at least 90 days and hourly averages for 400 days. Drag across a chart to zoom into that span (finer detail loads when it is stored); **Reset zoom** or a double-click goes back.
- Hosts that need a password or a new host key wait until you open a terminal on them; monitoring never prompts.
- **Forecasts**: the monitor fits a trend to the last 14 days and says when the fullest disk reaches 100 % (orange within two months, red within two weeks). Memory gets a line only when it keeps climbing towards 95 %, which can point at a leak. It needs at least two days of history.
- **GPUs**: NVIDIA cards (through `nvidia-smi`, if the driver is installed) and AMD cards (amdgpu) show busy %, VRAM, temperature and power in the monitor, with a GPU history chart. Intel GPUs aren't read yet.

## Services and integrations

- **Services**: a board of web apps with an up/down check, response time, certificate expiry and 30 days of history. Import them from a Homepage `services.yaml`, or add them by hand.
- **Add integration**:
  - **Plex**: what's playing.
  - **AMP**: game servers with start, stop and restart.
  - **Uptime Kuma**: monitors from a status page.
  - **Portainer**: "Open in Portainer" links.
- Tokens are kept in the keychain only.
- **Expiry watch** (bottom of Services): everything that runs out on a date, soonest first. HTTPS services show their certificate automatically, and proxy route certificates appear once you've opened Docker → Routes. **Add…** watches more:
  - a **certificate file on a host** (a PEM path): Nightgrid reads its end date with `openssl` over SSH when you add it, on **Read cert files**, and twice a day while the host is connected;
  - a **domain**: type the renewal date, or pick a host and **Look up**, which runs `whois` on that host (only when you click; the host asks the registry);
  - an **API token or key**, a **licence** or anything else, with a typed date. Only the name and date are kept, never the token.
  Each item has its own **warn days**; inside that window it raises an **EXPIRING** alert (critical in the last 3 days), under the same switch as certificate alerts in Settings → Alerts. Items are part of profile exports. **✦ Explain** asks DRIFT what to renew first.

## Tunnels

- **Local** (`-L`): reach a server-side port from this computer. **Remote** (`-R`): expose a local port on the server. **Dynamic** (`-D`): a SOCKS proxy.
- Saved tunnels can start with the app. Listening anywhere other than localhost is flagged as public.

## System

- Per host:
  - **services**: systemd units, with start, stop, restart, enable and disable;
  - **journal**: live logs with filters;
  - **schedules**: systemd timers (schedule, next and last run, and whether the last run failed, with a **Journal** button for the service it starts) and cron jobs from your crontab, `/etc/crontab` and `/etc/cron.d` in plain words ("daily at 03:30") with the next run. Read-only. Commands that look like they hold a token are masked until you click **Reveal**;
  - **timeline**: what happened on the host, newest first, for the last 24 hours, 7, 30 or 90 days. Nightgrid records alerts firing and clearing, power actions, compose runs, deploys and rollbacks, container and unit actions, upgrades and automation runs (marked **NG**, kept 90 days). The host's own history is read when you open the tab (marked **HOST**): boots and how long it was down, failed units, out-of-memory kills, apt/pacman/dnf transactions and Docker container crashes, restarts and health changes. Filter by kind or to **Problems only**; **✦ Summarize** has DRIFT explain the period. Failed units and OOM kills need a login that can read the system journal (the `systemd-journal` or `adm` group); Docker only remembers recent events. **Clear recorded history…** forgets Nightgrid's entries for that host;
  - **config**: config drift. **Save baseline** records the host's `/etc`, the compose files in its stack folders and its package list on the host itself (`~/.nightgrid/baselines/default`, readable only by that login). **Compare** then shows which files changed, appeared or went away and which packages were upgraded, downgraded, added or removed; click a changed file for a line diff (secret-looking values are masked until **Reveal secrets**). Pick another host under **Compare with** to compare two hosts live; files meant to differ (hostname, machine-id, SSH host keys, network config) are hidden unless you turn that off. Only files the login can read are compared, and `.env` files only by checksum;
  - **security**: a hygiene score (A–F) from checks that need no root: SSH root and password logins, an active firewall, databases or the Docker API listening on every interface, Docker ports that bypass ufw, fail2ban/CrowdSec, automatic security updates, a pending reboot and extra uid-0 accounts. Each finding says how to fix it, with copy buttons for the commands; Nightgrid never changes anything itself. Below the checks, every listening port shows who can reach it (local, VPN, one address, all interfaces) and which process or container owns it. **Port map · all hosts** scans every host and lays their reachable ports side by side;
  - **updates**: apt, dnf, pacman or rpm-ostree, with a reboot-needed flag (on rpm-ostree, **Upgrade** stages the new image for the next reboot);
  - **disks**: SMART health;
  - **VMs**: libvirt/KVM and Proxmox VE guests (VMs and LXC containers) with state, vCPUs and memory. **Start**, **Shut down**, **Reboot** and **Force stop** (asks first and names the guest; force stop needs its name typed). **Console** opens a guest's serial console and **Shell** enters a Proxmox container, in a terminal tab. **Read as root** appears when this login can't use `virsh`, `qm` or `pct`. **Screen** opens a running VM's graphical console in the Desktops view, and **RDP** connects your RDP client to it (see Desktops);
- **sudo**: asked each time by default. Tick "remember for this host" to keep it in the keychain. The password is sent on sudo's input, never on a command line.

## Fleet

Health across every host at once.

- **Updates**: **Check all** reads pending packages on every host (apt, dnf, pacman, rpm-ostree), three at a time. Hosts with security packages or a pending reboot come first. Click a host for its package list. Tick hosts and choose **Upgrade selected…**. To confirm, type the host's name, the group's name (when you picked exactly one whole group) or `upgrade N hosts`. Hosts are upgraded three at a time, and the output shows under each row. Hosts whose sudo needs a password that isn't remembered are skipped with a note: upgrade those from System → UPDATES and tick remember. Nothing reboots on its own, and **Reboot…** opens the host's usual confirmation.
- **Disks**: each host's ZFS pools, RAID arrays (mdraid) and btrfs filesystems, with degraded devices, error counters and scrub, resilver or resync progress. None of this needs root. SMART is shown where sudo needs no prompt; read it once from System → DISKS with **remember** ticked to include a host that asks for a password. Every mount shows its growth per day and **full in about N days**, from usage Nightgrid records while the host is monitored (one point an hour, kept 90 days, so the forecast starts after about a day). Alerts: **FILLING UP** inside the window set in Settings → Alerts (14 days by default) and **POOL DEGRADED**.
- **Backups**: a board for every host's restic, borg, rsync and rclone jobs with last run, size, result and a stale flag. A job reads a systemd timer (last run, exit status and the totals the tool printed) or a path such as a repository or rsync target (newest file and size). Pick a host and **Find jobs** to get suggestions from its timers and cron tables. Nightgrid never runs a backup.
- **Test restore** (restic, borg, or a path job for a folder copy) proves a backup can be read back. It restores one random file under 20 MB from the latest snapshot into a new folder under `/tmp` on the host, checks its size (or checksum for a folder copy), shows the SHA-256 and deletes the folder. It runs as root with the backup unit's own environment and user, so the repository password never leaves the host. A job that points at a folder path can't find a restic or borg password on its own, so test the unit job instead. Each job shows when its restore was last tested.
- **UPS**: **+ UPS** watches a UPS through NUT. Pick the host it's plugged into by USB (with NUT installed there, `nut` or `nut-client`) and its NUT name: `ups@localhost`, or `name@nas.lan` for a NAS's built-in NUT server, which any host can read. **Test** reads it once; `upsc -l` on the host lists the names. Every UPS shows its state, charge, runtime, load and input voltage, read every 30 s while its host is connected. Alerts: **ON BATTERY** (warning; critical when the battery is low or the runtime is under your limit) and the UPS's "replace battery" flag.
- **UPS shutdown**: tick **Shut a group down when the power stays out**, pick a host group and when to act: after some time on battery, when runtime drops under a limit, and always when the UPS itself reports low battery. Type the group's name to arm it. When it triggers, a red banner above every view counts down (2 minutes by default) and names the hosts; **Cancel shutdown** stops it, and so does the power coming back. Then each host in the group is told to shut down, the UPS's own host last. It works with the window closed (Nightgrid keeps running in the tray) and the notification ignores quiet hours. A host whose sudo needs a password that isn't remembered can't be shut down unattended and is listed in the final notification; allow passwordless `shutdown` there or tick **remember** for its sudo password. After you cancel, it won't start again until the power returns and fails again.
- **Uptime**: one bar per day for every host and checked service over the last 30 or 90 days (green from 99.9%, yellow from 99%, red below, grey where nothing was recorded), with availability and total downtime. Hover a bar for that day; click a row for its outages, with how long each lasted and the error it started with. Uptime is recorded while Nightgrid runs, also in the tray with the window closed and with alerts off. Availability counts only the time Nightgrid was watching, and a host that is asleep or rebooting on purpose isn't down. Kept 400 days on this computer, not in profile exports. The phone status page shows the same 90-day bars.
- **Monthly report…** (on the Uptime tab): pick a month and a target such as 99.9% to see each host's and service's availability, downtime, outages and longest outage, and whether it met the target. **Export HTML…** saves the report as one page with every outage listed; open it in a browser and print or save as PDF.
- **Check images** asks the registries whether any stack's image has a newer version, only when you click it. **Open stack** takes you to Docker → STACKS, where updating takes a snapshot first.

## Desktops

Graphical consoles of virtual machines, opened with **Screen** on a running VM in System → VMS. Each guest gets a tab.

- **How it connects**: Nightgrid asks the host where the VM's display is and streams it over the host's SSH connection. libvirt/KVM guests need a VNC display (`<graphics type='vnc' listen='127.0.0.1'/>`, which is virt-manager's default for new VMs; SPICE-only guests say so). Proxmox VMs go through `qm vncproxy`, which needs root, so sudo asks when it has to. No port is opened on the host or on this computer, and there is no token to set up.
- **Keyboard and mouse**: click the screen to send keys to the guest (it gets a magenta outline). App shortcuts are off while it has focus; click anywhere else to get them back. **Ctrl+Alt+Del** and **Type text…** cover keys and text the host keyboard can't send, for example in an installer or at a login prompt. Typing assumes the guest uses the same keyboard layout.
- **Clipboard**: **Send clipboard** puts this computer's clipboard text on the guest's, and **Copy guest clipboard** appears when the guest copies something. Both need a clipboard agent in the guest (spice-vdagent or the VNC equivalent); without one, use **Type text…**.
- **View**: **Fit** scales the screen down to the window, **1:1** shows real pixels with scroll bars, **Full** fills the screen, and **Redraw** asks for the whole picture again.
- **RDP** (Windows guests, or anything running an RDP server): **RDP** on the VM looks up its address from libvirt's DHCP leases or the QEMU guest agent; pick one or type it. Nightgrid starts a tunnel from a free port on 127.0.0.1 through the host to the guest and opens it in your RDP client. Sign in there with the guest's account. Running tunnels are listed at the top of Desktops with **Stop**; they also end when Nightgrid quits.

## Automation

- Type a command, pick hosts (click a group to select all of it) and **Run**. Each host shows its own output, exit code and time. **Select failed** retries just the hosts that failed.
- **Snippets** are saved commands with variables:
  - `{{name}}` asks for a value when it runs;
  - `{{name=default}}` pre-fills one;
  - `{{host}}`, `{{hostname}}` and `{{user}}` are filled in for each host.
  - Values are quoted safely, so don't wrap placeholders in quotes yourself.
- **Playbooks** run steps in order: commands, file uploads, wait for a port, and Docker start, stop, restart or pull.
- Anything destructive (`rm`, `prune`, `kill`, `compose down`, `reboot`, `upgrade`, `> file`, ...) is confirmed with the targets named. On several hosts you also type the group name or the number of hosts.

## Notes

- `Mod+9` opens **Notes**: markdown notes and runbooks, optionally tied to a host, with tags and pins. Search and the host and tag filters narrow the list.
- Put commands in ``` code blocks. In the note each block gets **Send to terminal** (typed at the prompt of a shell on the note's host, opening one if needed; you press Enter) and **Copy**.
- **Runbooks for alerts**: tick **Runbook for alerts** in the editor and pick the alert kinds it's for, and optionally a host (the note's host) and a service. Matching alerts get **▤ Runbook** in the alerts list, which opens it; alerts without one offer **+ Runbook**, which starts a note for that kind of alert with a step list. A runbook with no kinds, host or service shows on nothing. **✦ Triage** tells DRIFT about your runbook too.
- **Steps**: `- [ ] step` lines become checkboxes you tick while you work (they reset when you open the note again), and `[label](view:fleet)` becomes a link to that view (any view's id: overview, terminal, files, docker, services, tunnels, system, fleet, desktops, network, automation, notes, settings).
- Notes never store passwords or tokens: saving refuses them, and **Mask them** swaps the values for `••••`. Notes travel in encrypted backups.
- DRIFT searches your notes along with its guide and says which notes it used. Tick **Hide from DRIFT** on a note to keep it out.

## Network map

- `Mod+0` opens **Network**: a map with the internet on the left, then your router and tailnet, your hosts, and other devices on the network.
- Pick a host under **Scan from**, or **This computer** (macOS and Linux), and click **Scan**. It reads that machine's neighbour table, mDNS announcements (`avahi-browse`), Tailscale peers, router port forwards (`upnpc`), Tailscale Funnel and cloudflared tunnels; tools that aren't installed are listed as notes. **Ping sweep** also pings each address in its subnet (private /24 or smaller) to find quiet devices. Nothing is scanned until you click.
- **Red** lines and the ▲ count mark ways in from the internet: a public address on the host, a port forward on the router, a Funnel or a tunnel. Nightgrid finds these from your own machines and never asks an outside service. Cyan lines are the LAN, dashed lines the tailnet; a host with a dashed outline wasn't seen in the last scan.
- Select a host to see how the internet reaches it and its listening ports. **Audit ports** (or **Audit ports on all hosts**) runs the same check as System → Security; exposed ports are marked INTERNET. **Scan from here** rescans from that host.
- Select another device to see its address, MAC, names, services and how it was found. **Add as host** opens the host editor with the address filled in.
- **Label** devices that aren't hosts (a TV, a phone, a printer): a name, a type, `#tags` and a note. The label follows the device by its MAC address (or its address when the scan saw no MAC), shows on the map and in **Labelled devices** (with nothing selected), is included in profile exports, and DRIFT can read it ("what is 192.168.1.44?"). The router can be labelled too. **Clear** removes a label.

- Each device shows its maker from its MAC address (looked up offline); phones and VMs with private random MACs say "Random MAC". A device's IPv6 address is listed with its IPv4 entry.
- Docker containers aren't listed as devices; they're in the Docker view.
- Network shares the host mounts and machines it has open connections to are found even when they don't answer ping; shares show their mount point. **Ping sweep** also tries a few TCP ports on each address.

## DRIFT helper

- DRIFT is a small assistant in a side panel. Open it with `Mod+J`, the **DRIFT** button in the top bar, or **Ask DRIFT** in the command palette.
- **Turn it on** under **Settings → Helper**, then **Download** the model once (about 2.5 GB). The download can be paused and resumed and is checked against a fixed SHA-256.
- It runs entirely on this computer: the GPU on macOS, the CPU on Windows and Linux. Nothing you ask is sent anywhere.
- **Explain something**: select log lines, an error or a file in the terminal or a log view, then open DRIFT. The selection and the current view and host are attached as chips; click a chip to leave it out.
- **Explain buttons**: container logs, the System journal, compose output and Automation results have an **✦ Explain** button, and the file editor has **✦ Ask DRIFT**. It sends the newest lines (or the lines you selected there, or the open file) with a label saying what they are, and asks what's wrong. If the model isn't downloaded yet, the question waits in the box.
- **Show me**: when an answer says where something is, **Show me** switches to that view and outlines the control. It only points; it never clicks or runs anything.
- **What I was given** under each reply shows the exact prompt and the guide sections used. Values that look like passwords or tokens are masked before DRIFT sees them.
- **Stop** ends a reply early; **NEW** starts a fresh conversation. **Free memory** in Settings unloads the model; **Delete model** removes the file.
- **Your hosts**: name a saved host in a question ("what happened to the garage pi?", or just one word of its name) and DRIFT gets what Nightgrid knows about it right now: whether it answers, the monitoring state and last error, when it was last seen and its uptime, and active and recent alerts. A follow-up like "check its history" stays on the same host.
- **Shell history**: tick **DRIFT may read shell history** in a host's editor and DRIFT also reads that host's last 60 bash, zsh or fish commands when you ask about it. Lines that look like passwords or tokens are left out, and nothing is kept. Bash writes its history when a shell exits, so commands from a shell that is still open may be missing.
- **Checks**: before answering, Nightgrid runs exact checks and shows them in a card above the reply. Put a command in backticks ("is it safe to run \`rm -rf $DIR/\`?") or select one line in a terminal for a **command check** (deletes, overwrites, permissions, `curl | sh`, firewalls, shutdowns, `--force` and more). Attach a compose file for a **compose review** (restart policies, unpinned images, ports open on every interface, inline secrets, privileged containers, `docker.sock`). Error messages and exit codes get an offline **error reference** with the next step. A cron expression gets plain words, its next runs and the systemd `OnCalendar=` line, and "make a cron schedule for every Sunday at 4am" gets the expression. The chips in an empty panel start each one.
- **Send to a terminal**: a one-line command in a reply has **TERMINAL**, which types it at the prompt of the host you asked about (or the shell in front). Nothing runs until you press Enter; commands marked DANGER need a second click.
- **Triage an alert**: open the alerts list (the ALERTS counter in the status bar) and press **✦ Triage** on an alert: DRIFT gets the usual causes for that kind of alert, the first checks and where to look.
- **Daily summary**: ask "give me a daily summary" or "what happened overnight?" for a summary of the last 24 hours from what Nightgrid already knows (alerts, restarts, services, and the update, disk and backup checks that ran this session).
- **Runbook notes**: **Save as runbook note** under any answer opens a new note with the question, the checks and the answer to edit and save.
- **Regex**: write `/pattern/` (or "regex \`…\`") and select some lines: DRIFT explains the pattern and says how many lines match.
- With shell history shared, DRIFT also searches older history for the words in your question ("when did I last restart nginx on the nas?").
- **Guided tours**: DRIFT can walk you through a task one control at a time. It puts brackets around the control, says what to do and waits until you've done it (clicked it, opened the view, opened or closed the dialog); for steps that only explain, press **Next**. It never clicks anything itself. **Back**, **Skip step** and **End tour** are on the callout, and Esc ends a tour (when no dialog or palette is open). Tours: Getting around, Add your first host, Customize the Overview, Watch your services, Check fleet health, Back up your setup and Open a VM's screen. They don't need the model: start them from **HERE?** in the DRIFT panel or **Settings → Helper**, and DRIFT offers one in a reply when it fits ("how do I set up backups?"). On first start Nightgrid offers the Getting around tour once.
- **What can I do here?**: **HERE?** at the top of the DRIFT panel lists the controls of the current view that are on screen (click one to have it pointed out) and the tours, those for this view first.
- Answers can be wrong. Commands DRIFT suggests are yours to run, and Nightgrid still asks before anything destructive.

## Power

- **System → SUSPEND… / SHUT DOWN… / REBOOT…** act on the selected host after you type its name. They use sudo like other system actions.
- While a host is suspended, shut down or rebooting, it shows that instead of **OFFLINE**, with why and when it was last seen, and no offline alert fires. Nightgrid also notices when a Linux host goes to sleep or shuts down by itself (it listens for systemd-logind's announcement while it monitors the host). It goes back to normal as soon as the host answers; a reboot that takes over 15 minutes counts as an outage.
- **Sleep window** in the host editor (`23:00-07:00`, this computer's time): inside it, a host that doesn't answer shows **ASLEEP**.
- **GROUP…** (System toolbar) suspends, shuts down or reboots every host in a group, one at a time, after you type the group's name. A host whose sudo needs a password that Nightgrid doesn't remember is skipped and listed; tick "remember" for it once from its own System page, or give it passwordless sudo for `systemctl`.
- **System → Schedules → POWER SCHEDULE**: pick Shut down, Suspend, Reboot or Wake up, a time (the host's own clock) and days (none picked = every day), then **ADD**. The schedule is a systemd timer on the host, so it works with Nightgrid closed; scheduled wake-ups run on the host's **Wake from** host. Each action has one schedule per host; adding it again replaces it. **Remove** deletes the timer.
- **Wake-on-LAN**: put the host's **MAC address** in the host editor (the network map shows MACs), then **WAKE** in the System toolbar or ⏻ on its overview tile. **Wake from** sends the packet from another of your hosts on the same LAN, so you can wake a machine at home from anywhere you can reach that host. Wake-on-LAN must be enabled in the sleeping machine's firmware and network settings.

## Built-in Tailscale

- **Settings → Tailnet → Built-in Tailscale** joins your tailnet from Nightgrid itself, so hosts on it stay reachable away from home without the Tailscale app. It's off until you turn it on.
- **LOG IN** opens Tailscale's login page in your browser; once you finish, the state says **Connected** with Nightgrid's own tailnet name and address. If your tailnet requires device approval, approve it in the admin console. **LOG OUT** removes the device.
- Use a device's name (`server` or `server.tailXXXX.ts.net`) or its 100.x address as a host's address. **DEVICES** lists your tailnet; click an address to copy it.
- **Use subnet routes** (on by default) reaches LAN addresses that another device in your tailnet advertises, for example a home server routing your home subnet. Nightgrid tries the LAN first and the tailnet when the LAN doesn't answer, so a host saved with its LAN address works at home and away. The route must be approved in the tailnet's admin console.
- **Tailnet address** in a host's editor gives a host a second address (its 100.x address or MagicDNS name). It's tried when the main address doesn't answer, and Nightgrid remembers which one worked. The host key is always checked against the main address, so switching doesn't trigger a host-key warning.
- **Advanced**: the device name Nightgrid uses in your tailnet, and a self-hosted control server (Headscale). Saving restarts the client.
- With Built-in Tailscale off, tailnet addresses are reached through the Tailscale app if it's connected, as before.

## SSH keys

- **Settings → SSH keys** lists your key pairs in `~/.ssh` and the keys your SSH agent holds, with their fingerprints, whether a file has a passphrase, and which hosts Nightgrid is set to log in with each. **Copy public key** copies the `.pub` line.
- **NEW KEY…** makes an ed25519 key in `~/.ssh` (file name, comment, optional passphrase). It never overwrites a file. The passphrase only encrypts the file; Nightgrid asks for it when it connects and can remember it in the OS keychain then.
- **CHECK HOSTS** reads `~/.ssh/authorized_keys` on every saved host and fills **WHERE THEY WORK**: a table of hosts and your keys. **+** adds a key to that host (it's skipped if the key is already there), **●** means the host accepts it and removing it asks you to type the host's name. A ringed **●** is the key Nightgrid uses for that host. **OTHER** counts keys on the host that aren't yours; click it to see and remove them.
- **ROTATE A KEY**: pick the key to replace, the new key, and the hosts (the hosts that accept the old key are picked for you). For each host in turn Nightgrid adds the new key, logs in with only the new key, points the host's saved settings at it, and then removes the old key. If a step fails on a host, that host stops there and keeps its old key. With **Remove the old key** on, you confirm by typing the old key's name.
- Nightgrid never edits your `~/.ssh/config`, and private keys never leave this computer.

## Alerts and the tray

- Alerts fire for these conditions:
  - a host is offline for 2 minutes;
  - a disk is over its limit;
  - a container that was running exits with an error;
  - a service is down;
  - a certificate is expiring;
  - SMART reports a warning;
  - a backup job failed or is stale;
  - a UPS is on battery, low, or needs a new battery (Fleet → UPS);
  - (opt-in) a burst of errors or an error never seen before in a container's logs or the journal. **✦ What broke?** asks DRIFT about the recent error lines, on this computer.
- **Dependencies** (Settings → Dependencies): when something is down, alerts for what depends on it are folded under its alert ("explains 3 more alerts") and only the cause notifies. Everything about a host (containers, disks, backups, services checked on it) already depends on that host; add your own links such as "Media server depends on the NAS" or "everything depends on the router" (add the router as a host or a service check). In a chain the furthest-up outage is the cause. If a service goes down while its host is still inside the 2-minute grace for "offline", its notification waits up to 5 minutes for the host's alert, so you get one notification instead of two. Turn folding off with **Fold alerts under their cause**.
- Alerts arrive as desktop notifications, and optionally through an ntfy, Discord or JSON webhook. Quiet hours hold notifications but still show alerts in the app.
- The tray (menu bar on macOS) icon offers quick connect, lets you start and stop tunnels, and shows the alert count.

## Backups and moving between machines

- **Settings → Backup → Export** writes your setup to one file encrypted with a passphrase (an [age](https://age-encryption.org) file). It holds hosts, groups, tags, snippets, playbooks, services, tunnels, integrations, bookmarks and settings. Tick **Include saved passwords and tokens** to bring the keychain entries along.
- **Import** shows what's inside first.
  - **Merge** keeps everything you have and adds what's new.
  - **Replace** swaps your setup for the backup. Type `replace` to confirm.
  - Nightgrid restarts afterwards.
- **Sync folder**: pick a Syncthing, cloud-drive or git folder, and export and import use `nightgrid-backup.age` there in one click. The passphrase is never stored, so keep it somewhere safe. Without it the file can't be opened.
- **Profiles**: an export is your profile. It imports on macOS, Windows and Linux (**Export profile…** / **Import profile…** in the command palette). A reminder card appears when your last export from this computer is older than a week; change the interval or turn it off under **Remind me to export**.

## Phone status page and snapshots

- **Settings → Status page · Snapshot → Serve the phone status page** (off by default) shows a read-only page with your hosts, alerts and service checks at a secret link while Nightgrid runs. Pick where it listens: this computer's **LAN address**, **Tailscale only**, or **every interface** (avoid that one on a machine with a public address). **Copy** the link, open it on your phone and add it to the home screen; it refreshes every 30 seconds. **New link** makes the old one stop working.
- **Save snapshot…** writes the same page as one HTML file you can attach or archive. Host addresses are only included if you tick them.

## Handhelds (ROG Ally, Steam Deck) and controllers

- **Settings → Display → Handheld layout** turns on finger-sized buttons, 130 % scale and an icon-only sidebar. Turn it off to go back to the desktop sizes.
- **Controller navigation** works with any gamepad the system sees:

| Control | Action |
|---|---|
| D-pad / left stick | Move focus (hold to repeat) |
| A | Select the focused item |
| B | Back: close the dialog or menu |
| LB / RB | Previous / next view |
| Start | Command palette |
| Select / View | Collapse or expand the sidebar |

In text fields and terminals, left and right are left to the field; up and down move focus out of it. Typing still needs a keyboard (on the Ally, the on-screen one).

## Look and feel

- Four themes: NIGHTGRID, ICE, BLOODLINE and CUSTOM. CUSTOM lets you pick each palette slot (surfaces, text, accent, info, active, danger, OK, warning, neutral highlight) from a set of curated swatches that are all pre-checked for contrast. **Start from** copies a built-in scheme as the starting point; **Reset** goes back to the default custom palette. The palette is saved with your settings and travels in profile exports. (If you used the old PLAIN DARK scheme, it becomes CUSTOM with effects off until you pick a scheme.)
- **Reduce effects** turns off the glitch text, scanlines and glow. Your OS's reduced-motion setting does the same.
- **UI scale** goes from 90 to 150 %.

## Updates

Nightgrid checks GitHub Releases after start-up and offers **Install & restart** when a newer version is out. Turn this off, or check by hand, under Settings → Updates.

## Privacy

There's no telemetry. Nightgrid only connects to:
- your hosts;
- the services and integrations you added;
- GitHub Releases, for the update check;
- your webhook, if you set one;
- Tailscale's control server and relays (or your Headscale), only while Built-in Tailscale is on. Its log upload is switched off.

Secrets are kept only in the OS keychain (or inside an encrypted backup, if you ask for that) and never in the database, logs or config files.

## Troubleshooting

- **"needs a login"**: open a terminal on that host once so you can answer the password or host-key prompt. Background features never prompt.
- **Keychain unavailable** (Linux without GNOME Keyring or KWallet): everything still works, but "remember" options can't save.
- **Blank window on Linux with NVIDIA**: see the README section on WebKitGTK.
- **Logs**: the folder is shown in Settings → Diagnostics. Set `NIGHTGRID_LOG=debug` for more detail.
