# NIGHTGRID

Agentless homelab command center: SSH terminals, SFTP, Docker, monitoring, services and tunnels in one native-feeling desktop app. Built with Tauri 2, Rust and Svelte 5.

> Status: **M10 — DRIFT helper and backlog** (0.10.3: security score and port map). Hosts + terminals, SFTP + editor, Docker, monitoring, services, tunnels, system tasks, alerts, integrations, automation, encrypted backups, a handheld/controller layout and the local DRIFT helper are in. Next: more of the M10 backlog and the offline knowledge base (M11). How to use it: [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md).

## Repository layout

| Path | What |
|---|---|
| `core/` | `nightgrid-core` Rust library: storage, secrets, SSH/SFTP, Docker, metrics, services, automation, backups. UI-agnostic, unit tested. |
| `app/` | Tauri 2 crate: window, commands, logging, platform workarounds. |
| `ui/` | Svelte 5 + TypeScript frontend (Vite root). |
| `docs/` | `USER_GUIDE.md`, `ARCHITECTURE.md`, `DECISIONS.md`. |

## Prerequisites

- Rust stable (1.80+) — <https://rustup.rs>
- Node 22+ and pnpm 10 (`corepack enable` picks up the pinned version)
- Platform webview deps:
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Windows:** WebView2 (preinstalled on Windows 11) and the MSVC build tools.
  - **Linux (Debian/Ubuntu):** `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev patchelf`
  - **Fedora:** `sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel dbus-devel`
  - **Arch/CachyOS:** `sudo pacman -S webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg openssl dbus base-devel`

## Run

```sh
pnpm install
pnpm tauri dev      # app with hot reload
pnpm dev            # frontend only, in a browser at http://localhost:1420 (backend is mocked)
```

## Test & lint

```sh
pnpm check && pnpm lint && pnpm test       # svelte-check, eslint, vitest
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
NIGHTGRID_TEST_KEYRING=1 cargo test -p nightgrid-core secrets   # also exercise the real OS keychain
```

`ui/src/lib/bindings.ts` is generated from the Rust commands (tauri-specta). After changing a command or a type it exposes, run `cargo test -p nightgrid --lib bindings` and commit the result; CI fails if it's stale.

## Build installers

```sh
pnpm tauri build                       # all bundle targets for the current OS
pnpm tauri build --bundles app,dmg     # macOS
```

