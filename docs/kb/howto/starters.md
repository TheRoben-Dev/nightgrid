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

## Part 5 · The other shelves

The tabs above the cards hold more starters. Each says what it changes and asks before changing anything.

- [ ] **APPS**: one app as a compose stack (Vaultwarden, Immich, Nextcloud, Uptime Kuma, Paperless-ngx, Homepage, Forgejo, Navidrome, Audiobookshelf, Mealie, FreshRSS, Actual Budget). Same preview and apply as above.
- [ ] **JOBS**: scheduled jobs installed as Nightgrid timers (Docker cleanup, unattended upgrades, SMART self-tests, a backup kick-off, TRIM, ZFS scrubs). Some ask for a word such as the disk or pool; they show up in System → SCHEDULES.
- [ ] **FIREWALL**: allow-rule profiles (SSH only, web server, LAN only, Docker host). Turning the firewall on stays in System → FIREWALL, behind its rollback.
- [ ] **ALERTS**: alert rule sets (quiet nights, strict storage, calm log alerts, everything on). A table shows each setting now and after.
- [ ] **TILES** and **DASHBOARDS**: command tiles for the Overview, and whole dashboards for a media box, a NAS or network ops that bring their own tiles. Pick the host the tiles read from; dashboards are added beside yours.
- [ ] **LOG SEARCHES**: saved searches across every host (SSH logins, out-of-memory kills, disk errors, container errors, failed services); some alert on new matches.
- [ ] **PROXY ROUTES**: names like `jellyfin.your-domain` for the app starters, saved to a Caddy or Traefik proxy Nightgrid found on a host. Type your domain, untick what you don't want, **Preview the change** shows the diff, then confirm.
- [ ] **PROXMOX**: VM and container shapes (a Docker LXC, a small service LXC, a Linux VM, Home Assistant OS, Windows 11). They open Proxmox → Create with the values filled in; you pick the ID, storage and network and press Create there.
- [ ] **DRILLS**: a restore drill for each backup job that has none, weekly or monthly.
- [ ] **POSTURE**: security scoring and network watch settings as a set.

## Limits and gotchas

- Starters are a sound beginning, not a finished setup: change versions, ports and folders in Docker → STACKS afterwards like any stack.
- A stack name that already runs on the host shows as **THERE**; starting it again is skipped.
- Folders owned by root (such as `/opt/stacks` on a fresh host) need creating and handing to your user first, or pick one in your home folder.
- Proxy routes reach apps by container name, so the app and the proxy must share a Docker network; otherwise change the upstream to `host:port` in Docker → PROXY.
- Tiles and dashboards are added each time you confirm; remove extras from the Overview's edit mode.
- Runbooks for failed units and high memory have no alert kind yet, so they aren't offered on alerts; tie one to a host in its editor if you like.

## Related

- [Write runbooks for alerts](kb:howto/runbooks)
- [Deploy a new compose stack](kb:howto/deploy-stack)
- [First homelab weekend](kb:guides/starter-homelab)
