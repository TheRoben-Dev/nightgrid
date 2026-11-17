# Fix a full disk

"No space left on device" breaks databases, Docker, logins and updates in odd ways. Find what filled the disk, free space safely in the right order, and stop it happening again.

![Finding what filled the disk.](fig:lx-disk-full)

## What you end up with

- The real cause found in minutes (big files, deleted-but-open files, inodes or snapshots)
- Space freed without deleting anything you need
- An alert before it happens next time

## Part 1 · What is full?

```bash
df -h -x tmpfs -x devtmpfs
df -i -x tmpfs -x devtmpfs
```

- [ ] `df -h` at 100 %: out of space. `df -i` at 100 % with space left: out of **inodes** (millions of tiny files, usually a cache or mail queue).
- [ ] Note which mount is full: `/`, `/var`, a data disk. Everything below works on one filesystem at a time.

## Part 2 · Find the big stuff

```bash
sudo apt install ncdu        # or dnf install ncdu
sudo ncdu -x /
```

- [ ] `-x` stays on one filesystem. Arrow keys to dive in, `d` deletes (careful). Common culprits:
  - `/var/lib/docker`: images, build cache, container logs (see [Free disk space Docker is using](kb:linux/docker-cleanup))
  - `/var/log` and the journal (`journalctl --disk-usage`)
  - `/var/cache/apt` or `/var/cache/dnf`
  - a forgotten backup, ISO or core dump in a home folder or `/tmp`
- [ ] No ncdu? `sudo du -xh / --max-depth=2 2>/dev/null | sort -h | tail -20`.

## Part 3 · Free space in a safe order

```bash
sudo journalctl --vacuum-size=200M
sudo apt clean                       # or: sudo dnf clean all
docker system df
docker image prune -a
```

- [ ] Delete old logs and caches before anything else; they come back on their own if needed.
- [ ] Never delete files inside `/var/lib/docker`, `/var/lib/mysql` or `/var/lib/postgresql` by hand.

## Part 4 · The sneaky cases

- [ ] **Deleted but still open**: space doesn't come back after deleting a big log because a process still holds it.

```bash
sudo lsof +L1 | sort -k7 -n | tail
```

  Restart that service, or truncate the file through `/proc/<pid>/fd/<n>` with `: > /proc/<pid>/fd/<n>`.
- [ ] **Snapshots**: on ZFS (`zfs list -t snapshot -o name,used -s used`) and btrfs (`snapper list`), deleted data stays until old snapshots are destroyed.
- [ ] **Reserved blocks**: ext4 keeps 5 % for root. On a big data-only disk, `sudo tune2fs -m 1 /dev/sdX1` returns most of it (leave it on `/`).
- [ ] **Inodes**: find the folder with the most files with `sudo du --inodes -x / 2>/dev/null | sort -n | tail`.

#### Check

- [ ] `df -h` shows comfortable free space, and the service that failed starts again (`systemctl --failed` is empty).

## In Nightgrid

- [Fleet](view:fleet) → **Disks** shows every mount's growth per day and **full in about N days**, and alerts before it fills. Watch it after the cleanup to see whether usage climbs back.
- [System](view:system) → **timeline** shows what failed when the disk filled (failed units, Docker crashes).
- Make it routine: a weekly [System](view:system) → **schedules** → **+ Timer** with `journalctl --vacuum-size=500M` and `docker image prune -af --filter until=168h`.

## Limits and gotchas

- A full `/` can stop SSH logins for non-root users and break `sudo` (it writes to `/var`). Log in as root on the console if you must.
- Databases that ran out of space may need their own recovery or a restore; check their logs before assuming they're fine.
- Growing the disk (LVM, a bigger VM disk) is fine, but find the cause first or it fills again.

## Related

- [Free disk space Docker is using](kb:linux/docker-cleanup)
- [Journald: keep logs, but not forever](kb:linux/journald)
- [Keep an eye on disks and pools](kb:howto/disks)
