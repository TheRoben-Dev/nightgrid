# Project Prompt — NIGHTGRID (Homelab Command Center)

> Working name: **NIGHTGRID** (rename freely). Paste this whole file into Claude Code at the start of the project, or save it in the repo as `PROJECT_PROMPT.md` and tell Claude Code to read it first.

---

## 0. Role & ground rules

You are building a **cross-platform desktop app for managing a personal homelab** with me. I'm technically advanced: I'm a PC/Mac technician, comfortable with terminals, Linux, Docker and networking. I code in Python, C# and Swift, and I'm fine learning Rust/TypeScript as we go.

- Work in **milestones** (section 9). Finish each one fully, with a working build and tests, before starting the next.
- Before writing code in each milestone, give me a short plan and the files you'll touch, then build it.
- Pick **mature, maintained crates/packages**. If a dependency is abandoned or unstable, tell me and propose an alternative.
- Never weaken security for convenience (section 7). When in doubt, ask.
- Keep a `CHANGELOG.md` and a `docs/ARCHITECTURE.md` updated as you go.

---

## 1. What it is

A single native-feeling app that replaces **Termius / Royal TSX / Portainer-for-quick-tasks / a dozen browser tabs** for my homelab. It's **agentless**: everything works over plain SSH, with nothing to install on the servers.

### My environment (design around this)
- **2× Ubuntu servers**. One is named **`Lumen`** and runs a **Homepage (gethomepage.dev)** dashboard plus most Docker services (Plex, AMP game-server panel, ConvertX, etc.). New services are deployed to Lumen.
- **Clients:** Windows desktop (main), MacBook Pro M1 Max (macOS), ROG Ally on **Bazzite** (Fedora Atomic / immutable, KDE, Wayland), and Arch-based machines like **CachyOS**.
- Servers may be reached over LAN or **Tailscale**.

---

## 2. Tech stack (decided — don't change without asking)

| Layer | Choice | Notes |
|---|---|---|
| App shell | **Tauri 2** | Small binaries, native webview per OS |
| Backend/core | **Rust** (workspace: `core/` lib crate + `app/` Tauri crate) | All SSH/Docker/metrics logic lives in `core/`, UI-agnostic and unit-testable |
| SSH | **russh** + **russh-sftp** | Agent auth, keys, jump hosts, port forwarding, streamlocal forwarding |
| Docker | **bollard** over an **SSH-forwarded `/var/run/docker.sock`** (direct-streamlocal) | Fallback: `docker` CLI over SSH if socket forwarding is unavailable |
| Storage | **SQLite** via `rusqlite` or `sqlx` | Hosts, groups, snippets, metric history, settings |
| Secrets | **`keyring`** crate | macOS Keychain, Windows Credential Manager, Secret Service (KWallet/GNOME Keyring) on Linux |
| Frontend | **Svelte 5 + TypeScript + Vite** | Plain CSS with design tokens (no heavy UI kit) |
| Terminal | **xterm.js** + WebGL addon (canvas fallback), fit, search and web-links addons | |
| Editor | **CodeMirror 6** | Editing remote files, YAML/compose/env/nginx/etc. syntax |
| Charts | Lightweight (uPlot or hand-rolled SVG/canvas) | Must stay smooth at 1 Hz updates across multiple hosts |

### Platform-specific requirements
- **Windows 10/11:** WebView2. Support Windows OpenSSH agent (named pipe), Pageant and the 1Password SSH agent. Use `%APPDATA%` for config.
- **macOS (Apple Silicon first, Intel nice-to-have):** WKWebView. Support SSH agent via `SSH_AUTH_SOCK`, including the 1Password and Secretive agents. Unsigned local builds are fine; make notarization an optional CI step.
- **Linux:** WebKitGTK. **Wayland-first** (KDE Plasma), X11 must still work. Known WebKitGTK quirks (DMA-BUF/NVIDIA rendering glitches): detect them and fall back (e.g. `WEBKIT_DISABLE_DMABUF_RENDERER=1`), and document it. Follow XDG base dirs.
  - **Bazzite / Fedora Atomic is immutable** → **Flatpak is the primary Linux package**. Handle Flatpak sandbox permissions correctly: network, `SSH_AUTH_SOCK` passthrough, `~/.ssh` read access, Secret Service portal and notifications portal.
  - **CachyOS / Arch** → provide an **AUR `PKGBUILD`** (`nightgrid-bin`) plus an **AppImage**.

---

## 3. Feature set

Priority: **P0** = MVP, **P1** = soon after, **P2** = later.

