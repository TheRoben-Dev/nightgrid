# Patch every host from one place

See pending and security updates on all hosts, upgrade a batch with one confirmation, and know which machines still need a reboot.

![Check, pick, upgrade, reboot.](fig:ng-fleet-updates)

## What you end up with

- One list of hosts with pending, security and reboot-needed counts, worst first
- Batch upgrades, three hosts at a time, with output under each row
- Holds for anything you don't want touched: a host for a while, a package, an image or a whole stack
- Automatic security updates on the hosts for the gaps in between

## Part 1 · Check

- [ ] [Fleet](view:fleet) → **Updates** → **Check all**. apt, dnf, pacman and rpm-ostree are supported.
- [ ] Click a host for its package list.
- [ ] The Overview's UPDATES widget shows the same, refreshed every 3 hours while shown.

## Part 2 · Upgrade

- [ ] Tick hosts and **Upgrade selected…**. Confirm by typing the host's name, the group's name (when you picked exactly one whole group) or `upgrade N hosts`.
- [ ] Hosts whose sudo needs a password that isn't remembered are skipped with a note. Upgrade those from [System](view:system) → **updates** and tick remember.
- [ ] Nothing reboots on its own. **Reboot…** opens the host's usual confirmation.

#### Check

- [ ] **Check all** again: zero pending on the hosts you upgraded, reboot flags where a kernel changed.

## Part 3 · Hold things back

Nothing is held until you say so. Held things aren't counted as pending, the Overview stays quiet about them, and upgrades leave them alone. Everything held is listed under **HOLDS** at the bottom of the page, each with **Release**.

- [ ] **A whole host**: **Hold…** on its row, for 1 day, 1 week, 30 days or until you release it, with an optional note (why, until when). It can't be ticked for **Upgrade selected…** while held. Checking still works, and nothing changes on the host.
- [ ] **One package**: open the host's list and choose **Hold** on the row (or in [System](view:system) → **updates**, which asks for the sudo password when needed). On apt, dnf with versionlock and Windows the hold is set on the host itself (`apt-mark hold`, `dnf versionlock add`, hiding the update in Windows Update), so unattended upgrades and every other tool respect it too. On pacman, and dnf without the versionlock plugin, it's held in Nightgrid: Nightgrid's upgrades leave it out (`--ignore`, `-x`), but other tools on the host don't know about it.
- [ ] **A container image or a stack**: [Docker](view:docker) → stack → **Updates** → **Hold** on an image, or **Hold stack**. Also **Hold** next to an image in **Check images**. Pull and Update then skip its services; checks still say when a newer version exists, marked HELD.
- [ ] Packages held on the host outside Nightgrid (an earlier `apt-mark hold`, `IgnorePkg` in pacman.conf) show as HELD too, and **Release** works on them.

## Part 4 · Order matters

- [ ] Upgrade one test or low-stakes host first, then the rest.
- [ ] Hosts others depend on (DNS, the NAS, the hypervisor) go last, one at a time, with a snapshot or backup first.
- [ ] Proxmox nodes: upgrade from the node's terminal with `apt full-upgrade` (Proxmox needs full-upgrade, not plain upgrade).

## Part 5 · Automatic security updates

- [ ] Let the hosts install security fixes daily on their own: [Automatic security updates](kb:apps/unattended-upgrades).
- [ ] [System](view:system) → **security** counts it as a finding when they're off.

## Limits and gotchas

- rpm-ostree (Fedora Atomic, Bazzite) stages the new image; it applies on the next reboot.
- Container images aren't OS packages. Check them with **Check images** on the same page, or see [Update containers without fear](kb:howto/container-updates).
- A held package stays on its version on purpose, security fixes included. Hold for a reason, write it in the note, and release it when the reason is gone.
- Image-based systems (rpm-ostree) update as one image, so single packages can't be held: hold the host instead.
- An image hold is Nightgrid's: `docker compose pull` run by hand, or Watchtower, still pulls it. Pin the tag in the compose file (or by digest, `image@sha256:…`) to hold it everywhere.
- Holding or releasing on the host needs root (apt, dnf) or an administrator (Windows). From Fleet it only works where sudo needs no password; otherwise do it from System → **updates**.

## Related

- [Automatic security updates](kb:apps/unattended-upgrades)
- [Update containers without fear](kb:howto/container-updates)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
