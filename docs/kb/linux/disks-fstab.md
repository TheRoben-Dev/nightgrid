# Add a disk: partition, format and mount

Take a new drive from blank to mounted at boot, by UUID, with options that won't hang the machine if the disk is missing.

![From a device name to a mount point that survives reboots.](fig:lx-fstab)

## What you end up with

- A GPT partition with ext4 or xfs
- A mount in `/etc/fstab` by UUID with `nofail`
- The disk in Nightgrid's disk health and forecasts

## Before you start

- Identify the disk and make sure it's the empty one. Wiping the wrong disk is the classic mistake:

```sh
lsblk -o NAME,SIZE,MODEL,SERIAL,MOUNTPOINTS
```

## Part 1 · Partition and format

- [ ] Replace `sdX` with the disk from `lsblk` (DRIFT's command check flags `mkfs` and `wipefs`; read the target twice):

```sh
sudo wipefs -a /dev/sdX
sudo parted -s /dev/sdX mklabel gpt mkpart data 0% 100%
sudo mkfs.ext4 -L data /dev/sdX1
```

- [ ] xfs instead: `sudo mkfs.xfs -L data /dev/sdX1` (good for large files; can't shrink).

## Part 2 · Mount at boot

- [ ] Get the UUID: `sudo blkid /dev/sdX1`.
- [ ] Add to `/etc/fstab`:

```
UUID=<uuid>  /srv/data  ext4  defaults,noatime,nofail,x-systemd.device-timeout=10s  0  2
```

- [ ] Test without rebooting:

```sh
sudo mkdir -p /srv/data
sudo systemctl daemon-reload
sudo mount -a && findmnt /srv/data
```

- [ ] `sudo chown $USER: /srv/data` (or the uid your containers use).

## Part 3 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Disks**: the new mount appears with a fill-up forecast after about a day.
- [ ] [System](view:system) → **disks** shows its SMART health; run a long self-test on a new drive before trusting it: `sudo smartctl -t long /dev/sdX`.

#### Check

- [ ] `findmnt /srv/data` shows the mount after a reboot.

## Limits and gotchas

- `nofail` keeps the machine booting if the disk dies; services using it should depend on the mount (`RequiresMountsFor=/srv/data` in their unit).
- Never use `/dev/sdb` names in fstab; they change between boots.
- For several disks with redundancy, see [ZFS pools, datasets and snapshots](kb:apps/zfs) or [mergerfs and SnapRAID](kb:apps/mergerfs-snapraid).

## Related

- [Keep an eye on disks and pools](kb:howto/disks)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [LVM: grow and snapshot volumes](kb:linux/lvm)
