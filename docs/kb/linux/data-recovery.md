# Recover data from a failing drive

When a drive clicks, throws read errors or a partition vanished, copy it first and repair the copy. Every extra hour a dying disk runs costs readable sectors.

![Image first, then recover from the copy.](fig:lx-data-recovery)

## What you end up with

- A full image of the failing drive, read in the safest order
- Lost partitions found and files carved out of the image
- The original drive left alone after one careful pass

## Before you start

- `sudo apt install gddrescue testdisk` (the package is `ddrescue` on Fedora)
- A destination with more free space than the whole failing drive
- Don't mount, fsck or "repair" the failing drive itself

## Part 1 · Image the drive

- [ ] First pass, skip slow areas and grab the easy data fast:

```sh
sudo ddrescue -n /dev/sdX /mnt/rescue/disk.img /mnt/rescue/disk.map
```

- [ ] Second pass, go back for the bad areas with retries:

```sh
sudo ddrescue -d -r3 /dev/sdX /mnt/rescue/disk.img /mnt/rescue/disk.map
```

- [ ] The map file lets you stop and resume, even after a reboot. Keep it next to the image.
- [ ] A drive that heats up or drops off the bus: let it cool, reconnect, rerun the same command.

## Part 2 · Work on the copy

- [ ] Make a second copy of the image before experimenting, or use a writable overlay.
- [ ] Attach it as loop devices with partitions:

```sh
sudo losetup -Pf --show /mnt/rescue/disk.img
lsblk /dev/loop0
```

- [ ] Mount partitions read-only: `sudo mount -o ro /dev/loop0p2 /mnt/look`.

## Part 3 · Lost partitions

- [ ] `sudo testdisk /mnt/rescue/disk.img` → Analyse → Quick Search; Deeper Search if nothing shows. Write the found table to the image (not the original).

## Part 4 · Deleted or unreadable files

- [ ] `photorec /mnt/rescue/disk.img` carves files by signature (photos, documents, archives) into a folder you choose. Names and folders are lost; contents usually aren't.

#### Check

- [ ] `ddrescuelog -t /mnt/rescue/disk.map` shows how much was rescued, and the files you need open from the copy.

## In Nightgrid

- Long `ddrescue` runs belong in tmux on the rescue machine; check on them from a [Terminal](view:terminal) tab anywhere.
- Browse the mounted image and copy files out in [Files](view:files).

## Limits and gotchas

- Clicking or not spinning up is mechanical. Software can't fix it; stop and decide whether it's worth a lab.
- SSDs that died usually lock up completely; ddrescue helps only when they still answer.
- A filesystem `fsck` on the original can destroy what's left. Repair copies only.

## Related

- [Test a drive before you trust it](kb:linux/disk-testing)
- [Backups](kb:basics/backups)
- [Add a disk: partition, format and mount](kb:linux/disks-fstab)
