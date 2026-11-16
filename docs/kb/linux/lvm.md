# LVM: grow and snapshot volumes

Use LVM to resize volumes without repartitioning, add disks to a volume group, and take snapshots before risky changes. Ubuntu Server installs with LVM by default.

## What you end up with

- The root volume grown into free space (a common Ubuntu fix)
- New disks added to a volume group
- Snapshots you can roll back to

## Part 1 · Look

```sh
sudo pvs
sudo vgs
sudo lvs
```

- [ ] `VFree` in `vgs` is space you can give to volumes right now.

## Part 2 · Grow root (Ubuntu's half-used disk)

- [ ] Ubuntu's installer often leaves half the disk unallocated. Give it all to root and grow the filesystem in one step:

```sh
sudo lvextend -r -l +100%FREE /dev/ubuntu-vg/ubuntu-lv
df -h /
```

- [ ] `-r` resizes the filesystem too (ext4 and xfs both grow online).

## Part 3 · Add a disk

```sh
sudo pvcreate /dev/sdX
sudo vgextend ubuntu-vg /dev/sdX
sudo lvcreate -n data -L 500G ubuntu-vg
sudo mkfs.ext4 /dev/ubuntu-vg/data
```

- [ ] Mount it as in [Add a disk: partition, format and mount](kb:linux/disks-fstab).

## Part 4 · Snapshot before a risky change

```sh
sudo lvcreate -s -n root-pre-upgrade -L 10G /dev/ubuntu-vg/ubuntu-lv
```

- [ ] Roll back (applies at next activation, usually a reboot): `sudo lvconvert --merge /dev/ubuntu-vg/root-pre-upgrade`.
- [ ] Keep it: `sudo lvremove /dev/ubuntu-vg/root-pre-upgrade` once happy. A full snapshot becomes invalid.

## Part 5 · In Nightgrid

- [ ] Disk alerts and forecasts in [Fleet](view:fleet) → **Disks** tell you when to grow a volume.
- [ ] Save the commands as a runbook note in [Knowledge](view:notes) tied to the **Disk almost full** alert kind.

#### Check

- [ ] `df -h` shows the new size.

## Limits and gotchas

- Volumes spanning several disks fail if any disk fails. LVM isn't redundancy; use mdraid or ZFS under it for that.
- Shrinking ext4 requires unmounting; xfs can't shrink at all.

## Related

- [Add a disk: partition, format and mount](kb:linux/disks-fstab)
- [Write runbooks for alerts](kb:howto/runbooks)
