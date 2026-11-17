# Errors: ZFS, RAID and LVM

Messages from zpool/zfs, mdadm software RAID, LVM and SnapRAID/mergerfs. See also [mdadm RAID](kb:linux/mdadm-raid), [LVM](kb:linux/lvm) and Nightgrid's Fleet → **Disks**.

### pool '<pool>' is DEGRADED
Means: The pool lost redundancy: a disk faulted or went missing, but data is still available.
Fix: `zpool status -x` names the device. Replace it with `zpool replace <pool> <old> <new>` and let it resilver. Don't wait: one more failure may lose data.

### One or more devices could not be used because the label is missing or invalid.
Means: ZFS can't find a disk it expects (moved, renamed, dead, or a different /dev/sdX name).
Fix: Check cables and `lsblk`. Import with stable names: `zpool export <pool>; zpool import -d /dev/disk/by-id <pool>`.

### One or more devices has experienced an unrecoverable error. An attempt was made to correct the error. Applications are unaffected.
Means: ZFS found and repaired bad data using redundancy.
Fix: Check SMART on the device with errors; `zpool clear <pool>` after investigating. Recurring errors mean replace the disk or cable.

### One or more devices has experienced an error resulting in data corruption. Applications may be affected.
Means: ZFS found corrupt data it couldn't repair (no redundancy for that block).
Fix: `zpool status -v` lists affected files; restore them from backup, then `zpool clear` and scrub.

### cannot import '<pool>': pool was previously in use from another system.
Means: The pool wasn't exported before moving it (or the hostid changed after a reinstall).
Fix: If no other system uses it now, `zpool import -f <pool>`.

### cannot import '<pool>': no such pool available
Means: ZFS can't find the pool's disks.
Fix: `zpool import` (no args) lists importable pools; try `-d /dev/disk/by-id`. Check the disks are attached and the zfs module is loaded.

### cannot import '<pool>': I/O error Destroy and re-create the pool from a backup source.
Means: The pool's metadata can't be read from the available disks.
Fix: Try `zpool import -F <pool>` (rewinds to an earlier transaction group; may lose the last seconds of writes). Get expert help before anything destructive.

### The ZFS modules are not loaded. Try running '/sbin/modprobe zfs' as root to load them.
Means: The kernel module isn't loaded, often after a kernel update where the DKMS build failed.
Fix: `dkms status`; reinstall zfs-dkms for the new kernel, or boot the previous kernel.

### cannot create '<dataset>': dataset already exists
Means: A dataset with that name exists.
Fix: Use another name or `zfs list` to find it.

### cannot destroy '<dataset>': filesystem has children
Means: The dataset has child datasets or snapshots.
Fix: `zfs list -r -t all <dataset>` first; `-r` destroys recursively (check carefully).

### cannot destroy '<dataset>': dataset is busy
Means: The dataset is mounted and in use.
Fix: Find users with `fuser -vm <mountpoint>`; stop them, then retry.

