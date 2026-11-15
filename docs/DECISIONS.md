# Decisions

Non-blocking choices made during development. Newest last. Anything here can be revisited; open an issue or say so in the project thread.

## M0

### D-001 · `rusqlite` + `rusqlite_migration` over `sqlx`
Synchronous, zero-runtime, bundled SQLite (no system lib on any OS, works in Flatpak). `sqlx`'s compile-time checks need a DB at build time and pull in async machinery we don't need for a local file. `rusqlite_migration` tracks the version in `PRAGMA user_version`. `rusqlite` is pinned to 0.41 because `rusqlite_migration` 2.7 hasn't moved to 0.42 yet.

### D-002 · `keyring` v4 with its default (`v1`) API
v4 split into `keyring-core` + per-platform store crates; the umbrella crate's default feature keeps the familiar `Entry` API and enables the macOS Keychain, Windows Credential Manager and the zbus Secret Service store (pure Rust, no libdbus at runtime). Wrapped in our own `SecretStore` trait so we can swap to `keyring-core` + explicit stores later (e.g. a Flatpak-specific choice) without touching callers.

### D-003 · tauri-specta is a release candidate
`tauri-specta 2.0.0-rc.29` / `specta 2.0.0-rc.28` are the only maintained way to generate typed Tauri v2 bindings and the spec asks for them. They're pinned with `=` as upstream recommends. Risk: API churn between RCs; mitigation is that bindings are generated and checked in CI, so a bump either compiles and regenerates cleanly or fails loudly. Fallback if it stalls: `ts-rs` for types + hand-written invoke wrappers.

### D-004 · App identifier `dev.nightgrid.Nightgrid`
Reverse-DNS that's valid for Tauri (letters/digits/hyphens/dots), Flatpak app IDs and macOS bundle IDs, and doesn't end in `.app` (which Tauri warns about on macOS). It sets data/log dir names and the keychain service name, so changing it later means a migration. Rename alongside any product rename.

