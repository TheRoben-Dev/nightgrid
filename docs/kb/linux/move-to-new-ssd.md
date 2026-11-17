# Move a server to a new SSD

Swap a server's system disk for a bigger or healthier one without reinstalling: clone it block for block when the new disk is at least as big, or copy the files and reinstall the bootloader when you also want a new layout. Either way, the server comes back with the same hostname, keys and services.

![Moving to a new SSD, step by step.](fig:lx-move-ssd)

## What you end up with

- The system running from the new SSD, with all data and settings intact
- Partitions grown to use the extra space
- The old disk kept untouched until you're sure

## Before you start

- A full backup, and a note of what's on the old disk (`lsblk -f`, `cat /etc/fstab`).
- A way to connect both disks at once (second M.2 slot, USB enclosure) and a live USB or [netboot.xyz network boot menu](kb:apps/netbootxyz) for cloning offline.
- Check the old disk's health first ([Fleet](view:fleet) → **Disks**). A failing disk should be copied with ddrescue instead ([Recover data from a failing drive](kb:linux/data-recovery)).

## Part 1 · Clone (same layout, new disk as big or bigger)

- [ ] Boot the live system so the old disk isn't in use. Identify both disks carefully with `lsblk -o NAME,SIZE,MODEL,SERIAL`.
- [ ] Clone: `sudo dd if=/dev/OLD of=/dev/NEW bs=4M status=progress conv=fsync`. Double-check `of=`: it overwrites everything on that disk. Clonezilla does the same with a menu and copies only used blocks.
- [ ] GPT disks: `sudo sgdisk -e /dev/NEW` moves the backup partition table to the end of the bigger disk.
- [ ] Grow the last partition and its filesystem: `sudo growpart /dev/NEW 3` (the partition number), then `sudo resize2fs /dev/NEW3` for ext4, or `xfs_growfs` / `btrfs filesystem resize max` on the mounted filesystem. With LVM, `pvresize` then `lvextend -r` (see [LVM: grow and snapshot volumes](kb:linux/lvm)).
- [ ] Remove the old disk (or unplug it) before the first boot, so two disks with identical UUIDs don't confuse the boot.

## Part 2 · Copy (new layout or smaller disk)

- [ ] Partition and format the new disk as you want it, including an EFI partition (FAT32, 512 MB) on UEFI systems.
- [ ] Mount the new root at `/mnt/new` (and its EFI partition at `/mnt/new/boot/efi`), then copy from the live system: `sudo rsync -aAXH --info=progress2 /mnt/old/ /mnt/new/`.
- [ ] Update `/mnt/new/etc/fstab` with the new UUIDs from `blkid`.
- [ ] Reinstall the bootloader: bind-mount `/dev`, `/proc`, `/sys` and `/sys/firmware/efi/efivars` into `/mnt/new`, `chroot /mnt/new`, then `grub-install` and `update-grub` (Debian/Ubuntu) or `grub2-mkconfig -o /boot/grub2/grub.cfg` (Fedora/RHEL).

#### Check

- [ ] The server boots from the new disk alone, `lsblk` and `df -h` show the new size, and every container and service comes back up.

## In Nightgrid

- The host's SSH host keys come along with either method, so Nightgrid connects without a fingerprint warning. A warning after the move means you booted something else.
- [System](view:system) → **config** → **Compare** against a baseline saved before the move should show only `/etc/fstab` changed (for the copy method).
- [Fleet](view:fleet) → **Disks** shows the new SSD's SMART data.

## Limits and gotchas

- ZFS root pools and mdadm mirrors don't need cloning: add the new disk to the mirror, let it resilver, then remove the old one (see [ZFS pools, datasets and snapshots](kb:apps/zfs) and [Software RAID with mdadm](kb:linux/mdadm-raid)).
- Proxmox on ZFS also needs `proxmox-boot-tool format` and `init` on the new disk's EFI partition.
- Never boot with both cloned disks attached; the system may mount a mix of the two.

## Related

- [Add a disk: partition, format and mount](kb:linux/disks-fstab)
- [Fix a server that won't boot](kb:linux/boot-rescue)
- [Test a drive before you trust it](kb:linux/disk-testing)
