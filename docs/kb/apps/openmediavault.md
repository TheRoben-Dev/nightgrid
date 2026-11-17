# openmediavault NAS on Debian

A NAS web UI on top of plain Debian: disks, RAID, SMB and NFS shares, users and scheduled jobs from a browser, while the box stays a normal Debian server you can SSH into and run Docker on.

![The layers of a NAS OS.](fig:app-nas-os)

## What you end up with

- A Debian NAS managed from a web page
- SMB shares for Windows and Macs, NFS for Linux
- SMART monitoring and scheduled jobs in the UI

## Before you start

- A fresh Debian install (the version openmediavault's current release targets) on a small SSD, with data disks separate
- The data disks empty, or already formatted with a filesystem you're keeping

## Part 1 · Install

- [ ] Install with the project's script, from a console or SSH session on the fresh Debian:

```sh
wget -O - https://github.com/OpenMediaVault-Plugin-Developers/installScript/raw/master/install | sudo bash
```

- [ ] It installs openmediavault and the omv-extras plugins. Reboot when it finishes.
- [ ] Log in at `http://nas-ip` as `admin` / `openmediavault` and change the password (user menu).

## Part 2 · Storage and shares

- [ ] Storage → Disks: wipe new disks. Storage → File Systems: create ext4 or btrfs on each, or a RAID (Storage → Software RAID) first.
- [ ] For mixed-size disks, the mergerfs and SnapRAID plugins (from omv-extras) pool them.
- [ ] Storage → Shared Folders, then Services → SMB/CIFS → Shares. Users → Users for logins.

## Part 3 · Health

- [ ] Storage → S.M.A.R.T.: enable, and schedule short and long tests per disk.
- [ ] System → Notification: email settings so disk warnings reach you.

#### Check

- [ ] Map the share from a Windows PC (`\\nas-ip\share`) and copy a large file at near line speed.

## In Nightgrid

- Add the NAS as a host: it's Debian underneath, so [System](view:system), [Fleet](view:fleet) → **Disks** (SMART, mdraid, btrfs) and **Updates** all work.
- Let openmediavault manage its own config files. Editing `/etc/samba/smb.conf` by hand gets overwritten; [System](view:system) → **config** drift will show you when that happens.
- The compose plugin in omv-extras and Nightgrid's [Docker](view:docker) can both manage stacks; pick one per stack.

## Limits and gotchas

- Don't install a desktop or extra services that touch the same config openmediavault controls.
- Major openmediavault versions follow Debian releases; upgrade both together as its docs describe, not with a plain Debian release upgrade.

## Related

- [Build a DIY NAS](kb:guides/diy-nas)
- [Samba file shares](kb:apps/samba)
- [mergerfs and SnapRAID](kb:apps/mergerfs-snapraid)
