# Start from a ready-made setup

Not sure what to run on a new server, or just want the common thing done right? Nightgrid ships starters: ready-made setups for typical homelab jobs and runbooks for typical alerts. Each one says in plain words what it's for, and nothing touches a host until you've seen the plan and confirmed it.

## What you end up with

- A working stack (media server, reverse proxy, ad blocking, file sync, monitoring, home automation, a game server or a backup target) set up from [Fleet](view:fleet) → **BLUEPRINTS** → **STARTERS**
- Its passwords in the `.env` vault, generated for you when you leave them empty
- Runbooks for disk full, crash loops, failed units, expiring certificates, unreachable hosts, memory pressure and failed backups in [Knowledge](view:notes)

## Before you start

- A server host with Docker that your user can run (the docker group). Without Docker the plan says so and starting the stacks stays a step for you.
- The hardened Docker host starter needs sudo for its firewall rule and its timer.

## Part 1 · Pick a starter

- [ ] [Fleet](view:fleet) → **BLUEPRINTS** → **STARTERS**. Each card says what it's for and how many containers it brings.
- [ ] Open one. **What it sets up** lists every stack, the ports it opens on your LAN, the ones kept on the host only (open those from Docker → web UIs, through SSH), firewall rules, timers and the steps left to you.
- [ ] **Show the files it writes** shows the exact compose file and any config file.
- [ ] The Library chips open the guide for each app.

## Part 2 · Fill it in and preview

- [ ] Pick the host. The folder for the stacks starts as the host's stack folder, or `stacks` in your home folder; each stack gets its own folder inside.
- [ ] Answer the questions. Defaults are filled in; secrets marked optional are generated when left empty. Everything goes into the stack's `.env` vault, never into the starter.
- [ ] **Preview what it would do** reads the host and lists each step as **NEW**, **THERE**, **DIFFERS** or **BY HAND**, exactly like rebuilding from a blueprint. Untick anything you don't want.

#### Check

- [ ] The plan shows the stack's files as **NEW** and no step you didn't expect.

## Part 3 · Apply

- [ ] **Apply ticked steps…**, read the confirmation, then **Yes, apply**. Files are written, the vault is filled, firewall allow rules are added and stacks started. Nothing on the host is deleted.
- [ ] Work through the **BY HAND** steps: first-run wizards, DNS records, turning a firewall on (in System → FIREWALL, behind its rollback).
- [ ] **Preview again** shows what's left.

## Part 4 · Add starter runbooks

- [ ] [Knowledge](view:notes) → **Starters** lists the runbooks. Open one to read it, then **Add to my runbooks…** and confirm.
- [ ] It's now your note: it shows as **▤ Runbook** on matching alerts, and you can edit it to fit your setup. An alert with no runbook of yours offers **▤ Starter runbook** when one exists.

## Limits and gotchas

- Starters are a sound beginning, not a finished setup: change versions, ports and folders in Docker → STACKS afterwards like any stack.
- A stack name that already runs on the host shows as **THERE**; starting it again is skipped.
- Folders owned by root (such as `/opt/stacks` on a fresh host) need creating and handing to your user first, or pick one in your home folder.
- Runbooks for failed units and high memory have no alert kind yet, so they aren't offered on alerts; tie one to a host in its editor if you like.

## Related

- [Write runbooks for alerts](kb:howto/runbooks)
- [Deploy a new compose stack](kb:howto/deploy-stack)
- [First homelab weekend](kb:guides/starter-homelab)
