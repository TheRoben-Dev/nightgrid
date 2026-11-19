# An Unraid server alongside Nightgrid

Unraid runs its array, Docker containers and VMs from its own web UI. Nightgrid adds a terminal, file browsing, container logs and restarts, service checks and alerts next to it, without fighting Unraid's way of doing things.

![The layers of a NAS, as Unraid builds them.](fig:app-nas-os)

## What you end up with

- Key-based SSH to the Unraid server that survives reboots
- Unraid's Docker containers visible in Nightgrid
- Compose stacks from Nightgrid, if you add the Compose plugin

## Before you start

- Unraid runs from a USB flash drive and loads into RAM. Changes outside `/boot/config`, the array and the cache pool are lost at reboot. That shapes everything below.

## Part 1 · SSH with a key

- [ ] **Settings → Management Access**: SSH enabled (on by default for `root`).
- [ ] **Users → root → SSH authorized keys**: paste your public key (copy it with **Copy public key** in **Settings → Connections → SSH keys**). Unraid stores it on the flash drive, so it survives reboots. Keys added only to `/root/.ssh` vanish at the next boot.
- [ ] Add the host in Nightgrid with user `root` and connect once to accept the fingerprint.

## Part 2 · Docker and stacks

- [ ] The [Docker](view:docker) view shows containers created from Unraid's templates; logs, restart, stop and **⧉** work.
- [ ] Edit those containers in Unraid's UI, not in Nightgrid: Unraid recreates them from its templates and would undo outside changes.
- [ ] For compose stacks, install the **Docker Compose Manager** plugin from Community Applications. Keep stack folders on the cache pool or array (for example `/mnt/user/appdata/stacks`), not in `/root`, which is in RAM.

## Part 3 · Files and checks

- [ ] [Files](view:files) browses `/mnt/user/<share>` (all disks merged) or `/mnt/diskN` (one disk). Prefer `/mnt/user`.
- [ ] Add the Unraid web UI and your apps to [Services](view:services).

#### Check

- [ ] Reboot the Unraid server: Nightgrid reconnects with the key, and the containers and stacks come back.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** reads SMART for the array disks; parity checks and array state stay in Unraid's UI.
- [Services](view:services) → **Add integration** → **Unraid** (7.2 or newer, or the Unraid Connect plugin) with an API key from Settings → Management Access: array state and capacity, each disk's status, temperature and errors, and containers and VMs running (see [Add app cards](kb:howto/integrations)).
- [Fleet](view:fleet) → **Updates** doesn't apply to Unraid; update the OS and plugins from Unraid's own **Tools → Update OS** and **Plugins** pages.
- Unraid has no systemd journal, so [Fleet](view:fleet) → **Logs** finds container logs there but not the system log; read `/var/log/syslog` in the [Terminal](view:terminal).

## Limits and gotchas

- Packages installed by hand disappear on reboot; use plugins or containers instead.
- Unraid's `root` is the only real admin user; keep SSH on the LAN or tailnet.
- Unraid doesn't use systemd, so [System](view:system) services and timers don't apply, and crontab edits live in RAM and vanish at reboot. Use the **User Scripts** plugin for schedules there.

## Related

- [Run a TrueNAS box alongside Nightgrid](kb:apps/truenas)
- [A Synology NAS alongside Nightgrid](kb:apps/synology)
- [mergerfs and SnapRAID](kb:apps/mergerfs-snapraid)