### D-005 · Toolchain versions
Tauri **2.x** (3.0 is alpha; spec pins 2). Vite 8, Svelte 5, vitest 5, ESLint 10 (flat config), TypeScript **5.9** (not 7: `typescript-eslint` and `svelte-check` don't support TS 7 yet). pnpm 10 pinned via `packageManager`.

### D-006 · Frontend tooling at the repo root, sources in `ui/`
`package.json`, `vite.config.ts`, `tsconfig.json` and the ESLint config sit at the root so `pnpm tauri ...` works from the root; Vite's `root` is `ui/` and it builds to `dist/`.

### D-007 · Visual direction from Roben's references
Sharp 1px hairline rectangles are the default container; chamfers only on primary buttons and the palette. **Magenta** marks selected/active (rail item, theme tile, current breadcrumb segment, slider thumb) with a soft glow; **mint** marks completed/neutral (past breadcrumb segments, checked boxes, filled slider segments); **yellow accent** stays for primary actions and keyboard focus as the spec says. Segmented stepper breadcrumb, 10-tick-style segmented sliders with `N/M` readouts, tile-grid selection cards, decorative barcode strips and hex IDs. All original; no assets or copy from the reference.

### D-008 · All four themes shipped in M0
They're only token sets, so ICE, BLOODLINE and PLAIN DARK ship now (spec lists them for M9). The terminal palette and iTerm2/Windows Terminal scheme import still come later.

### D-009 · UI scale stored as an integer percent
`uiScale: u16` (90–150) instead of a float avoids NaN/precision edge cases and types cleanly to TS.

### D-010 · Strict CSP without `'unsafe-inline'`
Svelte components avoid static `style=""` attributes (use `style:` directives), so `style-src 'self'` holds. Verified with a headless run under the same CSP: no violations.

### D-011 · Keychain integration test is opt-in
CI runners often lack an unlocked keychain/Secret Service, so the real-keychain round trip runs only with `NIGHTGRID_TEST_KEYRING=1`. The in-memory store covers the trait contract in CI.

### D-012 · Tray icon deferred to M7
The spec lists the tray as P0 but schedules it in M7 with notifications. M0 ships no tray to keep the capability set minimal.

### D-013 · Runtime `<style>` elements mirrored into constructable stylesheets
xterm.js injects `<style>` elements, which a CSP without `'unsafe-inline'` blocks (Chromium and WebKit check CSP for any `<style>`, whatever its `type`). Instead of loosening the CSP, `ui/src/lib/term/cspStyles.ts` makes `document.createElement("style")` return an inert `<nightgrid-style>` element and mirrors its text into `document.adoptedStyleSheets`, which CSP allows. Verified in Chromium and WebKitGTK with zero violations. Needs Safari 16.4+ / WebKitGTK 2.38+ for constructable stylesheets (macOS 11+ has it through Safari updates). Libraries that read `HTMLStyleElement.sheet` would not work with this.

### D-014 · Keep `freezePrototype`, rewrite xterm's prototype-shadowing assignments
With `Object.prototype` frozen, strict-mode code like `ns.toString = fn` throws, and xterm.js does exactly that at module load (bundled VS Code helpers), leaving a blank window. Rather than turn the hardening off, a small Vite plugin (`ui/tools/freezeSafe.ts`) rewrites such assignments in `@xterm/*` to `Object.defineProperty`. A test runs the rewrite over the installed xterm packages so an upgrade that changes the pattern fails CI instead of shipping a blank window.

### D-015 · Terminal keyboard rule
Shells need `Ctrl+K`, `Ctrl+B`, `Ctrl+W` and friends. On Windows/Linux, while a terminal has focus, the app only takes chords with Shift (plus `Ctrl+Tab`); the palette has a `Mod+Shift+K` alternative. On macOS, Cmd chords always go to the app since shells never see Cmd. Copy/paste on Windows/Linux is `Ctrl+Shift+C/V`.

### D-016 · Secrets entered at a prompt are cached in memory for the session
So reconnects and new panes don't re-prompt. The cache is per process, keyed by hop label, zeroized on drop, and cleared for a host when its connection settings change, it is deleted, or "remember" is turned off. Only an explicit "remember" writes to the OS keychain.

### D-017 · NIGHTGRID keeps its own `known_hosts`
New and accepted keys go to `<app_data_dir>/known_hosts`; `~/.ssh/known_hosts` is only read (as a fallback source) or copied on request, in line with "never write to the user's SSH config without asking". A key changed in either file blocks the connection.

### D-018 · Reachability is a TCP connect to the SSH port
ICMP needs privileges on most OSes. A TCP connect to port 22 (or the host's port) every 30 s while the window is visible gives online/offline plus latency without authenticating. Hosts behind a ProxyJump show "unknown" unless already connected.

### D-019 · SSH integration tests use an in-process server plus a real `sshd`
Docker isn't available on macOS/Windows runners, so the default suite runs an in-process russh server on every OS, and `core/tests/ssh_openssh.rs` starts a real OpenSSH `sshd` as the current user on Linux and macOS CI (`NIGHTGRID_TEST_OPENSSH=1`) to catch interop issues.

### D-020 · SFTP via russh-sftp on the pooled connection
`russh-sftp` speaks SFTP over any channel stream, so each host gets one `sftp` subsystem channel on its existing pooled connection (cached per connection; a reconnect opens a new one). Transfers between two hosts stream through the app rather than needing host-to-host SSH trust.

### D-021 · Atomic save uses `mv -f` over SSH for the final rename
SFTP v3 `RENAME` refuses to replace an existing file on OpenSSH, and russh-sftp has no `posix-rename@openssh.com`. The editor writes `.<name>.nightgrid-<id>.tmp` with the original's mode (and owner, via `setstat`), then runs `mv -f -- '<tmp>' '<file>'` on the same connection (one `rename(2)`, paths shell-quoted). Plain SFTP rename is tried first for servers that allow it. If the temp file can't be created (folder not writable) or the owner can't be kept, it falls back to rewriting the file in place, which keeps owner and mode but isn't atomic; the UI reports which happened.

### D-022 · Save conflicts are detected by size + mtime
The editor remembers the file's size and mtime when it opens (or last saved) the file and refuses to save over a file whose stamp changed. mtime has one-second resolution over SFTP v3, so two edits within the same second with the same size can slip through; that's an accepted trade-off against hashing every file before save.

### D-023 · Transfer conflicts are decided per top-level item
A job is one selected file or folder. If the destination exists, the job waits for "overwrite / skip / keep both" (or follows "apply to all"). Overwriting a folder merges into it and replaces files inside. Resume and retry skip the check and continue each file from the size already at the destination, so a retried job never asks twice.

### D-024 · CodeMirror is code-split
The editor and its language packs load on first use, keeping the main bundle under the 900 kB warning limit. CodeMirror injects its styles through constructable stylesheets, which our CSP allows, and none of its packages assign over `Object.prototype` members (checked against `freezePrototype` in a Chromium run).


### D-025 · Docker API over per-request SSH channels
bollard's custom transport hands us each HTTP request. We open a fresh `direct-streamlocal` channel to the daemon socket on the host's pooled SSH connection, run HTTP/1.1 over it with hyper, and let the channel close when the response body is done. That keeps the client stateless across reconnects (the per-host `DockerCache` is keyed to the pooled connection) and lets long log streams run beside short calls. Stats requests run 6 at a time so a big host doesn't use up sshd's session limit.

### D-026 · Socket discovery and dial-stdio fallback
On first use a small `sh` probe looks for a readable+writable socket at `/var/run/docker.sock`, then `$XDG_RUNTIME_DIR/docker.sock` (rootless), and checks whether the `docker` CLI exists. If the socket is there but not accessible, we stop with a "join the docker group" message instead of trying sudo. Being in that group is root-equivalent, and the UI says so. If forwarding fails (`AllowStreamLocalForwarding no`), we use `docker system dial-stdio` over an exec channel. With no socket but a CLI (contexts, a `DOCKER_HOST` in the login environment), dial-stdio is tried directly.

### D-027 · Container shell uses the docker CLI in a PTY
Exec over the API needs a hijacked HTTP connection plus our own TTY resize plumbing. Running `docker exec -it <id> sh -c 'command -v bash … || exec sh'` in a PTY channel reuses the terminal stack (resize, reconnect, copy/paste) for free. It needs the docker CLI on the host, which every normal install has. Container ids/names are validated (`[A-Za-z0-9_.-]`, no leading `-`) and shell-quoted.

### D-028 · Typing the host name only for removing several containers
Rule 5 asks for a confirmation that names the targets, and for typed names on multi-host runs. Docker actions are single-host, so stop/restart/kill/remove always show the exact containers. Only removal of more than one container also needs the host name typed, because removal is the one action you can't undo by starting the container again. Start, pause and unpause run without asking.

### D-029 · Env values travel to the webview but are masked there
`docker_inspect` returns env values, so the panel can reveal them on click without another round trip. They are masked by default in the UI, replaced with `••••••` in the raw JSON view, and never logged (bollard's debug logging is filtered out). They are never written to SQLite.

### D-030 · "Edit in Nightgrid" is a session-only shell hook
Roben wanted normal editor keybinds instead of nano when editing on a server. With the setting on, SSH terminals start through a bootstrap: `echo <base64> | base64 -d | /bin/sh`. That form only uses base64 characters, so any login shell (sh, bash, zsh, fish, csh) runs it the same way. It creates a private `mktemp -d` folder holding an editor helper and starts the user's shell as a login shell: bash via `--rcfile`, which sources `/etc/profile` and the first of `~/.bash_profile`/`~/.bash_login`/`~/.profile`; zsh via a `ZDOTDIR` shim that sources the real dotfiles; fish via `-C`. It defines a `nano` function and sets `EDITOR`/`VISUAL` to the helper, and removes the folder when the shell exits (folders left over from a dropped connection are cleaned after a day). The helper prints `OSC 7766;<token>;<base64 abs path>` and waits with echo off until the app types the token back (tab closed) or the user presses Enter. The token is random per terminal start, so file contents that happen to contain the sequence don't open anything. The app opens files only on that same host, over SFTP as the SSH user. `sudo nano` still runs real nano; `sudoedit` works because it edits a user-owned temp copy. The hook is off by default because it changes how the remote shell starts (for example `logout` says "not a login shell" in bash), and it is never used for container shells.

### D-031 · Editor keybinds stay CodeMirror's standard keymap
The built-in editor already uses the platform-standard bindings: CodeMirror's default, history, search and fold keymaps (`Mod+Z`, `Mod+Shift+Z`/`Mod+Y`, `Mod+F`, `Mod+A`, Shift+arrows, Home/End), plus `Mod+S`. No vim/emacs modes and no nano-style chords.

### D-032 · Compose runs the CLI on the host, not the API
Compose is a client-side tool, so stacks run `docker compose` (or `docker-compose`) in the stack folder over an SSH exec channel, with `exec 2>&1` so progress and errors arrive in order. For rootless sockets we set `DOCKER_HOST` the same way as for dial-stdio. The project name, folder and compose files come from the container labels (or the folder scan), are checked (compose's project-name rule, absolute paths without control characters) and are shell-quoted. Cancel sends SIGINT on the channel, like Ctrl-C. "Update" compares each service's image id before the pull and after `up -d`, and lists only the services that changed.

### D-033 · Stack folders are per host, in SQLite
Running containers only reveal stacks that are up. To manage stacks that are down, the user adds folders per host (`stack_dirs` table, migration 0004). A short `sh` scan looks for compose files in each folder and one level below. Paths may start with `~`, which expands on the host. No secrets are involved; `.env` files stay on the host and are only read when the user opens one in the editor.

### D-034 · Which stack actions confirm
Down, Restart and Update interrupt running services, so they confirm with the stack name and host (rule 5). Up and Pull don't stop anything that is running and run straight away. Stack actions are single-host, so no typed name is needed.

### D-035 · ASCII host icons live in the existing `hosts.icon` column
Roben asked for an ASCII icon above each saved host's name, styled like the app. The `icon` column has existed since migration 0001 and was unused, so the art text is stored there directly (no migration, and it travels with the host). Presets are just text the editor fills in, so changing a preset later doesn't change hosts that already use it. The backend normalizes and validates the icon (≤ 8 lines × 32 characters; printable ASCII, Latin-1, arrows, box drawing, blocks, geometric shapes and braille, which are all single-width in monospace fonts; no control characters, so nothing can inject terminal or markup sequences). The UI applies the same rules to warn as you type. Icons render as plain text (white-space: pre), never as HTML.

## M4

### D-036 · Metrics come from one `sh` script per sample, nothing installed
Each sample runs one short POSIX `sh` script on an exec channel of the host's pooled SSH connection. It prints `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, uptime, `uname -sr`, the hostname, `/proc/net/dev`, `/proc/diskstats`, `/proc/mounts`, `df -kP` (wrapped in `timeout 4` where available, so a hung NFS mount can't stall it) and hwmon/thermal sensor readings, each after an `@@section` marker. Parsing happens in Rust (`core/src/metrics/parse.rs`) and is forgiving: a missing section only blanks that metric, so BusyBox, old kernels and containers work. Rates (CPU %, network, disk I/O) are computed in the app from consecutive raw snapshots, so the script keeps no state on the host. Virtual interfaces (veth, docker, bridges, CNI) and virtual block devices (loop, zram, dm, md) are left out of the totals; pseudo filesystems and Docker/snap overlay mounts are left out of the disk list. Linux only for now; other OSes show "no metrics".

### D-037 · History is minute points plus 15-minute rollups in SQLite
Samples are averaged per host into one point per minute (`metric_points`, res 60, migration 0005). Each minute point also updates a running average in a 15-minute row (res 900), so long ranges never scan minute data. Minute points are kept for **Settings → Monitoring → Keep detail** days (default 7, max 90) and rollups for at least 90 days; pruning runs hourly. Ranges up to 2 days read minute points, longer ones read rollups. Only numbers are stored: no hostnames, mount names or process data. History is recorded only while Nightgrid runs.

### D-038 · Polling follows what's on screen and never prompts
Every saved host gets a poller while metrics are on. Hosts shown on the Overview or in an open monitor are sampled at the fast interval (default 2 s); the rest at the background interval (default 60 s), which also applies when the window is hidden. Offline hosts back off from 5 s to 5 min, and Retry wakes them. A poller never shows a prompt: if a host needs a password, a passphrase or a host-key decision it goes to "login needed" and waits until a terminal or file pane has connected that host, then reuses the pooled connection. This keeps background work from surprising the user with dialogs and keeps security rules 2 and 3 in the user's hands.

### D-039 · Float fields are exported to TypeScript as `number`
specta exports `f32`/`f64` as `number | null` (because of NaN/Infinity). Metric structs never contain NaN (parsers drop non-finite values), so their float fields are annotated `specta(type = u32)` to get a plain `number` in `bindings.ts`. The annotation only changes the generated type; serde still writes the float.

### D-040 · App icon is a terminal window in the theme colours
Roben picked a terminal window with a title-bar stepper, drawn with the NIGHTGRID tokens in the roles the UI gives them: mint hairline frame, magenta for the active step and cursor, mint for the finished step, cyan for the prompt. The tile follows the macOS grid (an 824 px rounded square on a 1024 px canvas), so it sits right in the Dock next to other apps; Windows and Linux use the same art. `app/icons/source.svg` is the source of truth; regenerate the platform files with `pnpm tauri icon app/icons/source.png` (render the SVG to a 1024 px PNG first) and keep only the files `tauri.conf.json` lists.

### D-041 · Panel sizes live in localStorage, not settings
Roben wanted adjustable workspace borders. One `Resizer` component drives every splitter (pointer drag, arrow keys, Home/End, Enter/double-click to reset) against a small layout store with a min, max and default per size. Sizes are per-machine layout preferences, so they're kept in the webview's localStorage (`nightgrid.layout.v1`) rather than in SQLite settings. A laptop and a desktop can then keep different layouts, and nothing sensitive is involved. Split sizes are percentages of the parent, so they survive window resizes; the sidebar width is in pixels.

### D-042 · DRIFT is the LLM helper's mascot
Roben chose DRIFT, a 24×18 pixel HUD drone, from four original concepts and a round of variants. It is drawn only with theme tokens: a dark hull, a mint hairline outline, chamfered corners like the app icon, a forehead stepper, cyan eyes and a magenta thruster. That way it recolours with the theme and fits the UI instead of sitting on top of it. Pixel art was chosen because it stays crisp at integer scales and its states are cheap frame swaps that still work with reduce effects. The reference sheet, frame data and spec are in `docs/mascot/`, and `drift-sprites.json` is the source of truth for the implementation.

### D-043 · Self-update runs from Rust against GitHub "latest"
The Tauri updater plugin is used from Rust only (`update_check` / `update_install` commands); its JS commands are not granted to the webview, so the IPC allow-list stays small (rule 8). The endpoint is `releases/latest/download/latest.json` on the project's GitHub, which counts as "the update server" in rule 7. Because `latest` skips prereleases, releases are now published as regular releases. The CI release job writes `latest.json` from the signed bundles (`scripts/updater-manifest.mjs`), with this version's CHANGELOG section as the notes. The notes are shown as plain text. `requireSignedVersion` is on, so a manifest can't pair a new version number with an older signed bundle. The public key lives in `app/updater.pub`; while it's empty, checks return `notConfigured` and CI builds without updater artifacts, so nothing breaks before Roben generates the key. Linux updates only the AppImage; deb/rpm are left to the package manager. A launch check happens once, 8 s after start, and only when enabled. "Skip this version" is stored in settings.

### D-044 · The scramble effect is part of the identity, used widely
Roben asked for the glitch text animation in more places, so spec §4's "boot, first panel title, alerts only" rule is relaxed. The `scramble` action owns its element's text and re-decodes when the text changes. Duration scales with length (220–650 ms), so labels settle almost instantly. It is applied to view titles, panel and dialog titles, notices and error lines, and host state words. It is deliberately not applied to live numbers, logs, terminal output or file contents. It is fully off under reduce-effects, the plain theme and `prefers-reduced-motion`.

### D-045 · Service checks run from this computer, and capture the certificate even when it isn't trusted
A health check is one HTTP/1.1 GET from the app straight to the service URL (no agent, no SSH): status, time to response headers and, for https, the leaf certificate. TLS goes through rustls with the OS verifier (so internal CAs installed in the system store count as trusted), wrapped by a verifier that records the leaf and the verdict and then lets the handshake finish, so the board can say "self-signed, 40 days left" instead of only "down". Handshake signatures are still checked by the real verifier. An untrusted certificate marks the service down unless the user ticks "allow untrusted", which turns it into a warning. Redirects are not followed and 3xx is healthy by default (`200-399`, editable per service), because login redirects are the norm for self-hosted apps. Checks run every 60 s by default (15 s–15 min), 8 at a time; results are kept 30 days for the latency trend and 24 h uptime. Service icons from Homepage are stored but never fetched (no remote content in the webview, rule 8); cards show a monogram.

### D-046 · Homepage import is a re-runnable sync keyed on group/name
`services.yaml` is read over SFTP from a chosen host (or locally), parsed with yaml-rust2 (nested groups flattened to `Parent / Child`, `href` or else `siteMonitor` as the URL, entries with `{{HOMEPAGE_VAR_…}}` templates or no URL skipped and listed), and ordered by `settings.yaml`'s `layout:` when present. "Detect" looks for a container whose image name contains `homepage` and offers its `/app/config` mount, as a convenience only. Imported rows carry `source = homepage` and `source_key = "group/name"`, so running the import again updates name, URL, group, description and icon from the file while keeping what was set in Nightgrid (checks, healthy codes, untrusted toggle, host link). Entries gone from the file are reported and only deleted when "remove services no longer in the file" is ticked. Nothing is ever written back to Homepage.

### D-047 · Tunnels ride the pooled connection and reconnect until stopped
A running tunnel is one task: get the host's pooled SSH connection (`connect_host`, never prompting), serve, and when the connection drops, retry with backoff (2, 5, 10, 30, 60 s) until the user stops it, like autossh. Problems retrying can't fix (port in use, server refused the remote listen, host needs a login) stop it with the reason. Local and SOCKS listeners are bound before the tunnel reports "up", so a busy port fails immediately. Remote forwards are routed by bound port through a per-connection map that the russh handler consults for `forwarded-tcpip` channels; unknown ports are rejected, and dropping the forward cancels it on the server. SOCKS is SOCKS5 CONNECT without authentication, which is why listeners default to loopback (127.0.0.1 locally, `localhost` on the server); any other bind address shows a "reachable from other machines" warning in the editor and a PUBLIC chip in the list. Auto-start tunnels start when the app opens and again whenever their host gets a new connection. "Open through tunnel" (Tunnels toolbar and service cards linked to a host) creates an unsaved local forward on a free port, reuses it for the same target, and opens the rewritten `http(s)://127.0.0.1:<port>/…` URL in the system browser; an https service will show a certificate name mismatch there, which is expected. Traffic counters are byte totals through the local end, sent once a second while anything runs.

### D-048 · sudo: password on stdin, checked first, prompted per action
Root commands run as `sudo -S -k -p <marker> -- sh -c '<cmd>'` on an exec channel. The password is written to the channel as one line followed by EOF, so it never appears on a command line, in the environment, in `ps` or in a log; `-k` makes sudo always consume that line (a cached timestamp would otherwise pass it to the command's stdin), and EOF makes a wrong password fail at once instead of waiting for a second try. sudo's prompt marker and "Sorry, try again" are stripped from output, and its own failures are classified (needs password / wrong password / not in sudoers / sudo missing) so the UI can react. Before each privileged action one round trip checks `id -u` and `sudo -n true`: root and NOPASSWD hosts never see a prompt. Otherwise the password comes from the prompt for that action, or from the keychain only if "remember for this host" was ticked (off by default, rule 4); it is checked with `sudo true` before the real command starts, so a long upgrade never begins with a bad password, and a stored password that stopped working is deleted and the prompt shown again. Nothing is cached in memory between actions. Commands always go through `sh -c` so the user's login shell (fish, zsh) doesn't matter.

### D-049 · System view scope: services, JSON journal, three package managers, SMART heuristics
- **Units:** service units only (`list-units --all` merged with `list-unit-files`, so disabled services can be started or enabled; templates like `getty@.service` are left out). Stop, restart and disable confirm with the unit and host named; start, reload and enable don't, since they don't interrupt anything. Timers and the user manager are left for the backlog's timer explorer.
- **Journal:** `journalctl -o json` parsed per line (binary `MESSAGE` byte arrays are decoded lossily), streamed in batches, newest 5000 rows kept in memory, follow mode stopped with SIGTERM plus a channel close. When journalctl says only the user's own entries are visible, the view offers **Read with sudo** instead of failing.
- **Packages:** apt (`apt list --upgradable`, with a note when lists are over two days old and a sudo **Refresh lists**), dnf (`check-update`, handling dnf4's wrapped lines) and pacman (`checkupdates`, falling back to `pacman -Qu` with a note). Image-based systems (`/run/ostree-booted`) are detected and left to rpm-ostree/bootc. Reboot required = `/var/run/reboot-required` (with its package list), a running kernel whose modules are gone, or `needs-restarting -r`. Upgrades run non-interactively (apt keeps existing config files, `--with-new-pkgs` but no removals) and stream output; they are not detached from the SSH session, so the dialog warns to stay connected. Reboot needs the host name typed.
- **SMART:** one sudo session runs `smartctl --scan-open` and `smartctl -a -j` per device. Health is the worst of: self-assessment failed or an attribute at/below its threshold (FAILING); non-zero reallocated/pending/offline-uncorrectable/uncorrectable/CRC/spin-retry counts, NVMe media errors, ≥ 90 % endurance used or running hot (HDD > 55 °C, SSD > 65 °C, NVMe > 70 °C) (WARNING); unreadable devices such as unknown USB bridges (UNKNOWN). Each reason is listed in plain words on the disk's card.

## M7

### D-050 · Alerts are derived from existing watchers, with grace periods and a never-prompt rule
Alerts don't add a second monitoring system: one loop every 15 s reads the metrics pollers' states and latest mounts and the service checker's latest results, lists containers every 60 s and reads SMART every 6 h, and only on hosts already in the SSH pool (no background connections, no prompts; SMART runs only when `resolve_sudo` succeeds without input, i.e. root, NOPASSWD or a remembered password). Sources report conditions to a pure `Tracker` (`core/src/alerts.rs`) that owns grace periods (host offline 120 s, service down 90 s, the rest immediate) and a 5-minute clear hold for disks so a mount hovering at the threshold doesn't flap. Each report is scoped by source and, where a source only saw some hosts (Docker, SMART, disk usage of hosts currently sampled), by host, so a host that's offline keeps its other alerts instead of having them "resolve". Containers only alert after they were seen running in this session and then stop with a code other than 0/143 (or restart, or turn unhealthy), so containers kept stopped on purpose never alert. Active alerts and the last 200 resolved ones live in memory only; nothing about alerts is written to disk except the settings.

### D-051 · Delivery: native notifications, quiet hours, webhook URL in the keychain
New alerts become native notifications through `tauri-plugin-notification`, sent from Rust so the webview gets no notification permission. More than three in one round become one summary. Quiet hours (a local-time window that may wrap midnight) hold notifications and webhooks but alerts still show in the app. The optional webhook posts to ntfy (plain body with Title/Priority/Tags headers), Discord (`content` with mentions disabled) or a generic JSON endpoint (`{source, state, alert, headline}`); the URL is treated as a secret (Discord and ntfy URLs act as tokens) and kept only in the OS keychain, with just its scheme and host shown back. Webhook requests use a small shared HTTP client (`core/src/http.rs`, hyper + rustls with the OS verifier) that refuses untrusted certificates before anything is sent. This is a "configured integration" under rule 7: nothing is sent unless the user picks a webhook and saves a URL.

### D-052 · Tray menu is rebuilt only on change; close-to-tray is opt-in
Tauri's tray menus can't be patched in place across platforms, so the menu is rebuilt from a content signature (hosts and their poll state, saved tunnels and whether they run, alert counts) on each alerts round and right after a tunnel is toggled from it. Quick connect and "open alerts" are emitted as events to the UI, which opens a terminal through the normal flow (host-key and password prompts included); the tray never connects by itself. On macOS the icon is a template image so it follows the menu bar's light/dark style; elsewhere it's the app icon. Linux loads libayatana-appindicator at runtime, which panics when it's missing, so tray creation is wrapped and a missing library only logs a warning. "Keep running when the window is closed" defaults off so closing still quits as people expect; it only hides the window while a tray icon exists to bring it back.

### D-053 · Integrations are read on demand from this computer, with secrets in the keychain
Plex, AMP, Uptime Kuma and Portainer are stored as rows in an `integrations` table (kind, name, base URL, login name, status-page slug, Portainer endpoint, linked host, allow-untrusted), never with a token or password: those go to the OS keychain under `integration-<id>`, and the list only reports whether one is saved (rule 1, rule 6: masked in the editor with click to reveal). Status is read by the shared one-request HTTP client (`core/src/http.rs`) only while the Services view is open, every 30 s and paused when the window is hidden, so nothing polls in the background and the only calls go to URLs the user entered (rule 7). Each client parses into a small typed status: Plex `GET /` and `/status/sessions` with `X-Plex-Token`; Uptime Kuma's public status-page APIs (`/api/status-page/<slug>` and `/heartbeat/<slug>`), so no Kuma API key or socket.io login is needed; Portainer `GET /api/system/status` for reachability and version, used only to build "Open in Portainer" links for containers on the linked host (no Portainer token, nothing is managed through it); AMP logs in with `/API/Core/Login` for each read or action and drops the session id afterwards. AMP stop and restart are destructive (rule 5): the UI confirms with the instance named, instance names are validated before they reach a request, and the action is logged with its target; start runs at once. The kind of an integration can't change after it's created, so a stored secret never ends up sent to a different kind of server. Untrusted certificates are refused unless "allow untrusted" is ticked, as with service checks (D-045).
