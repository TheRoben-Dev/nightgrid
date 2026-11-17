# Software RAID with mdadm

Mirror two disks (RAID 1) or stripe with parity (RAID 5/6) using Linux's built-in md RAID, get mail and Nightgrid alerts when a disk drops, and replace a failed disk.

![Partitions become one md device, which carries the filesystem.](fig:lx-mdadm)

## What you end up with

- `/dev/md0` surviving a disk failure
- Monthly consistency checks
- A tested replacement procedure

## Part 1 · Create a mirror

```sh
sudo apt install -y mdadm
sudo mdadm --create /dev/md0 --level=1 --raid-devices=2 /dev/sdX /dev/sdY
cat /proc/mdstat
```

- [ ] Save the array so it assembles at boot:

```sh
sudo mdadm --detail --scan | sudo tee -a /etc/mdadm/mdadm.conf
sudo update-initramfs -u
```

- [ ] Format and mount `/dev/md0` as in [Add a disk: partition, format and mount](kb:linux/disks-fstab).

## Part 2 · Alerts

- [ ] `MAILADDR root` in `/etc/mdadm/mdadm.conf` plus a working mail relay ([Send mail from your servers](kb:apps/mail-relay)).
- [ ] [Fleet](view:fleet) → **Disks** shows the array, degraded members and resync progress; turn on **Storage pool degraded** in Settings → Alerts.

## Part 3 · Replace a failed disk

```sh
sudo mdadm --detail /dev/md0
sudo mdadm /dev/md0 --fail /dev/sdX --remove /dev/sdX
```

- [ ] Swap the disk (check the serial with `lsblk -o NAME,SERIAL` so you pull the right one), then:

```sh
sudo mdadm /dev/md0 --add /dev/sdZ
watch cat /proc/mdstat
```

#### Check

- [ ] `/proc/mdstat` shows `[UU]` after the resync.

## Limits and gotchas

- RAID isn't backup: deletions and ransomware are mirrored instantly.
- RAID 5 with large disks has long rebuilds during which a second failure loses everything; prefer RAID 6, RAID 10 or ZFS RAIDZ2.
- Debian's `mdadm` package runs a monthly `checkarray`; leave it on.

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Keep an eye on disks and pools](kb:howto/disks)
