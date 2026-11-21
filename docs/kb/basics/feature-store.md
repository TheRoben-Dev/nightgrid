# The feature store: a slim core and components you add

Nightgrid ships as a slim core with the tools nearly every homelab needs. Everything more specialised is an optional component you install from **Settings → STORE**. You only see what you use, and a component you haven't installed costs nothing: its view stays hidden, its code never loads and its background checks never run.

## What's in the core

The core is always there and can't be removed:

- **Hosts and SSH**, the **Overview** dashboard and **alerts**
- **Terminal**, **Files** and **Docker**
- **Services** (health checks) and **System** (one host's units, journal, firewall, updates and power)
- **Fleet**: updates, disks, backups and uptime across all hosts
- **Tunnels** and **Network tools**
- **Knowledge** (your notes) and this **Library**

## What's in the store

Components sit on shelves:

| Shelf | Components |
| --- | --- |
| Virtualization | Proxmox, VM desktops (VNC and RDP) |
| Monitoring | UPS, log search, expiry watch, power & cost, capacity planner, incident replay, Nightgrid Node |
| Operations | automation, journal, blueprints, service catalog, restore drills |
| Network | network map, speed tests |
| Security | security score and secrets, access audit |
| Extras | DRIFT helper, UI sounds |

## Installing and removing

- **Install** switches a component on right away, with no restart. Its views appear in the sidebar, the command palette, shortcuts, Overview widgets and DRIFT.
- **Remove** hides it again and stops its background work. Nothing it stored is deleted (history, settings, notes, saved searches), so installing it again later brings everything back.
- Opening something that isn't installed, from a shortcut, a link or DRIFT, lands on the store with **Install and open**.
- What's installed travels with your profile export and with device sync.

**Settings → FEATURES** is the finer switch: inside the components you have installed, it turns single views and checks on or off (for example Fleet's UPS tab, or only the expiry alerts).

## Updating from an older version

Nothing disappears when you update. Nightgrid installs every component you had at least one feature switched on for, and anything you had turned off in Features stays out. A fresh install starts with the core only.

## Component patches

Some components get fixes on their own, between app releases: Proxmox, VM desktops, Security, the network map and Automation so far. When one of them changes, a patch for just that component is published to Nightgrid's GitHub releases.

- Nightgrid looks for patches for the components you have installed after start-up and every 6 hours. **Check for patches** in the store looks now, and installing a component fetches its newest patch.
- Every patch is signed with the same key as app updates. It's checked before it's stored and again every time it's used, and nothing from anywhere else is ever accepted.
- A patch is only used when it's newer than the copy built into your app and everything it needs from the app is unchanged. Otherwise the built-in copy keeps running, so a patch can never leave a view broken after an app update.
- A patch applies the next time you open that view after a restart. The component's card says when it's running a patch and what the patch fixed, or why a patch isn't used.
- If a patch ever fails to load or draw, Nightgrid switches to the built-in copy at once and never tries that patch again.
- Patches follow **Settings → Updates → Interface fixes between releases**. Turn that off and Nightgrid stops looking.

A patch can only change a component's screens. What the app is allowed to do (which commands exist, what it may reach on your computer) is built into the app itself, so a patch can't widen it.

## Good to know

- Components aren't plugins: they are all written and signed by Nightgrid's developer, and there's no way to add code from anyone else.
- Removing a component doesn't uninstall anything on your servers. Nightgrid Node, for example, keeps running on a host until you remove it there.
- If a view you expect is missing, check the store first, then **Settings → FEATURES**.
