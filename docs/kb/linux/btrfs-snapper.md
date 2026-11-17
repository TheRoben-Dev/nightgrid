# Btrfs snapshots with Snapper

Use btrfs subvolumes and Snapper to take automatic snapshots of root and data, roll back a bad update, and recover deleted files. Fedora, openSUSE and many desktops already use btrfs.

## What you end up with

- Hourly and daily snapshots with automatic cleanup
- Pre/post snapshots around package updates (with the dnf or apt plugin)
- Monthly scrubs, visible in Nightgrid

## Part 1 · Set up Snapper

```sh
sudo apt install -y snapper        # or: sudo dnf install snapper
sudo snapper -c root create-config /
sudo snapper -c data create-config /srv/data
```

- [ ] Limits in `/etc/snapper/configs/root`: `TIMELINE_LIMIT_HOURLY="10"`, `TIMELINE_LIMIT_DAILY="7"`, `TIMELINE_LIMIT_WEEKLY="2"`, `TIMELINE_LIMIT_MONTHLY="0"`.
- [ ] Enable the timers: `sudo systemctl enable --now snapper-timeline.timer snapper-cleanup.timer`.

## Part 2 · Recover

```sh
sudo snapper -c data list
sudo snapper -c data diff 12..13
sudo snapper -c data undochange 12..13 /srv/data/important.txt
```

- [ ] Or copy straight out of `/srv/data/.snapshots/<n>/snapshot/`.

## Part 3 · Scrub

- [ ] A monthly [timer](kb:howto/schedules): `btrfs scrub start -B /` (Fedora ships `btrfs-scrub@.timer` in btrfsmaintenance).
- [ ] [Fleet](view:fleet) → **Disks** lists btrfs filesystems with errors and scrub progress.

#### Check

- [ ] `snapper list` shows new timeline snapshots every hour.

## Limits and gotchas

- Snapshots share the disk; heavy churn (VM images, databases) eats space. Make those `chattr +C` folders or separate subvolumes excluded from snapshots.
- Btrfs RAID 5/6 is still not recommended; RAID 1 is fine.

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Keep an eye on disks and pools](kb:howto/disks)
