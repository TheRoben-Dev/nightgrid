# Errors: disks, filesystems and mounts

Messages from mount, fsck, the kernel's block layer, ext4, XFS and Btrfs, and partition tools. See also [Add a disk](kb:linux/disks-fstab) and [When the disk is full](kb:linux/disk-full).

### No space left on device
Means: ENOSPC: the filesystem has no free blocks, or no free inodes (many small files), even if `df -h` shows space. On Btrfs, metadata can run out while data has room.
Fix: `df -h` and `df -i` on the path. Usual culprits: Docker (`docker system df`), journald (`journalctl --vacuum-size=500M`), /var/log, old kernels, deleted-but-open files (`lsof +L1`).

### mount: <dir>: wrong fs type, bad option, bad superblock on <dev>, missing codepage or helper program, or other error.
Means: mount couldn't mount the device. Common causes: the filesystem driver or helper isn't installed (nfs-common, cifs-utils, ntfs-3g, exfatprogs), a typo in options, or a damaged filesystem.
Fix: `dmesg | tail` shows the real reason. `blkid <dev>` shows the actual filesystem type. Install the helper for NFS/SMB mounts.

### mount: <dir>: special device <dev> does not exist.
Means: The device path in the command or fstab isn't there: a renamed disk (sdb became sdc), an unplugged disk, or a typo.
Fix: Use `UUID=` in fstab instead of /dev/sdX. `lsblk -f` lists devices and UUIDs.

### mount: <dir>: mount point does not exist.
Means: The target directory hasn't been created.
Fix: `sudo mkdir -p <dir>`, then mount again.

### mount: <dir>: <dev> already mounted on <dir>.
Means: That device is already mounted there (or elsewhere).
Fix: `findmnt <dev>` shows where. Unmount first if you want it somewhere else.

### mount: <dir>: can't read superblock on <dev>.
Means: The kernel couldn't read the filesystem's superblock: a failing disk, the wrong partition, or a damaged filesystem.
Fix: `dmesg -T | tail -30`, `smartctl -a`. For ext4 try a backup superblock with `e2fsck -b 32768 <dev>` (unmounted, after imaging if data matters).

### mount: <dir>: unknown filesystem type '<type>'.
Means: The kernel has no driver for that filesystem, or the type name is wrong.
Fix: Install the tools (`exfatprogs`, `ntfs-3g`, `zfsutils-linux`), or load the module (`modprobe <fs>`). Check the type with `blkid`.

### mount: only root can do that
Means: Normal users can mount only entries marked `user` or `users` in /etc/fstab.
Fix: Use sudo, add `user` to the fstab options, or use `udisksctl mount -b <dev>` on desktops.

### mount: <dir>: permission denied.
Means: The mount was refused, often inside an unprivileged container or because the mount point is inside a filesystem you can't write to.
Fix: Run as root on the host. In LXC containers, mounts need to be done from the host (bind mounts) or the container needs the right features.

### umount: <dir>: target is busy.
Means: A process still has a file open or its working directory inside the mount.
Fix: `sudo fuser -vm <dir>` or `sudo lsof +f -- <dir>` shows who. Stop it, or `umount -l` to detach lazily.

### umount: <dir>: not mounted.
Means: Nothing is mounted there (it may already be unmounted, or the path differs by a trailing component).
Fix: `findmnt` lists what's actually mounted.

### EXT4-fs error (device <dev>): <function>: <details>
Means: ext4 found on-disk inconsistencies. Depending on the `errors=` option it continues, remounts read-only, or panics.
Fix: Check the disk's health (`smartctl -a`), back up, then unmount and run `e2fsck -f <dev>` (from a rescue boot for the root filesystem).

### EXT4-fs (<dev>): Remounting filesystem read-only
Means: ext4 hit an error and switched to read-only to protect the data (errors=remount-ro).
Fix: Find the earlier error in `dmesg -T`. Check SMART; run `e2fsck -f` while unmounted; replace the disk if it reports reallocated or pending sectors.

### Read-only file system
Means: EROFS: the filesystem is mounted read-only, by choice (an immutable OS like Bazzite or Fedora Silverblue, a read-only bind mount) or because the kernel remounted it after errors.
Fix: `findmnt -T <path>` shows the mount options. If errors caused it, see `dmesg` and check the disk. On immutable systems, /usr is read-only by design: use rpm-ostree, Flatpak or containers.

