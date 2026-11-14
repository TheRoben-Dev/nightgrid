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