### cannot mount '<dataset>': directory is not empty
Means: The mountpoint directory already has files (often written while the pool wasn't imported).
Fix: Move those files away, then `zfs mount <dataset>`. Find out what wrote there.

### cannot receive incremental stream: destination <dataset> has been modified since most recent snapshot
Means: The target dataset changed after the last received snapshot, so the incremental can't apply.
Fix: Roll back the target to the last common snapshot (`zfs rollback`), or receive with `-F` (discards the target's changes).

### cannot send '<snapshot>': incremental source (<snapshot>) does not exist
Means: The base snapshot for an incremental send was deleted on the source.
Fix: Do a full send, or pick a common snapshot that still exists on both sides.

### out of space
Means: ZFS is full (also counting reservations and snapshots); performance drops badly above about 80%.
Fix: `zfs list -o space` shows what uses it, snapshots included. Delete old snapshots before data.

### cannot set property for '<dataset>': size is greater than available space
Means: A quota, reservation or volsize can't fit.
Fix: Free space or lower the value.

### zfs-import-cache.service: Failed with result 'exit-code'.
Means: Boot-time import from the cache file failed (the cache is stale or a disk is missing).
Fix: Import by hand, then `zpool set cachefile=/etc/zfs/zpool.cache <pool>` to refresh it.

### scan: resilver in progress
Means: ZFS is rebuilding redundancy onto a new or returning disk.
Fix: Let it finish; avoid heavy load. `zpool status` shows progress.

### errors: Permanent errors have been detected in the following files:
Means: ZFS lists files with unrecoverable corruption.
Fix: Restore those files from backup and delete the damaged ones, then scrub.

### mdadm: <dev> is busy - skipping
Means: The disk is already part of an active array or in use.
Fix: `cat /proc/mdstat`; stop the array first if you intend to change it.

### md/raid1:<md>: Disk failure on <dev>, disabling device.
Means: The kernel kicked a member out of the RAID after errors.
Fix: `mdadm --detail /dev/<md>`. Replace the disk: `mdadm --manage /dev/<md> --remove <dev>`, add the new one with `--add`.

### mdadm: DegradedArray event detected on md device <md>
Means: mdadm's monitor reports the array is degraded.
Fix: As above: find the failed member and replace it.

### md: <md>: recovery done.
Means: Informational: the rebuild finished.
Fix: Nothing; `cat /proc/mdstat` should show [UU].

### mdadm: No arrays found in config file or automatically
Means: mdadm found no arrays to assemble (missing config, or superblocks not recognized).
Fix: `mdadm --examine --scan` to find them; add the output to /etc/mdadm/mdadm.conf and update the initramfs.

### mdadm: cannot open <dev>: Device or resource busy
Means: The device is held by something else (another md, LVM, or a mounted filesystem).
Fix: `lsblk` shows what sits on it; stop that first.

### md<n>: unknown partition table
Means: Harmless: the md device has a filesystem directly, not a partition table.
Fix: Nothing.

### Volume group "<vg>" not found
Means: LVM can't find that VG (disk missing, wrong name, or not scanned yet).
Fix: `vgscan`, `pvs`, and check disks are present.

### Cannot process volume group <vg>
Means: Same family: the VG's disks or metadata aren't available.
Fix: `pvs -a` and `vgchange -ay`; check for missing PVs.

### WARNING: Couldn't find device with uuid <uuid>.
Means: A physical volume of the VG is missing.
Fix: Reconnect the disk. If it's dead, `vgreduce --removemissing <vg>` (data on it is lost).

### Insufficient free space: <n> extents needed, but only <n> available
Means: The VG doesn't have enough free space for that size.
Fix: `vgs` shows free space; add a PV (`vgextend`) or use `-l 100%FREE`.

### Logical volume <lv> contains a filesystem in use.
Means: You tried to shrink or remove a mounted LV.
Fix: Unmount first. Remember to shrink the filesystem before the LV, or use `lvresize -r`.

### Thin pool <pool> transaction_id is <n>, while expected <n>.
Means: Thin pool metadata is inconsistent (often after a crash).
Fix: Back up metadata, then `lvconvert --repair <vg>/<pool>`. Common on Proxmox local-lvm after power loss.

### WARNING: Thin pool <vg>/<pool> metadata is now <n>% full.
Means: The thin pool's metadata space is running out; at 100% the pool goes read-only.
Fix: `lvextend --poolmetadatasize +1G <vg>/<pool>`.

### WARNING: Sum of all thin volume sizes (<n>) exceeds the size of thin pool <vg>/<pool>
Means: The pool is overcommitted: fine until the data really fills it.
Fix: Watch `lvs` Data% closely; extend the pool before it reaches 100%.

### device-mapper: reload ioctl on <name> failed: Device or resource busy
Means: The mapping is in use and can't be changed.
Fix: Close what uses it (mounts, LUKS, VMs) and retry.

### snapraid: Error opening the parity file '<file>'
Means: A parity disk isn't mounted or the path in snapraid.conf is wrong.
Fix: Mount the parity disk; check the `parity` line.

### snapraid: DANGER! In the array there are <n> errors!
Means: sync or scrub found errors (data or parity).
Fix: `snapraid status` and `snapraid -e fix` to repair from parity; check SMART on the disk involved.

### snapraid: The disk '<disk>' is missing.
Means: A data disk isn't mounted.
Fix: Mount it before syncing, or SnapRAID refuses (good: it protects parity).

### mergerfs: <path>: No space left on device
Means: mergerfs's create policy chose a branch that's full, or every branch is.
Fix: Use a policy like `category.create=mfs` and set `minfreespace`.