### 3.1 Hosts & connections
- **P0** Host list with groups, tags, colors, icons, favorites and search.
- **P0** Import from `~/.ssh/config` (respect `Host`, `HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`). Never write to the user's SSH config without asking.
- **P0** Auth: SSH agent (preferred), key files (with passphrase from keychain), password (keychain, opt-in).
- **P0** Host key verification: TOFU with my own `known_hosts`, plus import from the system `known_hosts`. A **loud red warning screen** when a key changes.
- **P0** Connection status indicators (online/offline/latency) using lightweight checks: TCP connect to port 22, optional ICMP.
- **P1** Jump hosts / bastions (`ProxyJump`, chained).
- **P1** **Tailscale awareness**: if the `tailscale` CLI exists locally, read `tailscale status --json` to show tailnet devices and offer "add as host". Prefer MagicDNS names.
- **P1** Wake-on-LAN (store MAC + broadcast address per host).
- **P2** Windows hosts over OpenSSH, with PowerShell-aware snippets.

### 3.2 Terminal
- **P0** Tabs, **split panes** (horizontal/vertical, resizable), auto-reconnect with a visible state.
- **P0** Local shell tabs too (PowerShell/pwsh on Windows, zsh on macOS, the user's `$SHELL` on Linux), using a PTY via `portable-pty`.
- **P0** Copy on select (optional), right-click paste, search in scrollback, clickable URLs, true color, Nerd Font support, configurable scrollback.
- **P1** **Broadcast input** to selected panes (with an obvious visual indicator that it's active).
- **P1** **Snippets library**: saved commands with variables (`{{container}}`, `{{path}}`), scoped global/per-host/per-group, launched from the command palette.
- **P1** Session logging to file (opt-in per host, with secrets scrubbed where possible).
- **P2** Session recording/playback in asciinema `.cast` format.

### 3.3 Files (SFTP)
- **P0** Dual-pane browser (local ↔ remote, or remote ↔ remote via the app), drag & drop, multi-select.
- **P0** Transfer queue: progress, speed, pause/resume/retry, conflict handling (overwrite/skip/rename/compare).
- **P0** Edit remote files in the built-in editor (CodeMirror) with syntax highlighting. Save writes atomically: temp file + rename, keeping permissions and owner.
- **P1** `sudo` edit mode for root-owned files (e.g. `/etc/...`): copy via SSH with `sudo tee`, with a clear confirmation.
- **P1** Bookmarks per host (e.g. `/opt/stacks`, `/srv/media`, the Homepage config dir).
- **P2** Folder sync (one-way, rsync-like diff, dry-run first).

### 3.4 Docker (per host, agentless)
- **P0** Container list: name, image, status, health, uptime, ports, CPU/RAM, and the **compose project** grouping (from `com.docker.compose.*` labels).
- **P0** Actions: start / stop / restart / kill / remove (with confirmation), **live logs** (follow, search, timestamps, tail N), and **exec shell** into a container (opens a terminal tab).
- **P0** Container detail: env vars (**masked by default**), mounts, networks, labels, restart policy, and the raw inspect JSON.
- **P1** **Compose stacks**: find compose files (from labels' `working_dir`, plus user-defined stack folders). Show the stack, edit `compose.yaml`/`.env` in the editor, and run `up -d`, `pull`, `down`, **"update stack"** (`pull` + `up -d` + show which images changed). Stream output live.
- **P1** **Update checker**: compare local image digests vs the registry and show an "update available" badge. Read-only check, and the update is never automatic.
- **P1** Images/volumes/networks lists with prune (dry-run summary first, then confirmation).
- **P2** Container resource history graphs.

### 3.5 Monitoring (agentless)
- **P0** Per-host live stats via SSH reading `/proc`/`/sys` (no agent): CPU per core, load, RAM/swap, disk usage per mount, disk I/O, network throughput per interface, uptime and kernel.
- **P0** Temperatures from `/sys/class/hwmon` (labels when available), with a graceful "n/a".
- **P0** Overview dashboard: all hosts as HUD tiles with sparklines.
- **P1** **systemd**: unit list with state, start/stop/restart/enable/disable (sudo, with confirmation), and **journal viewer** (`journalctl -fu`, priority filter, time range).
- **P1** **Package updates**: count of upgradable packages (`apt list --upgradable`, and handle `dnf`/`pacman` too), a "reboot required" flag, and run upgrades with live output (sudo, with confirmation).
- **P1** **SMART / disk health** via `smartctl -j` (sudo), with a warning when attributes go bad.
- **P1** Metric history in SQLite (configurable retention, e.g. 7 days at 1-min resolution with downsampling).
- **P2** ZFS/mdadm/btrfs pool status if detected.
- Collection must be **efficient**: one persistent SSH session per host, batched reads, a configurable interval (default 2 s while visible, 60 s in the background), and backoff when hosts are offline.

### 3.6 Services & integrations
- **P0** **Services board**: HTTP(S) health checks (status code, latency, **TLS cert expiry**), an "open in browser" button, and grouping by host.
- **P0** **Import from Homepage**: read `services.yaml` (and `settings.yaml` for groups) from Lumen over SFTP (path configurable) and turn it into service entries. Keep it re-syncable.
- **P1** Integrations as small modules with a common trait (API base URL + token in keychain):
  - **Plex**: server status, active streams/now playing, transcode count.
  - **AMP (CubeCoders)**: instances list, running state, player counts, start/stop/restart instance.
  - **Uptime Kuma** (status pull), **Portainer** (deep links only).
- **P2** Plugin folder for user-written integrations (a manifest + small scripts or WASM, to be decided later).

### 3.7 Tunnels
- **P0** Port forwards: local (`-L`), remote (`-R`) and dynamic SOCKS (`-D`). Save them per host with **auto-start on connect** and status/traffic shown in the UI.
- **P1** "Open web UI through tunnel": one click forwards a remote-only port and opens the browser.

### 3.8 Automation
- **P1** **Multi-host run**: run a command or snippet on selected hosts/groups in parallel, with results per host (exit code, stdout/stderr, duration).
- **P1** **Playbooks** (simple, not Ansible): an ordered list of steps (command, upload file, wait for port, docker action), run with live output and stop-on-error.
- **P2** Schedules (only while the app is running), e.g. a nightly "check updates on all hosts" with a notification.

### 3.9 Notifications & alerts
- **P1** Native desktop notifications for: host offline, container exited/unhealthy, disk > threshold, cert expiring in < N days, SMART warning, service down.
- **P1** Alert rules editable in settings, with sensible defaults. Quiet hours.
- **P2** Optional webhook out (ntfy / Discord) for alerts.

### 3.10 App-wide
- **P0** **Command palette** (`Ctrl/Cmd+K`): jump to a host, run a snippet, open a container's logs, toggle a tunnel, change theme. **Keyboard-first** everywhere, with all shortcuts rebindable.
- **P0** System tray / menu bar icon: quick connect, host status, active tunnels.
- **P0** Settings: theme, fonts, terminal options, polling intervals, data retention, keybindings.
- **P1** **Encrypted config export/import** (hosts, snippets, settings; secrets optional) using **age** with a passphrase, so I can move my setup between Windows, Mac and the Ally. Optional: point at a folder (e.g. a git repo or Syncthing folder) for sync.
- **P1** Auto-update via the Tauri updater (GitHub Releases), on by default but able to be turned off.
- **P2** Localization-ready strings (English first, Swedish later).

---

## 4. Visual design — "corporate netrunner terminal"

Neat, modern and readable first; **very digital** second. Inspired by the *feel* of a Cyberpunk 2077-style HUD/terminal. **Do not use any CD Projekt Red assets, logos, names, fonts or exact UI copies.** It's an original look in that spirit.

### Design tokens (CSS variables, all themeable)
```
--bg-0:     #07080B   /* app background, near-black blue */
--bg-1:     #0D0F14   /* panels */
--bg-2:     #141821   /* raised / hover */
--line:     #1F2633   /* hairline borders */
--text-0:   #E6EDF3   /* primary text */
--text-1:   #8B98A9   /* secondary text */
--accent:   #F3E600   /* signal yellow, primary actions & focus */
--cyan:     #00E5FF   /* data, links, healthy-active */
--magenta:  #FF2A6D   /* highlights, broadcast mode */
--danger:   #FF003C   /* errors, destructive actions */
--ok:       #2BFF88   /* healthy */
--warn:     #FF9F1C   /* warnings */
--glow:     0 0 12px color-mix(in srgb, var(--cyan) 35%, transparent);
```

### Typography
- Data, terminal and numbers: **JetBrains Mono** (or user-chosen mono / Nerd Font). Use tabular numbers.
- Headings and labels: **Rajdhani** or **Chakra Petch** (OFL fonts, bundled locally, never loaded from a CDN). Section labels in UPPERCASE with letter-spacing, e.g. `// SYS.STATUS`, `> CONTAINERS [12]`, `NODE::LUMEN`.

### Shapes & motifs
- **Chamfered corners** (cut 45° corners via `clip-path`) on panels and buttons, mixed with sharp rectangles. No big rounded corners.
- 1px hairline borders, **corner brackets** on focused panels, thin divider ticks, small hex/ID readouts (`0x1F4A`) as decoration in panel headers only.
- Subtle **grid / dot-matrix background** and an optional **scanline overlay** (very low opacity, toggleable).
- Status uses **color + icon + text**, never color alone.

### Motion
- Fast and purposeful: 120–200 ms, ease-out. A brief **glitch/scramble text effect** only on: app boot, panel title on first open, and alert banners. Never on content you're reading.
- Number tickers for live stats, and sparklines that draw left-to-right.
- Optional **boot sequence** splash (< 800 ms, skippable, off after first run if I want).
- **Respect `prefers-reduced-motion`** and add an in-app "reduce effects" toggle that disables glitch, scanlines and glow.

### Readability rules (non-negotiable)
- Body text contrast at least **WCAG AA** against its background. Glow is for accents and focus only, never on body text.
- Information density like a pro tool: compact tables, but click targets of at least 28 px.
- UI scale setting (90–150%) for the Ally's 7" screen. Add an **Ally/handheld layout preset**: bigger touch targets, controller-friendly focus navigation, and a collapsed sidebar.

### Themes
- `NIGHTGRID` (default, as above), `ICE` (cyan/white on blue-black), `BLOODLINE` (red/amber), and a **plain dark** theme for when I want zero effects.
- The terminal palette follows the theme but stays independently configurable (and can import iTerm2/Windows Terminal color schemes).

### Layout
- Left rail: hosts (grouped) + the main sections (Overview, Terminal, Files, Docker, Services, Tunnels, Automation, Settings).
- Top bar: breadcrumb (`NODE::LUMEN > DOCKER > plex`), global search/command palette, connection/tunnel indicators.
- Bottom status strip: active sessions, transfers, alerts, clock — styled like a HUD ticker.
- Tabs and panes are freely arrangeable. Remember the layout per workspace.

---

## 5. Architecture requirements
- `core/` crate: `ssh` (session pool, auth, known_hosts, forwarding), `sftp`, `docker`, `metrics` (collectors + parsers for `/proc` etc., **with unit tests against captured fixtures**), `services` (health checks), `integrations/*`, `store` (SQLite + migrations), `secrets` (keyring wrapper), `config` (export/import).
- **One multiplexed SSH connection per host**, shared by terminal channels, SFTP, metrics and Docker socket forwarding. Reconnect with backoff, and surface the state to the UI.
- Tauri commands for request/response, **Tauri events/channels for streams** (terminal I/O, logs, metrics, transfer progress). Throttle UI updates, and never block the main thread.
- Frontend state: Svelte stores per domain. All backend types generated to TS (e.g. `specta`/`tauri-specta`) so the API is typed end to end.
- Structured logging (`tracing`) to a rotating file in the app data dir, with a "copy diagnostics" button that redacts secrets.

---

## 6. Packaging & CI
- GitHub Actions matrix: **windows-latest, macos-latest (arm64), ubuntu-latest**.
- Artifacts:
  - Windows: **NSIS installer** + portable zip. Prepare a winget manifest later.
  - macOS: **.dmg** (arm64; universal if cheap). Notarization as an optional, secret-gated job.
  - Linux: **Flatpak** (primary; for Bazzite), **AppImage**, **AUR PKGBUILD** for `nightgrid-bin`, and `.rpm`/`.deb` via Tauri bundler.
- Lint and test gates: `cargo clippy -D warnings`, `cargo test`, `svelte-check`, `eslint`, `vitest`.
- **Integration test environment**: `docker compose` with 2–3 `openssh-server` containers (one with Docker-in-Docker, one with password auth, one behind a jump host) so SSH/SFTP/Docker/tunnel features are tested automatically.

---

## 7. Security rules (hard requirements)
1. **No plaintext secrets** anywhere: not in SQLite, config files, logs or exports (unless the export is age-encrypted and I opted in).
2. Prefer the **SSH agent**. Key passphrases and passwords go only into the OS keychain, and only if I tick "remember".
3. **Host key changes block the connection** until I explicitly accept, with a fingerprint diff shown.
4. **sudo**: prompt every time by default. An optional per-host "remember sudo password in keychain", off by default. Never put the password on a command line; feed it via stdin (`sudo -S`) on the channel.
5. **Destructive actions** (rm, prune, kill, down, reboot, shutdown, upgrade, overwrite) need a confirmation that names the exact target(s). Multi-host destructive runs require typing the group/host name.
6. Env vars and integration tokens are **masked by default** in the UI, with click to reveal.
7. **No telemetry.** No network calls except to my hosts, configured integrations, the update server and registries (for the update checker).
8. Tauri: lock down capabilities/permissions per window, strict CSP, no remote content in the webview, and the IPC allow-list kept minimal.

---

## 8. Non-goals (v1)
- No RDP/VNC (maybe P2+), no Kubernetes, no cloud account/backend, no mobile apps.
- Not a full Ansible replacement. Playbooks stay simple.
- No agent installed on servers.

---

## 9. Milestones

**M0 — Skeleton.** Tauri 2 + Svelte 5 workspace, design tokens, the theme system with the default NIGHTGRID theme, left rail/top bar/status strip, the command palette shell, SQLite with migrations, the keyring wrapper, and CI building on all 3 OSes.
✅ *Done when:* an empty app launches styled on Windows, macOS and Linux (Wayland), and CI is green.

**M1 — Hosts + Terminal.** Host CRUD, groups/tags, `~/.ssh/config` import, agent/key/password auth, known_hosts handling, terminal tabs + splits, local shells, reconnect.
✅ *Done when:* I can connect to Lumen with my agent key on all 3 OSes, split panes, and survive a network drop.

**M2 — SFTP + Editor.** Dual-pane browser, transfer queue, atomic remote edit, bookmarks.

**M3 — Docker.** Socket-over-SSH via bollard, container list grouped by compose project, actions, live logs, exec, inspect.

**M4 — Monitoring + Overview.** `/proc` collectors with fixture tests, temps, an overview dashboard of HUD tiles, and metric history.

**M4.5 — App auto-updates** (pulled forward from M9, since prereleases now exist). Tauri updater against GitHub Releases: signed update bundles + `latest.json` published by the release job, check on launch and from Settings, "update available" prompt with release notes, on by default with an off switch and a stable/prerelease channel.

**M4.6 — In-app knowledge base.** Searchable guide articles bundled with the app (offline markdown, no network), rendered in the app theme as plain sanitized text/markdown (no raw HTML). Full-text search, reachable from the command palette (`Help: …`), a Help section, and contextual "?" links in each view that open the matching article. Two shelves: **Using Nightgrid** (one article per feature: hosts & SSH auth, terminal, files & editor, Docker & stacks, monitoring, and every later feature) and a limited **Homelab basics** library (SSH keys & agents, hardening sshd, Docker & compose basics, the docker group, reverse proxies, Tailscale/WireGuard, backups, reading logs). From then on, each milestone ships its feature articles.

**M5 — Services + Homepage import + Tunnels.** Health checks, TLS expiry, `services.yaml` import, saved/auto-start tunnels, "open through tunnel".

**M6 — systemd, journal, package updates, SMART.**

**M7 — Integrations (Plex, AMP), notifications/alerts, tray.**

**M8 — Automation**: multi-host run, snippets with variables, playbooks.

**M9 — Polish & ship**: Ally/handheld layout preset, alternate themes, encrypted config export/import, auto-updater, Flatpak/AUR/AppImage/NSIS/DMG release pipeline, and docs (`README`, user guide, `ARCHITECTURE.md`).

**M10 — Backlog (added 2026-11-14).** All generic: no assumptions about a particular server, stack or service; any specific app is an optional example.
- **Deploy wizard**: pick an image or paste a compose file → suggested volumes, ports, env and labels (from the image's metadata) → review → deploy to any host as a stack.
- **Snapshot & rollback**: before risky actions (compose update/down, config edits, package upgrades), save a copy of the affected compose/.env/config files and offer one-click rollback.
- **Backup status board**: restic / borg / rsync jobs per host, with last run, size and result, and a "stale" flag after a configurable age.
- **Network / port map**: hosts, containers and published/listening ports in one view, with conflicts and exposed ports highlighted.
- **Log anomaly alerts**: spot error bursts and new error patterns in container logs and the journal; optional AI "what broke" summary (opt-in, user-supplied provider, nothing sent without consent).
- **Image changelogs before pulling**: show what changed between the running and the new image (release notes / tags / digest) before an update.
- **VMs**: Proxmox and libvirt VM list with start/stop/reboot and console.
- **Phone status page**: optional read-only status page served by the app on the LAN or Tailscale (off by default, token-protected, no actions).

---

## 10. How to work with me
- Ask me only about real decisions (naming, a UX tradeoff, a security exception). Otherwise choose sensibly and note the choice in `docs/DECISIONS.md`.
- When something needs me to run commands on a server (e.g. checking the docker socket path or Homepage config location on Lumen), give me the exact commands and tell me what output you need.
- After each milestone, give me: what was built, how to run/test it on each OS, known issues, and what's next.
