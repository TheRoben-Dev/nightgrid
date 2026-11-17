# Patch every host from one place

See pending and security updates on all hosts, upgrade a batch with one confirmation, and know which machines still need a reboot.

## What you end up with

- One list of hosts with pending, security and reboot-needed counts, worst first
- Batch upgrades, three hosts at a time, with output under each row
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

## Part 3 · Order matters

- [ ] Upgrade one test or low-stakes host first, then the rest.
- [ ] Hosts others depend on (DNS, the NAS, the hypervisor) go last, one at a time, with a snapshot or backup first.
- [ ] Proxmox nodes: upgrade from the node's terminal with `apt full-upgrade` (Proxmox needs full-upgrade, not plain upgrade).

## Part 4 · Automatic security updates

- [ ] Let the hosts install security fixes daily on their own: [Automatic security updates](kb:apps/unattended-upgrades).
- [ ] [System](view:system) → **security** counts it as a finding when they're off.

## Limits and gotchas

- rpm-ostree (Fedora Atomic, Bazzite) stages the new image; it applies on the next reboot.
- Container images aren't OS packages. Check them with **Check images** on the same page, or see [Update containers without fear](kb:howto/container-updates).
- A held package (`apt-mark hold`) stays pending forever by design.

## Related

- [Automatic security updates](kb:apps/unattended-upgrades)
- [Update containers without fear](kb:howto/container-updates)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
