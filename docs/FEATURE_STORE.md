# Feature store

A slim core app plus optional components installed from **Settings → STORE**, each patchable on its own between app releases. Decisions: D-205 (part 1) and later D-numbers per part.

## What's core and what's a component

Core is the standard homelab essentials, the things nearly every user of a homelab tool expects:

hosts and SSH, Overview, Terminal, Files, Docker, Services (health checks), System (one host), Fleet updates / disks / backups / uptime, Tunnels, Network tools, alerts and notifications, Knowledge (notes), Library, Settings.

Everything else ("non industry standard") is a component:

| Shelf | Components |
| --- | --- |
| Virtualization | Proxmox (with sandboxes), VM desktops (VNC/RDP) |
| Monitoring | UPS, log search, expiry watch, power & cost, capacity planner, incident replay, Nightgrid Node |
| Operations | automation, journal, blueprints, service catalog, restore drills |
| Network | network map, speed tests |
| Security | security (posture + secrets), access audit |
| Extras | DRIFT helper, UI sounds |
| Integrations (part 3) | one component per integration family (media, downloads, DNS filters, storage, monitoring…) |

The list lives in `core/src/components.rs` and `ui/src/lib/storeCatalog.ts` (kept in step by a test).

## Direction: Rust stays in core

Roben chose this on 2026-11-21. Rust backends stay compiled into the app; a component that isn't installed is inert (no commands called, no pollers, no alert rules, view never loaded). What a component carries independently of the app:

- its **interface** (the Svelte view, as a separately built ES module),
- its **Library pages**,
- its **host scripts** (the shell/PowerShell it runs on hosts),
- over time, **logic moved out of Rust** into the component (parsers and checks that only need the generic core API: run a command over SSH, HTTP request, key/value store).

The rejected alternative was WASM backends via wasmtime: fully patchable backends, but ~10 MB of runtime and porting every niche backend to a capability ABI before anything ships. The release train already turns a Rust fix into a signed release in ~20 minutes.

## Parts

1. **Store and install state (D-205).** STORE tab, install / remove, `AppSettings.installed_components`, migration (existing users keep every component with a feature on; a first launch gets the core only), unknown ids kept, everything gated through `AppSettings::feature`. Components are still bundled with the app.
2. **Signed component packs on their own channel.** Per component: `component-<id>.json.gz` + `.sig` (minisign, release key, same verification as kb packs and interface patches) + unsigned `.meta.json`, all on the rolling prerelease `components-latest` with a signed `index.json`. CI (`components.yml`) builds only components whose files changed since the last pack, so a fix to Proxmox ships just the Proxmox pack. Install downloads the pack when one is newer than the bundled copy; the app re-verifies on every load and falls back to the bundled copy on any failure (60 s ready watchdog, as for interface patches).
   - **Compatibility without pinning to one app build.** Interface patches pin to the exact app version; component packs must survive app updates. Each pack lists the Tauri commands it calls with a per-command signature hash (computed from `bindings.ts` at build time); the app computes the same hashes at build and refuses a pack whose commands changed. Shared interface code is imported through stable host modules (`/host/svelte.js`, `/host/lib/*.js`, an import map in `index.html`), and the pack lists the host exports it uses plus the Svelte major.minor it was compiled with.
   - **Serving.** Pack files are served same-origin through the existing asset wrapper (`app/src/uipatch_cmd.rs` `UiAssets`) under `/components/<id>/…`, so the CSP and command allow-list stay as compiled.
3. **Integrations as components.** Feature ids per integration family, then store entries.
4. **Slimmer bundle.** Once packs have proven themselves, drop the bundled interface of components from the app build so they really are download-only (the Rust side stays).
5. **Logic out of Rust, case by case.** Move parsers of niche features into their component behind the generic core API, so fixes to them ship as component patches.

## Security

- Only this repository's releases, only the release key: every pack and the index are minisign-verified before they're stored and again on every load. No third-party components, no other sources.
- A pack only adds web assets, Library pages and host scripts. Capabilities, the CSP and the command allow-list are compiled into the app, so a pack can't widen what the app may do.
- Host scripts in a pack are listed with SHA-256 in the signed manifest and run only through the same paths as built-in scripts.
- A newer index is required to replace an older one, so a withdrawn pack can't be replayed.

## Release rules

Component patches follow the project's release rules: each pack carries notes shown in the store (the in-app changelog for that component), and the store shows the component's version and when it was patched. App releases keep their codenames; packs are named by component and date.
