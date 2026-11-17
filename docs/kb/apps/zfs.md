# ZFS pools, datasets and snapshots

Build a ZFS pool on Debian or Ubuntu, organise it in datasets, take automatic snapshots and send them to a second machine.

## What you end up with

- A mirrored or RAIDZ pool with sensible defaults
- Datasets per use (media, backups, VMs) with their own settings
- Hourly and daily snapshots with sanoid, replicated with syncoid
- Pool health and scrubs visible in Nightgrid

## Before you start

- Disks identified by stable ids: `ls -l /dev/disk/by-id`.
- At least 8 GB RAM for a small pool (ZFS uses free RAM as cache and gives it back).
- Back up anything on those disks; creating a pool wipes them.

## Part 1 · Install and create

- [ ] Debian (enable `contrib` in apt sources first) or Ubuntu:

```sh
sudo apt install -y zfsutils-linux
```

- [ ] A mirror of two disks:

```sh
sudo zpool create -o ashift=12 -O compression=lz4 -O atime=off -O xattr=sa -O acltype=posixacl tank mirror /dev/disk/by-id/<disk-a> /dev/disk/by-id/<disk-b>
```

- [ ] For 4+ disks, `raidz2` survives two failures. Mirrors are faster and easier to grow.

## Part 2 · Datasets

- [ ] One dataset per job:

```sh
sudo zfs create -o recordsize=1M tank/media
sudo zfs create tank/backups
sudo zfs create -o recordsize=16K tank/db
```

- [ ] Large files like media benefit from `recordsize=1M`; databases from small records.
- [ ] Hand them to the right user: `sudo chown -R 1000:1000 /tank/media`.

## Part 3 · Snapshots with sanoid

- [ ] `sudo apt install -y sanoid`, then `/etc/sanoid/sanoid.conf`:

```ini
[tank/backups]
  use_template = production
[tank/db]
  use_template = production

[template_production]
  hourly = 24
  daily = 30
  monthly = 6
  autosnap = yes
  autoprune = yes
```

- [ ] The package ships a systemd timer: `systemctl list-timers | grep sanoid`.
- [ ] Replicate to another ZFS host over SSH (run on the source, as a [timer](kb:howto/schedules)):

```sh
syncoid --recursive tank/backups backup@backup-host:pool/backups
```

## Part 4 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Disks** shows the pool, degraded devices, errors and scrub progress, and the fill-up forecast.
- [ ] Turn on **Storage pool degraded** in Settings → Alerts.
- [ ] Add the syncoid timer as a job in Fleet → **Backups** (systemd timer type).

#### Check

- [ ] `zpool status` shows ONLINE and a recent scrub; `zfs list -t snapshot` shows sanoid snapshots.

## When Nightgrid is closed

- Scrubs (monthly, from the zfsutils cron job), sanoid snapshots and syncoid replication all run on the host.

## Limits and gotchas

- ZFS is an out-of-tree kernel module. On Debian, kernel updates rebuild it with DKMS; check `zfs version` after a kernel upgrade before rebooting a remote box.
- Don't put ZFS on hardware RAID or USB hubs that hide disks.
- Snapshots aren't backups until they're on another machine.

## Related

- [Keep an eye on disks and pools](kb:howto/disks)
- [Proxmox media server](kb:guides/proxmox-media-server)
- [NFS shares](kb:apps/nfs)
- [Samba file shares](kb:apps/samba)