### e2fsck: Cannot continue, aborting.
Means: e2fsck refused to run, usually because the filesystem is mounted.
Fix: Unmount it first; for the root filesystem boot a rescue system or use `fsck.mode=force` on the kernel command line.

### <dev> is mounted.
Means: You ran a repair tool on a mounted filesystem, which can corrupt it.
Fix: Unmount it (or boot rescue media) before running fsck or xfs_repair.

### The superblock could not be read or does not describe a valid ext2/ext3/ext4 filesystem.
Means: e2fsck didn't find an ext filesystem there: wrong device (whole disk vs partition), another filesystem type, or a damaged primary superblock.
Fix: `blkid` and `lsblk -f` to confirm the type. For ext4, list backup superblocks with `mke2fs -n <dev>` (only prints) and try `e2fsck -b <block> <dev>`.

### Bad magic number in super-block
Means: The data at that location isn't an ext superblock.
Fix: Same as "superblock could not be read": confirm the device and filesystem type first.

### UNEXPECTED INCONSISTENCY; RUN fsck MANUALLY.
Means: The boot-time check found problems it won't fix automatically, so boot stops in an emergency shell.
Fix: In that shell run `fsck -y <dev>` on the reported device, then `reboot`. Check SMART afterwards.

### Superblock last mount time is in the future.
Means: The system clock was wrong when the filesystem was last mounted (dead RTC battery, or a VM restored from a snapshot).
Fix: Usually safe: let fsck fix it. Set the clock right (`timedatectl`), replace the CMOS battery if it keeps happening.

### XFS (<dev>): Corruption detected. Unmount and run xfs_repair
Means: XFS found inconsistent metadata and shut the filesystem down.
Fix: Unmount, then `xfs_repair <dev>`. If it complains about the log, mount once to replay it; use `xfs_repair -L` only as a last resort (it discards the log).

### xfs_repair: ERROR: The filesystem has valuable metadata changes in a log which needs to be replayed.
Means: The XFS journal has unapplied changes; repairing now would lose them.
Fix: Mount the filesystem once and unmount it to replay the log, then run xfs_repair. Use `-L` only if mounting fails.

### XFS (<dev>): Filesystem has been shut down due to log error
Means: XFS couldn't write its journal (usually an I/O error underneath) and stopped all access.
Fix: Check the disk or RAID below it (`dmesg`, `smartctl`). Unmount, repair after the hardware is fine.

### BTRFS error (device <dev>): bdev <dev> errs: wr <n>, rd <n>, flush <n>, corrupt <n>, gen <n>
Means: Btrfs keeps per-device error counters; non-zero read/write errors point at a failing disk or cable, corrupt counts at checksum failures.
Fix: `btrfs device stats <mount>`; check SMART; run `btrfs scrub start <mount>`. Reset the counters with `btrfs device stats -z` after fixing the cause.

### BTRFS: error (device <dev>) in btrfs_<function>: errno=-28 No space left
Means: Btrfs ran out of space for metadata even though `df` may show free space, because space is allocated in chunks.
Fix: `btrfs filesystem usage <mount>`. Free space by deleting snapshots, then `btrfs balance start -dusage=10 <mount>` to compact data chunks.

### BTRFS warning (device <dev>): checksum error at logical <n>
Means: Data read from disk doesn't match its checksum: silent corruption, a bad disk, or bad RAM.
Fix: With a redundant profile (RAID1), a scrub repairs it. Otherwise restore the named file from backup. Test RAM if errors appear on several disks.

### parent transid verify failed on <n> wanted <n> found <n>
Means: Btrfs metadata on disk is older than expected, typically after a power loss on a disk that lies about write caching, or a failing disk.
Fix: Try `mount -o ro,rescue=usebackuproot`. Copy data off before attempting `btrfs check --repair`, which is a last resort.

### blk_update_request: I/O error, dev <dev>, sector <n>
Means: The disk failed to read or write that sector. Often a failing disk, but also a loose cable or failing controller.
Fix: `smartctl -a /dev/<dev>`: look at Reallocated_Sector_Ct, Current_Pending_Sector and UDMA_CRC_Error_Count (CRC errors mean cable). Back up now.

