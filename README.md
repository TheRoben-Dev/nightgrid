# NIGHTGRID

Agentless homelab command center: SSH terminals, SFTP, Docker, monitoring, services and tunnels in one native-feeling desktop app. Built with Tauri 2, Rust and Svelte 5.

> Status: **M4 — monitoring**. Hosts + terminals, SFTP + editor, Docker + compose stacks and live host monitoring are in; the rest lands milestone by milestone (see [`PROJECT_PROMPT.md`](PROJECT_PROMPT.md) §9).

## Repository layout

| Path | What |
|---|---|
| `core/` | `nightgrid-core` Rust library: storage, secrets, and (later) SSH/SFTP/Docker/metrics. UI-agnostic, unit tested. |
| `app/` | Tauri 2 crate: window, commands, logging, platform workarounds. |
| `ui/` | Svelte 5 + TypeScript frontend (Vite root). |
| `docs/` | `ARCHITECTURE.md`, `DECISIONS.md`. |

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

Prereleases with all installers are on the [Releases](https://github.com/TheRoben-Dev/nightgrid/releases) page. CI also builds an NSIS installer + portable zip (Windows), a `.dmg` (macOS arm64) and `.deb`/`.rpm`/AppImage (Linux) on every PR; grab them from the run's artifacts. Flatpak and the AUR package come in M9.

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
