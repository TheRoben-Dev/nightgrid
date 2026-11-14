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