### I/O error, dev <dev>, sector <n> op 0x<n>:(READ)
Means: Newer kernels' wording of the same disk read error.
Fix: Same as blk_update_request I/O error: SMART, cables, backup.

### ata<n>: hard resetting link
Means: The SATA link dropped and the kernel is resetting it: a bad cable, loose power, a failing disk, or an overloaded PSU.
Fix: Reseat or replace the SATA and power cables first. If it continues with the same disk on another port, the disk is the problem.

### ata<n>.00: failed command: READ FPDMA QUEUED
Means: A queued read command to the disk failed. Repeated, it's a failing disk or bad link.
Fix: Check SMART and cables as for "hard resetting link".

### Buffer I/O error on dev <dev>, logical block <n>
Means: A filesystem-level read or write to that block failed because the device returned an error.
Fix: Treat as a failing device: check `dmesg` for the underlying error, SMART, and back up.

### Medium Error [current] Sense Key : Medium Error
Means: The disk itself reports it couldn't read the data at that location (an unreadable sector).
Fix: The disk is failing. Replace it; with RAID or ZFS, let the redundancy rebuild.

### nvme nvme<n>: I/O <n> QID <n> timeout, aborting
Means: The NVMe drive didn't answer in time. Common with aggressive power saving (APST) on some drives, overheating, or a failing drive.
Fix: `nvme smart-log /dev/nvme<n>` for temperature and errors. Try `nvme_core.default_ps_max_latency_us=0` on the kernel command line to disable deep power states.

### nvme nvme<n>: controller is down; will reset
Means: The NVMe controller stopped responding and the kernel reset it.
Fix: Same as NVMe timeouts: firmware update, disable APST, check cooling and the M.2 slot.

### smartctl: Read Device Identity failed: scsi error unsupported field in scsi command
Means: smartctl can't talk to the disk through that USB bridge or RAID controller with the default device type.
Fix: Try `-d sat` for USB-SATA bridges, `-d megaraid,<n>` for LSI RAID, or put the HBA in IT mode.

### SMART overall-health self-assessment test result: FAILED!
Means: The disk's own firmware predicts it will fail soon.
Fix: Back up immediately and replace the disk. Nightgrid's Fleet → DISKS shows this per host.

### Device: /dev/<dev>, <n> Currently unreadable (pending) sectors
Means: smartd reports sectors the disk couldn't read and is waiting to reallocate.
Fix: A rising count means a failing disk: back up and replace. A single one that clears after a full write may be harmless.

### Device: /dev/<dev>, <n> Offline uncorrectable sectors
Means: The disk found sectors it couldn't recover during offline testing.
Fix: Same as pending sectors: plan a replacement and keep backups current.

### partprobe: Error: Partition(s) <n> on <dev> have been written, but we have been unable to inform the kernel of the change
Means: A partition on that disk is in use (mounted, swap, LVM, RAID), so the kernel kept the old table.
Fix: Unmount or deactivate what uses it, then `partprobe <dev>`, or reboot.

### Error: Partition(s) on <dev> are being used.
Means: parted won't change partitions that are mounted or used by LVM/RAID/swap.
Fix: `lsblk <dev>` shows what's on it; unmount and deactivate first.

### GPT PMBR size mismatch (<n> != <n>) will be corrected by write.
Means: The disk is larger than the GPT says, typically after growing a VM disk or cloning to a bigger disk.
Fix: Safe to let fdisk/parted fix it on write, or run `sgdisk -e <dev>` to move the backup GPT to the end.

### The backup GPT table is corrupt, but the primary appears OK, so that will be used.
Means: The copy of the partition table at the end of the disk is damaged or missing (cloned to a different-sized disk).
Fix: `sgdisk -e <dev>` rewrites the backup header at the end of the disk.

### Warning: The kernel is still using the old partition table.
Means: Partition changes were written but the kernel couldn't reload the table because the disk is in use.
Fix: `partprobe` after freeing the disk, or reboot.

