# Keep Nightgrid simple with feature presets

Hide the views you don't use so the sidebar, palette and DRIFT stay focused, and stop their background checks to save resources. Nothing is ever deleted.

## What you end up with

- Only the views you need in the sidebar
- No background polling for features that are off
- One switch to bring any feature back, with all its data

## Part 1 · Pick a preset

- [ ] Settings → **FEATURES**.
- [ ] **Simple**: terminal, files, Docker, services and System.
- [ ] **Standard**: everything except Proxmox, VM screens, UPS, log search, access audit and speed tests.
- [ ] **Everything**: all of it.

## Part 2 · Fine-tune

- [ ] Flip individual views and checks on or off. Turning on Proxmox for a new server doesn't need anything else.
- [ ] A shortcut to a feature that's off opens this tab with **Turn on and open**.

#### Check

- [ ] The sidebar only shows what you kept, and the palette doesn't offer hidden views.

## Limits and gotchas

- A feature that's off isn't loaded at all: its code, pollers and alerts stop. For example, SMART scans stop with Fleet → Disks off, and auto-start tunnels don't start with Tunnels off.
- The Library is always on.

## Related

- [Add your first host](kb:howto/first-host)
- [Build dashboards with command widgets](kb:howto/command-widgets)
