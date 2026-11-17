# Day-to-day Proxmox from Nightgrid

Run a Proxmox VE node or cluster from Nightgrid: guests, snapshots, backups, hardware, network, firewall and storage, all over SSH with no API token.

![Nightgrid talks to one node; pvesh sees the whole cluster.](fig:ng-proxmox)

## What you end up with

- The whole cluster read with `pvesh` over SSH as root
- Snapshots before risky changes and restores that ask the right questions
- Network and firewall changes that roll back by themselves if they cut you off

## Before you start

- Proxmox VE 8 with root SSH key login from this computer (or a user with sudo).
- Settings → **FEATURES**: turn on **Proxmox** (off in the Standard preset).

## Part 1 · Guests

- [ ] [Proxmox](view:proxmox) (`Mod+Shift+P`), pick the node's host.
- [ ] **SUMMARY**: CPU, memory, root disk, uptime and quorum. **GUESTS**: filter by name, id, state or `#tag`.
- [ ] **Start**, **Shut down**, **Resume** run at once; **⋯** has Reboot, Pause and Force stop, which ask first.
- [ ] **+ VM** and **+ Container** create guests on the connected node. Containers need an SSH public key (no password is set).

## Part 2 · Snapshots and backups

- [ ] Click a guest → **SNAPSHOTS** → take one before an upgrade. **with RAM** saves a running VM's memory.
- [ ] **BACKUPS** tab: every guest's newest backup, worst first; guests no job covers are flagged.
- [ ] **Back up now** runs vzdump. A backup can be restored as a new guest or over the original.

## Part 3 · Hardware, network, firewall

- [ ] **HARDWARE**: cores, memory, CPU type, disks (**Grow…** only grows), NICs, PCI/USB passthrough and boot options.
- [ ] **NETWORK**: bridges, bonds and VLANs; changes are staged as a diff and **Apply…** needs **Keep** within 90 seconds.
- [ ] **FIREWALL**: datacenter, node, groups and per guest. Add a rule for how you reach the node before turning the datacenter firewall on.

## Part 4 · Storage and tasks

- [ ] **STORAGE**: usage, ISO images and CT templates. **Download ISO…** (paste a SHA-256 to verify) and **Get CT template…**.
- [ ] **TASKS**: recent tasks with logs.

#### Check

- [ ] Take and delete a snapshot on a test guest; the task log shows OK.

## Limits and gotchas

- **Screen** and **Console** work for guests on the node you're connected to; add other nodes as hosts.
- Nightgrid lists backup jobs but doesn't create them; use `pvesh create /cluster/backup …` (see [Game server host](kb:guides/game-server-host)).
- Snapshots on LVM-thin slow down as chains grow. Delete them once a change has proven itself.

## Related

- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
- [Open VM screens and RDP](kb:howto/vm-desktops)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