Prereleases with all installers are on the [Releases](https://github.com/TheRoben-Dev/nightgrid/releases) page. CI also builds an NSIS installer + portable zip (Windows), a `.dmg` (macOS arm64) and `.deb`/`.rpm`/AppImage (Linux) on every PR; grab them from the run's artifacts. Flatpak and AUR packages are skipped on purpose (D-061); the AppImage runs on Arch/CachyOS.

### Updates

Nightgrid checks GitHub Releases shortly after launch and offers **Install & restart** when there's a newer version (Settings → Updates to turn it off or check now). Self-update needs the release signing key in the repo secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and its public half in `app/updater.pub`; generate them with `pnpm tauri signer generate -w ~/.tauri/nightgrid.key`.

### macOS: opening an unsigned build

CI builds are not signed or notarized yet, so Gatekeeper will say the app is damaged or from an unidentified developer. After copying `Nightgrid.app` to `/Applications`:

```sh
xattr -dr com.apple.quarantine /Applications/Nightgrid.app
```

### macOS: testing M1 (hosts + terminal)

1. Open the app, press `Cmd+2` (Terminal) and click **+ LOCAL**: your login shell opens. Try `Cmd+Shift+D` (split right), `Cmd+Shift+S` (split down), drag the divider, `Cmd+Shift+F` (find), select text (it's copied) and right-click (paste).
2. In the sidebar, click **IMP** to import from `~/.ssh/config`, or **+** to add a host. "SSH agent" is the default and uses `SSH_AUTH_SOCK` (macOS's built-in agent works out of the box, e.g. keys added with `ssh-add --apple-use-keychain`). Agents configured only through `IdentityAgent` in `~/.ssh/config` (1Password, Secretive) aren't picked up yet; use "Key file" for those hosts for now.
3. Click a host: the first time you'll see its fingerprint; check it and press **Trust & connect**. With password/key auth you'll be prompted; tick "Remember in OS keychain" only if you want it stored.
4. Network drop: turn Wi-Fi off for ~10 s with a session open. The pane shows "reconnecting"; when Wi-Fi is back it reconnects and prints `[nightgrid] reconnected`.
5. Changed host key (optional): edit the host's line in `~/Library/Application Support/dev.nightgrid.Nightgrid/known_hosts` to a different key and reconnect. You should get the red screen, and it only proceeds after typing the host name.

### macOS: testing M2 (files + editor)

1. Press `Cmd+3` (Files). The left pane is this Mac, the right pane your first favourite host (switch either with the dropdown). The first connect shows the same host-key / password prompts as the terminal.
2. Browse: type a path (`~` works) and Enter, double-click folders, `.*` shows hidden files, ★ bookmarks the folder.
3. Transfers: select files (Cmd-click, Shift-click) and press `F5`, click **COPY →**, or drag them onto the other pane or onto a folder. Try a big file: pause, resume, cancel. Copy the same file again to see the conflict choices.
4. Edit: double-click `compose.yaml` or a `.env` on one of your hosts, change it, `Cmd+S`. The footer says whether the save was atomic. To see conflict protection, change the file from a terminal (`echo >> file`) and save again in the editor.
5. Delete asks with the exact paths; deleting several items on a host needs the host name typed.

### macOS: testing M3 (Docker)

1. Press `Cmd+4` (Docker) and pick a host with Docker in the HOST dropdown. The header shows the engine version. Containers are grouped by compose project with CPU/memory filled in after a few seconds.
2. If you see **NO ACCESS TO DOCKER**, run the command it shows on the server (`sudo usermod -aG docker <you>`), close that host's terminals, and press Retry.
3. Click a container: live logs open on the right. Try search, "only matches", tail and timestamps. Check DETAILS, ENV (values hidden until clicked) and INSPECT JSON.
4. `›_` opens a shell inside the container in a terminal tab. `↻`/`■`/`▶` restart/stop/start; the confirmation names the container. Select several (or a project checkbox) for bulk actions.
5. `Cmd+K` → `Docker: <host>`, `Logs: <name>` or `Shell: <name>`.
6. **STACKS** tab: your compose projects are listed. Open **STACK FOLDERS** and add the folder that holds your stacks (e.g. `/opt/stacks`) to see stopped ones too.
7. On a stack, click `compose.yaml` or `.env` to edit it in the editor. **Pull** and **Up** run straight away; **Update**, **Restart** and **Down** ask first. Watch the output console; **Update** ends with a list of services that got a new image. **Cancel** stops a running command.

### macOS: testing M4 (monitoring)

1. Open the Overview (`Cmd+1`). Within a few seconds each Linux host's tile shows CPU with a sparkline, MEM and DSK meters, network rates and uptime (temperature too on bare metal / a Pi).
2. A host that needs a password or a new host key says **LOGIN NEEDED** and doesn't prompt. Open a terminal on it once (`Cmd+2`), then its tile goes live.
3. Click a host's tile: the monitor shows per-core bars, mounts, disks, interfaces and temperatures. Run `yes > /dev/null &` in a terminal on that host and watch CPU climb; `kill %1` after.
4. **HISTORY**: after a couple of minutes the 1H chart has points; hover for values. Switch CPU / MEMORY / NETWORK / DISK I/O / TEMP and the ranges. History only records while the app runs.
5. Offline: turn Wi-Fi off. The tile goes **OFFLINE · RETRY nS** with a growing interval; `↻` on the tile retries now.
6. Settings (`Cmd+,`) → **Monitoring**: change the sample interval or turn metrics off; tiles follow straight away. `Cmd+K` → `Monitor: <host>` opens the panel too.

### macOS: testing M5 (services + tunnels)

1. **Services** (`Cmd+5`) → **Add service**: name it, paste an http(s) URL of something you run, pick the host it runs on. Within a second the card shows ONLINE/DOWN, HTTP status and latency; https adds a CERT column (hover it for issuer and expiry).
2. Add one with a self-signed certificate: it's DOWN with "certificate not trusted". Edit it, tick **Allow untrusted certificate**: it turns ONLINE with an orange UNTRUSTED badge. Stop a service (or use a wrong port) and press **Check**: DOWN with the reason.
3. **Import Homepage**: pick the host running Homepage, press **Detect** (or type its config folder), **Preview**, then **Import**. Run it again after editing `services.yaml`: changed entries update, your check settings stay.
4. **Tunnels** (`Cmd+6`): in **Open web UI via**, pick a host and type a port that only listens on the host (e.g. `localhost:9090`), press **Open**: your browser opens `http://127.0.0.1:<port>`. A TEMP row appears; **Close** ends it. On a service card linked to a host, **Tunnel** does the same for its URL.
5. **Add tunnel** → SOCKS -D, port 1080, start now. In Firefox set a SOCKS5 proxy `127.0.0.1:1080` (with "proxy DNS"), and `.lan` names on the host's network resolve and load. Traffic counters move.
6. Tick **Auto-start** on a tunnel, quit and reopen Nightgrid: it's UP again. Turn Wi-Fi off: it goes RETRY nS, and comes back UP on its own when the network returns.

### macOS: testing M6 (system)

1. **System** (`Cmd+7`), pick a Linux host. **Units** lists its services; **FAILED** shows only failed ones. Click one: its status appears on the right.
2. Press **Restart** on a harmless service: the confirmation names the unit and host. Confirm, and the sudo prompt asks for your password (a wrong one says so and asks again). Leave **Remember** off: the next action asks again. Tick it once: later actions don't ask, and **Forget sudo password** in the toolbar removes it from Keychain.
3. **Logs** on a unit (or the **Journal** tab): its journal, following new lines (run `logger -t test hello` on the host to see one arrive). Set **LEVEL** to "WARN and worse" and **Apply**. If a hint says you only see your own entries, press **Read with sudo**.
4. **Updates**: the upgradable count and packages for apt/dnf/pacman, plus **REBOOT REQUIRED** with the reason when the host wants one. **Upgrade all…** lists the packages; confirm to see apt/dnf output live. **Reboot…** needs the host name typed.
5. **Disks** → **Read SMART**: one card per disk with health, temperature, power-on time (NVMe: wear and data written). Needs `smartmontools` on the host; without it the tab says how to install it.

### macOS: testing M7a (alerts + menu bar)

1. Look at the menu bar: a small window icon. Its menu shows the alert count, **Connect** (hosts with their status; pick one and a terminal opens), **Tunnels** (tick/untick to start or stop) and **Quit**.
2. **Settings → Notifications → Send test**: a macOS notification "NIGHTGRID · TEST" appears (allow notifications for Nightgrid in System Settings if macOS asks).
3. Stop a running container on a connected host with `docker kill <name>` on the host: within a minute a **CONTAINER DOWN** notification arrives, and the status strip shows **ALERTS 1** in red. Click it: the alert, with **Docker** to jump there. `docker start <name>` and it moves to **RESOLVED**.
4. Add a service pointing at a dead port (e.g. `http://127.0.0.1:1/`): after about 90 s, **SERVICE DOWN**. Turn **Service down** off in **Settings → Alerts** and it disappears.
5. Optional webhook: **Settings → Notifications → WEBHOOK → ntfy**, paste `https://ntfy.sh/<some-random-topic>`, **Save**, **Send test**, and watch it arrive at that URL in a browser.
6. **Settings → Desktop → Keep running when the window is closed**, then close the window: Nightgrid stays in the menu bar; the Dock icon or **Show Nightgrid** brings it back.

### macOS: testing M7b (integrations)

1. **Services → Add integration → Uptime Kuma**: your Kuma URL and the slug of a public status page (create one in Kuma if needed). **Test** shows "Connected: N/M up", **Save**: a card lists the monitors.
2. **Plex**: URL like `http://<server>:32400` and your X-Plex-Token, **Test**, **Save**. Play something: within 30 s the card shows the stream. Reopen the editor: the token is masked, **Show** reveals it.
3. **AMP** (if you have it): panel URL, login name and password. The card lists instances; **Stop** asks with the instance named before doing anything.
4. **Portainer**: URL, endpoint id (usually 1 or 2) and the host it runs on. In **Docker**, select a container on that host: **Open in Portainer** opens the container page in your browser.
5. Check Keychain Access: entries named `integration-<id>` hold the secrets; removing an integration deletes its entry.

### macOS: testing M8 (automation)

1. **Automation** (`Cmd+8`): type `uptime; echo {{msg=hello}} from {{host}}`, click one or more hosts, check the **PREVIEW**, press **Run** (or `Cmd+Enter`). Each host shows OK and its output; click a host to see its lines.
2. Type `ls /nope` and run it: the host turns red with **EXIT 2** and the error in red.
3. **Save as snippet**, name it, pick **One host**. Press `Cmd+K` with a terminal open to that host and type `snippet`: the snippet is listed, along with **Type into terminal**, which pastes it at the prompt without running it.
4. Destructive check: `touch /tmp/ng-x; rm /tmp/ng-x` on two hosts. A red dialog lists `rm` and the hosts and asks you to type **2**; with a whole group picked it asks for the group name instead.
5. **Playbooks → New**: add a command step, an upload (from `~/somefile` to `/tmp/somefile`, permissions `600`), a wait for port (`127.0.0.1`, `22`) and save. Run it: steps tick off one by one, and the upload is confirmed first as an overwrite.
6. A snippet marked **Run as root** on a host that needs a sudo password: leave the password empty and the host fails with "sudo needs your password"; type it in **SUDO PASSWORD** and run again.

### macOS: testing M9a (encrypted backups)

1. **Settings → Backup**: type a passphrase twice (at least 8 characters) and press **Export…**. Save the file; it starts with `age-encryption.org/v1` and is unreadable without the passphrase (`age -d file.age` opens it).
2. Tick **Include saved passwords and tokens** and export again: macOS may ask to allow keychain access for each saved entry.
3. Delete a snippet, then **Import…**, enter the passphrase and pick the file. The preview lists what's inside; choose **Merge**: only the missing snippet comes back and the app restarts.
4. **Import…** again and choose **Replace**: you have to type `replace`. Everything matches the backup afterwards, including the theme.
5. **Choose folder…** under SYNC FOLDER (e.g. a Syncthing or iCloud Drive folder). **Export to sync folder** and **Import from sync folder** now skip the file dialog, and the panel shows when the last backup was written.

### macOS: testing M9b (handheld layout + controller)

1. **Settings → Display → Handheld layout**: buttons, fields and sidebar items get taller, the scale jumps to 130 % and the sidebar collapses to icons. Switch it off: scale returns to 100 %.
2. Pair a controller (Xbox, PlayStation or 8BitDo) over Bluetooth and press any button with Nightgrid in front. The D-pad moves a yellow outline between buttons, A presses, B closes a dialog, LB/RB switch views and Start opens the command palette.
3. Open **Import…** under Backup and use the D-pad: focus stays inside the dialog until you close it with B.

### macOS: testing M10a (DRIFT helper)

1. Press `Cmd+J`: the DRIFT panel opens asleep. **Turn on DRIFT**, then **Download (2.50 GB)**. The bar fills; **Pause** and **Resume download** continue where it stopped. (Activity Monitor shows the file growing in `~/Library/Application Support/dev.nightgrid.Nightgrid/models/`.)
2. Ask "How do I back up my setup?". The first question loads the model (a few seconds), then the reply streams in. **Show me** jumps to Settings and puts magenta brackets around the Backup panel; clicking anywhere clears them.
3. Open a terminal, run `docker compose up` in a broken folder (or `cat /nonexistent`), select the error lines, press `Cmd+J` and ask "What does this mean?". The SELECTION chip shows the line count; the reply explains it. **What I was given** shows the exact prompt; a line like `PASSWORD=x` in the selection appears as `PASSWORD=••••`.
4. **Settings → Helper**: *Free memory* unloads the model; *Delete model…* removes the 2.5 GB file after you confirm.

### macOS: testing 0.10.12 (power states, Wake-on-LAN, network map fixes)

1. **System**, pick a Linux host, **SUSPEND…**, type its name: its overview tile turns **SUSPENDED** (not OFFLINE) with "suspended from Nightgrid" and when it was last seen; no offline alert. Wake it: it turns UP again.
2. Suspend a host from its own power menu or `systemctl suspend` in its terminal: it also shows **SUSPENDED** ("the host announced it was going to sleep").
3. Put a MAC address on a host (host editor), suspend it, then **WAKE** (or ⏻ on the tile). Set **Wake from** to another host and try again.
4. Set a host's **Sleep window** to include the current time and stop it: it shows **ASLEEP**.
5. **Network → Scan** from a Docker host: no 172.x container addresses; devices show their maker; the router appears; a NAS you mount shows "NFS share → /mnt/…" even if it ignores ping.

### macOS: testing M10j (built-in Tailscale, DRIFT knows your hosts)

1. Quit the Tailscale app (or turn it off). **Settings → Tailnet → Built-in Tailscale** on: the state goes to **Needs a login**; **LOG IN** opens the browser. Log in: the state turns **Connected**, and **DEVICES** lists your tailnet.
2. Add a host with a tailnet device's name or 100.x address and open a terminal: it connects through the built-in client. Files, Docker and the monitor tile work too.
3. With **Use subnet routes** on and a device in your tailnet advertising your home subnet (approved in the admin console), go off your Wi-Fi (phone hotspot): a host saved with its LAN address still connects, after a short wait on the first try.
4. Edit a host, set **Tailnet address** (the field suggests your tailnet devices), and go off your Wi-Fi: it connects through the tailnet address; back on the Wi-Fi it uses the LAN again. No host-key prompt either way.
5. Quit and reopen Nightgrid: it reconnects without a new login. **LOG OUT** removes the device; turning the toggle off stops the client.
6. Ask DRIFT "what happened to <one of your hosts>?": the reply talks about that host's status, last seen and alerts (**What I was given** shows a MY HOSTS block). Tick **DRIFT may read shell history** on the host and ask "check its history": the reply cites recent commands.

### macOS: testing 0.10.10 (terminal fixes, key hints, scan from this computer)

1. Open an SSH terminal and run `htop`: the strip under the terminal shows htop's keys; quit and it clears. Try `vim`, `less /etc/hosts`, `journalctl -f`, `tmux`. In a local terminal (`Cmd+Shift+T`), run `top` or `vim`: the strip follows within a second or so.
2. Click **×** on the strip: hints for that program stay hidden; **Show hidden** brings them back. **Settings → Terminal → Key hints** off removes the strip.
3. Ask DRIFT "how do I quit vim?" with vim open: the reply uses the key map.
4. **Network → Scan from → This computer**, tick **Ping sweep**, **Scan**: devices on your Wi-Fi appear, and "This computer" is a node under your hosts.
5. **Terminal stress**: split right and down a few times (`Cmd+Shift+D` / `Cmd+Shift+S`), run `htop` or `fastfetch` in each, then close panes in different orders and switch tabs: every remaining pane keeps its text and fits its space. The app backdrop shows behind the text; **Settings → Terminal → Solid background** switches back.
6. Open a terminal on a host, then click that host in the sidebar from another view: it jumps back to that terminal. Alt-click opens a second one. **+ NEW** lists your hosts; pick one with **Split right**.
7. `Cmd+Shift+F` opens Find; Esc, **×** or the ⌕ button closes it.
8. **Network**: scan, select a device that isn't a host, fill in **Label** (name, type, `#tags`, note) and **Save label**: the map shows the name and type. Rescan: the label sticks. With nothing selected, **Labelled devices** lists it. Ask DRIFT "what is <its address>?".

### macOS: testing M10i (network map, custom scheme, notes in DRIFT)

1. Press `Cmd+0`: **Network** shows your hosts. Pick a host under **Scan from** and click **Scan**: the router, tailnet peers and other devices appear, and any router port forward, Funnel or cloudflared tunnel shows as a red line with a ▲ count on the host it lands on. Turn on **Ping sweep** and scan again to find quiet devices.
2. Click **Audit ports on all hosts**, then select a host: its listening ports are listed, with internet-facing ones in red. Select an unknown device and click **Add as host**: the editor opens with its address filled in.
3. **Settings → Theme → CUSTOM**: pick a surface set, text pair and colours; the app recolours as you click. **Start from → ICE** copies ICE; **Reset** goes back. Quit and reopen: the palette is kept.
4. In **Notes**, write a note such as "The NAS backup runs at 03:00 via restic" and save. Ask DRIFT "when does the NAS backup run?": the reply uses it and shows a **From your notes** chip that opens it. Tick **Hide from DRIFT** on the note, ask again: it isn't used.

### macOS: testing M10h (VMs, status page, snapshot)

1. On a Proxmox or libvirt host: **System → VMs** lists the guests. If **Read as root** shows, click it and enter the sudo password: the rest appear. **Start** a stopped guest; **Shut down** asks first with the guest and host named; **Force stop** needs the name typed.
2. On a running guest, **Console** (VM) or **Shell** (Proxmox container) opens a terminal tab. If it asks, type the sudo password in the terminal itself.
3. **Settings → Status page · Snapshot**: turn on **Serve the phone status page**. A masked link appears; **Reveal link**, **Copy**, and open it on a phone on the same Wi-Fi: hosts, alerts and services, refreshing every 30 s. macOS may ask whether Nightgrid can accept incoming connections: allow it. A wrong token returns "Not found."; **New link** stops the old one working.
4. **Save snapshot…**: pick a file, open it in a browser. Host addresses appear only when **Include host addresses** is on.

### macOS: testing M10g (backup board, proxy routes, log alerts)

1. On a test host with a restic, borg or rsync job in a systemd timer or crontab: **System → Backups → Find jobs on …** lists it. **Add…**, keep the defaults and **Save**: the row shows the last run, size and **OK** (a timer) or the newest file time and `du` size (a path).
2. Edit the job, set **Stale after** to 6 hours on a job that last ran yesterday: it turns **STALE** (yellow). Point a path job at a folder that doesn't exist: **CAN'T READ** with the reason.
3. On a host running Traefik, Caddy or Nginx Proxy Manager: **Docker → Routes** lists each domain, the upstream, the container (click to open it) and days until the certificate expires. A domain whose certificate expires within your alert window is yellow.
4. **Settings → Alerts → Log anomalies** on. With a terminal open on the host, run `logger -p user.err "nightgrid test error"`. The first read (within a minute) only learns. Ten minutes later, after running `logger -p user.err "nightgrid widget exploded"`, a **NEW ERROR** alert appears. With DRIFT on, **✦ What broke?** opens DRIFT with the error lines.

5. **Profile reminder**: with at least one host, set **Settings → Backup · Profile → Remind me to export** to Weekly. On a fresh install the card appears a week after first launch (or a week after the last export). **Tomorrow** hides it for a day; **Export profile…** jumps to the export form, and exporting resets it.

### macOS: testing M10f (snapshots, updates, deploy wizard)

1. **Docker → Stacks**, on a stack that uses a `:latest` image: **Updates** lists each image with a badge. If one says UPDATE, it shows the old → new version, and **What changed** opens GitHub in your browser.
2. **Snapshots → Snapshot now**: a MANUAL entry appears with each service's image id and the files saved. On the host, `ls ~/.local/state/nightgrid/snapshots/<stack>/` shows the copies (`ls -ld` shows `drwx------`).
3. Edit the stack's `compose.yaml` (e.g. change an image tag), run **Up**, then **Snapshots → Roll back…** on the earlier snapshot. The dialog names the compose file and each service; confirm and the output shows the files restored and the services recreated. The list now also has a BEFORE ROLLBACK snapshot.
4. **+ New stack**, image `ghcr.io/linuxserver/syncthing` (or any image), **Next**: ports, volumes and variables are pre-filled. Add a variable `ADMIN_TOKEN` with a value; it stays masked. **Review**: `compose.yaml` has `ADMIN_TOKEN: ${ADMIN_TOKEN}` and the `.env` preview is masked. **Create stack** in a test folder, then **Start it**. On the host `stat -c %a .env` prints `600`. Create it again in the same folder: a red box names the existing files, and nothing is replaced unless you click **Replace them in …**.

### macOS: testing M10e (notes, history search)

1. `Cmd+9` → **New**. Title "Restart stack", host = a test host, body with a ```` ```sh ```` block containing `docker ps`. **Save**: the note renders with **Send to terminal** under the block; clicking it opens (or reuses) a terminal on that host with `docker ps` typed at the prompt, not run.
2. **Edit**, add a line `DB_PASSWORD=hunter22`: a warning appears and **Save** is refused. **Mask them** turns it into `DB_PASSWORD=••••` and it saves.
3. **Settings → Terminal → Shell history search** on. In an SSH terminal, **HIST** appears top right; type `docker`: recent matching commands show with their shell. ↑/↓ and Enter type one at the prompt. Run `export TEST_TOKEN=abc123456789` in that shell, `history -a`, and search `export`: it is counted as hidden, not shown.
4. **Settings → Backup → Export** then open the file in **Import**: the summary lists the notes.

### macOS: testing M10d (security, port map)

1. **System → Security** on a Linux host: a grade and score appear with a list of checks; failures and warnings are on top with what to do and **Copy** buttons. Nothing asks for sudo.
2. On a test host, `docker run -d -p 6379:6379 redis` and click **Re-check**: "Databases aren't open to the network" fails naming `redis on 6379 (container …)`, and with ufw on there's also a Docker/ufw warning. The port list shows 6379 as ALL INTERFACES with the container name. Remove the container afterwards.
3. **Port map · all hosts**: every host gets a row with its grade and reachable ports (magenta = all interfaces, yellow = one address, mint = VPN). A host that needs a password says so instead of prompting.

### macOS: testing M10c (schedules, GPUs, forecasts)

1. **System → Schedules** on a Linux host: timers are listed with their schedule, next run ("in 2h 10m") and last run with `OK` or a red `EXIT n`. Hover a time to see it in the host's clock. **Journal** on a timer jumps to the last 7 days of its service.
2. Add a test job on the host with `crontab -e`: `*/15 * * * * echo hi API_KEY=secret`. Click **Refresh**: it shows as "every 15 minutes", the next run is under 15 minutes away, and the key reads `API_KEY=••••` until you click **Reveal**.
3. On a host with an NVIDIA (driver installed) or AMD GPU, open its monitor from the Overview: a **GPU** panel shows busy %, VRAM, temperature and power, and the history bar gets a **GPU** tab.
4. After a host has a few days of history, the monitor's Disks panel shows a line like "Fullest disk: reaches 100% in about 7 weeks" (or "steady").

### macOS: testing M10b (Explain buttons)

1. With DRIFT on and the model downloaded, open **Docker**, pick a container and stay on **Logs**. Click **✦ Explain**: DRIFT opens, your question shows `+ DOCKER LOGS: <NAME> · N LINES` under it and the reply explains the log. Select a few lines first and click again: only those go (`…, SELECTED LINES`).
2. **System → Journal**, filter to a unit, click **✦ Explain** in the footer. Then **Docker → Stacks**, run *Pull* on a stack and click **✦ Explain** in the output header when it finishes (a failed run asks what went wrong).
3. **Automation**: run `systemctl status nonexistent` on a host, pick the host in Results and click **✦ Explain**.
4. **Files**: open a config file in the editor and click **✦ Ask DRIFT**. Turn DRIFT off under Settings → Helper: the buttons disappear.

### macOS: testing ASCII host icons

1. Hover a host in the sidebar, click `…` (edit), and under **ASCII ICON** pick a preset (e.g. Server) or type your own. The preview shows it above the name in the host color.
2. Save. The icon appears above the name on the Overview (`Cmd+1`) tile and on the Terminal (`Cmd+2`) connect tiles.

### macOS: testing "edit server files in Nightgrid"

1. Settings (`Cmd+,`) → Terminal → turn on **Edit server files in Nightgrid**, then open a new terminal on a host.
2. Run `nano ~/test.txt` (or `crontab -e`, `git commit`). An editor tab opens with normal shortcuts while the terminal waits.
3. Edit, then click **Save & return**: you're back in the terminal and the command continues. Pressing Enter in the terminal stops waiting.

### Linux: blank or glitchy window (WebKitGTK)

On the NVIDIA proprietary driver, NIGHTGRID disables WebKitGTK's DMA-BUF renderer automatically. To force it either way:

```sh
NIGHTGRID_DISABLE_DMABUF=1 nightgrid   # turn it off on any GPU
NIGHTGRID_FORCE_DMABUF=1 nightgrid     # keep it on even with NVIDIA
```

Setting `WEBKIT_DISABLE_DMABUF_RENDERER` yourself always wins. Settings → Diagnostics shows the current state.

## Where things live

| | macOS | Windows | Linux |
|---|---|---|---|
| Data (SQLite) | `~/Library/Application Support/dev.nightgrid.Nightgrid/` | `%APPDATA%\dev.nightgrid.Nightgrid\` | `$XDG_DATA_HOME/dev.nightgrid.Nightgrid/` |
| Logs | `~/Library/Logs/dev.nightgrid.Nightgrid/` | `%LOCALAPPDATA%\dev.nightgrid.Nightgrid\logs\` | `$XDG_DATA_HOME/dev.nightgrid.Nightgrid/logs/` |
| Secrets | Keychain | Credential Manager | Secret Service (GNOME Keyring / KWallet) |

Set `NIGHTGRID_LOG=trace` (any `tracing` filter) for verbose logs.

## License

MIT