### mkfs.<fs>: <dev> contains a <type> file system
Means: mkfs found an existing filesystem and asks before destroying it.
Fix: Double-check with `lsblk -f` that it's the right disk. Only then confirm or use `-F`/`-f`.

### wipefs: error: <dev>: probing initialization failed: Device or resource busy
Means: The device is in use (mounted, swap, part of RAID or LVM).
Fix: Unmount and deactivate it first (`swapoff`, `vgchange -an`, `mdadm --stop`).

### resize2fs: Bad magic number in super-block while trying to open <dev>
Means: resize2fs was pointed at something that isn't ext4, often an XFS filesystem.
Fix: Check with `blkid`. Use `xfs_growfs <mountpoint>` for XFS, `btrfs filesystem resize max <mount>` for Btrfs.

### resize2fs: On-line shrinking not supported
Means: ext4 can grow while mounted but can only shrink while unmounted.
Fix: Unmount (or boot rescue media), `e2fsck -f`, then `resize2fs <dev> <size>`.

### xfs_growfs: <path> is not a mounted XFS filesystem
Means: xfs_growfs needs the mount point of a mounted XFS filesystem.
Fix: Mount it and pass the mount point, not the device. XFS can't shrink at all.

### NOCHANGE: partition <n> is size <n>. it cannot be grown
Means: growpart found no free space after the partition (the disk wasn't actually enlarged, or another partition follows).
Fix: Check `lsblk` and the hypervisor's disk size; rescan with `echo 1 > /sys/class/block/<dev>/device/rescan`.

### swapon: <file>: swapon failed: Invalid argument
Means: The swap file has holes (created with fallocate on some filesystems) or sits on Btrfs without the right setup.
Fix: Create it with `dd if=/dev/zero` on ext4/XFS. On Btrfs use `btrfs filesystem mkswapfile`. See [Swap and zram](kb:linux/swap-zram).

### swapon: <file>: insecure permissions 0644, 0600 suggested.
Means: The swap file is readable by other users, who could read swapped-out memory.
Fix: `sudo chmod 600 <file>`.

### swapon: <file>: read swap header failed
Means: The file or partition hasn't been formatted as swap.
Fix: `sudo mkswap <file>` first.

### fstrim: <mount>: the discard operation is not supported
Means: The device (or a layer in between: LUKS, a USB bridge, a RAID controller) doesn't pass TRIM through.
Fix: For LUKS add `discard` to crypttab; for LVM set `issue_discards`; some USB bridges never support it.

### losetup: <file>: failed to set up loop device: No such device or address
Means: No free loop devices, or the loop module isn't loaded.
Fix: `sudo modprobe loop`; `losetup -f` finds a free one.

### cryptsetup: No key available with this passphrase.
Means: The passphrase didn't match any key slot on that LUKS device.
Fix: Check keyboard layout (initramfs often uses US layout). `cryptsetup luksDump <dev>` shows used slots. See [LUKS](kb:linux/luks).

### Device <name> already exists.
Means: A LUKS mapping with that name is already open.
Fix: Use it as /dev/mapper/<name>, or close it with `cryptsetup close <name>` first.

### Requested offset is beyond real size of device <dev>.
Means: The LUKS header expects more space than the device has, usually because you're opening the wrong (smaller) device or partition.
Fix: Check `lsblk` sizes; open the correct partition.

### mount error: could not find filesystem '<dev>'
Means: An initramfs couldn't find the root filesystem at boot: the UUID in the kernel command line or fstab no longer exists, or a driver is missing from the initramfs.
Fix: From the emergency shell run `blkid` to see real UUIDs. Fix fstab/GRUB, then rebuild the initramfs (`update-initramfs -u` or `dracut -f`).

### A start job is running for /dev/disk/by-uuid/<uuid>
Means: systemd waits (90 s by default) for a disk listed in fstab that never appears.
Fix: Correct or remove the fstab line. For removable or optional disks add `nofail,x-systemd.device-timeout=10s`.

### Dependency failed for /<mount>.
Means: systemd couldn't mount that fstab entry (missing device, bad option or a failed fsck), so units that need it didn't start.
Fix: `journalctl -b -u <mount>.mount` (path with `-` instead of `/`, for example `mnt-data.mount`). Fix the fstab line, `systemctl daemon-reload`, `mount -a`.

### mount: (hint) your fstab has been modified, but systemd still uses the old version; use 'systemctl daemon-reload' to reload.
Means: You edited /etc/fstab; systemd generates mount units from it and hasn't regenerated them yet.
Fix: `sudo systemctl daemon-reload`, then `sudo mount -a` to test.

### findmnt: <path>: parse error at line <n> -- ignored
Means: /etc/fstab has a malformed line (wrong field count or a stray character).
Fix: `sudo findmnt --verify` lists every problem; fix the line before rebooting.

### df: <mount>: Transport endpoint is not connected
Means: A FUSE mount (sshfs, rclone, mergerfs) died and left a dead mount point.
Fix: `fusermount -uz <mount>` then mount it again.

### du: cannot access '<path>': Permission denied
Means: du skipped directories your user can't read, so the total is incomplete.
Fix: Run with sudo, or add `2>/dev/null` if you only want your own files.

### rm: cannot remove '<path>': Device or resource busy
Means: The path is a mount point or a file in use by the kernel.
Fix: `findmnt <path>`: unmount it first. Never `rm -rf` across mount points you meant to keep (use `--one-file-system`).

### rm: cannot remove '<path>': Operation not permitted
Means: The file has the immutable or append-only attribute, or a container/sandbox blocks it.
Fix: `lsattr <path>`; remove the flag with `sudo chattr -i <path>` if it's intended to be deleted.

### rm: cannot remove '<path>': Read-only file system
Means: The filesystem is mounted read-only.
Fix: See the "Read-only file system" entry: check why it's read-only before remounting rw.

### mv: cannot move '<a>' to '<b>': Directory not empty
Means: The destination already has a non-empty directory of that name.
Fix: Merge with `rsync -a <a>/ <b>/` then delete the source, or rename one side.

### cp: cannot stat '<path>': No such file or directory
Means: The source path doesn't exist (often a glob that matched nothing, so the literal `*` was passed).
Fix: `ls` the source; use `shopt -s nullglob` in scripts.

### cp: error writing '<path>': No space left on device
Means: The destination filesystem filled up during the copy.
Fix: `df -h` on the destination; the partial file is left behind, so remove it.

### cp: preserving permissions for '<path>': Operation not supported
Means: The destination (FAT, exFAT, NTFS, some SMB mounts) can't store Linux permissions or ACLs.
Fix: Harmless for plain data. Use a Linux filesystem if permissions matter, or copy without `-a`/`-p`.

### chown: changing ownership of '<path>': Operation not permitted
Means: Only root can give files away; on NFS with root_squash even root can't; FAT/exFAT have no owners at all.
Fix: Use sudo. On NFS, chown on the server or adjust the export. For FAT/exFAT set `uid=`/`gid=` mount options.

### chmod: changing permissions of '<path>': Read-only file system
Means: The filesystem is read-only.
Fix: Check the mount options (`findmnt -T <path>`); fix permissions in the image or source instead.

### ln: failed to create hard link '<b>' => '<a>': Invalid cross-device link
Means: Hard links can't cross filesystems (and Docker volumes or bind mounts often are separate ones).
Fix: Put both paths on the same filesystem (for *arr apps: one shared /data mount), or use a symlink or copy.

### fallocate failed: Operation not supported
Means: The filesystem doesn't support fallocate (for example some network filesystems or old ext3).
Fix: Use `dd if=/dev/zero of=<file> bs=1M count=<n>`.

### No medium found
Means: ENOMEDIUM: a card reader or optical drive has no card/disc inserted.
Fix: Insert the media, or ignore if it's an empty reader slot showing up in `lsblk`.

### Structure needs cleaning
Means: EUCLEAN: the filesystem (often XFS or ext4) detected corrupt metadata in what you accessed.
Fix: Unmount and run the filesystem's repair tool (`xfs_repair` or `e2fsck -f`) after checking the disk.

### Disk quota exceeded
Means: EDQUOT: the user, group or dataset has reached its quota, although the disk may still have space.
Fix: `quota -s` for user quotas, `zfs get quota,refquota,used <dataset>` on ZFS, `btrfs qgroup show` on Btrfs.
