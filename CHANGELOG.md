# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [SemVer](https://semver.org/).

## [Unreleased]

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
