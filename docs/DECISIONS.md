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

