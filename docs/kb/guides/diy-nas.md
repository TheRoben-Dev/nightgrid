# Build a DIY NAS

Turn a Debian box with a few drives into a NAS: a ZFS mirror, SMB shares for every computer, NFS for other servers, snapshots, offsite backups and disk alerts, all run from Nightgrid.

## What you end up with

- A ZFS pool with datasets for shares, media and backups
- Samba shares (with Time Machine for Macs) and NFS exports
- Automatic snapshots, replication or offsite copies, and health alerts
- A UPS that shuts it down cleanly

## Before you start

- A machine with room for 2+ data drives (ECC RAM is nice, not required), Debian 12 or 13 installed on a separate SSD.
- Drives of the same size for a mirror.
- The host added to Nightgrid with key login.

## Part 1 · Pool and datasets

- [ ] [ZFS pools, datasets and snapshots](kb:apps/zfs): create the mirror and datasets `tank/share`, `tank/media` (`recordsize=1M`), `tank/backups`, `tank/timemachine`.
- [ ] sanoid snapshots on `tank/share` and `tank/backups`.

#### Check

- [ ] [Fleet](view:fleet) → **Disks** shows `tank` ONLINE.

## Part 2 · Shares

- [ ] [Samba file shares](kb:apps/samba) for `tank/share` and a Time Machine share on `tank/timemachine` (`fruit:time machine = yes`, plus `fruit:time machine max size = 1T`).
- [ ] Expose snapshots to Windows "Previous Versions": add `vfs objects = shadow_copy2 fruit streams_xattr` with `shadow:snapdir = .zfs/snapshot` and the sanoid name format in the share (see Samba's shadow_copy2 docs).
- [ ] [NFS shares](kb:apps/nfs) of `tank/media` for your media server.

## Part 3 · Health and alerts

- [ ] `smartd` and ZED email ([Send mail from your servers](kb:apps/mail-relay)), and Nightgrid's **SMART warning**, **Storage pool degraded** and **Disk full soon** alerts ([Keep an eye on disks and pools](kb:howto/disks)).
- [ ] [Scrutiny SMART dashboard](kb:apps/scrutiny) if you want long attribute history.

## Part 4 · Backups of the NAS

- [ ] The NAS is not its own backup. Replicate `tank/backups` and `tank/share` with syncoid to a second ZFS box, or copy them offsite with [Offsite copies with rclone](kb:apps/rclone) or restic.
- [ ] Add the jobs in [Fleet](view:fleet) → **Backups** and test-restore monthly.

## Part 5 · Power

- [ ] [NUT server for your UPS](kb:apps/nut) on the NAS, other servers as NUT secondaries.
- [ ] In Nightgrid, [UPS monitoring with NUT](kb:howto/ups) and a dependency "media server depends on the NAS" under Settings → Alerts → Dependencies.

#### Check

- [ ] A Mac backs up with Time Machine, a Windows PC sees Previous Versions, and the media server reads over NFS.

## When Nightgrid is closed

- Keeps running: shares, snapshots, scrubs, smartd and ZED mail, replication timers, NUT shutdown.
- Needs Nightgrid: dashboards, pool and SMART alerts in the app, test restores.

## Limits and gotchas

- ZFS mirrors grow by replacing both disks with bigger ones, or adding another mirror pair. Plan bays accordingly.
- Avoid SMR drives in ZFS; resilvers can take days.

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Samba file shares](kb:apps/samba)
- [First homelab weekend](kb:guides/starter-homelab)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
