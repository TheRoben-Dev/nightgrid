# mergerfs and SnapRAID

Pool mismatched disks into one folder with mergerfs and protect them with SnapRAID parity. A good fit for a media server that grows one disk at a time.

![Data disks pooled by mergerfs and protected by SnapRAID parity.](fig:app-mergerfs)

## What you end up with

- One mount (`/mnt/storage`) spread across disks of any size
- One or two parity disks that can rebuild a failed data disk
- Nightly sync and weekly scrub on timers, visible in Nightgrid

## Before you start

- Data disks formatted ext4 or xfs and mounted (`/mnt/disk1`, `/mnt/disk2`, …) by UUID in `/etc/fstab`.
- A parity disk at least as large as the largest data disk, mounted at `/mnt/parity1`.
- Best for data that changes rarely. Files changed since the last sync aren't protected.

## Part 1 · mergerfs

- [ ] `sudo apt install -y mergerfs`.
- [ ] Add to `/etc/fstab`:

```
/mnt/disk* /mnt/storage fuse.mergerfs cache.files=off,category.create=mfs,dropcacheonclose=true,minfreespace=50G,fsname=mergerfs 0 0
```

- [ ] `sudo mkdir -p /mnt/storage && sudo mount /mnt/storage`.

## Part 2 · SnapRAID

- [ ] `sudo apt install -y snapraid`, then `/etc/snapraid.conf`:

```
parity /mnt/parity1/snapraid.parity
content /var/snapraid.content
content /mnt/disk1/snapraid.content
content /mnt/disk2/snapraid.content
data d1 /mnt/disk1/
data d2 /mnt/disk2/
exclude *.unrecoverable
exclude /tmp/
exclude /lost+found/
```

- [ ] First sync (takes hours): `sudo snapraid sync`.

## Part 3 · Schedule it

- [ ] In [System](view:system) → Schedules → **+ Timer**: name `snapraid-sync`, command `/usr/bin/snapraid sync`, schedule `*-*-* 03:00:00`.
- [ ] A second timer `snapraid-scrub`: `/usr/bin/snapraid scrub -p 8 -o 10`, schedule `Sun 05:00`.
- [ ] Add `snapraid-sync` in [Fleet](view:fleet) → **Backups** → **+ Add job** so a failed or stale sync alerts.

#### Check

- [ ] `sudo snapraid status` reports no errors and a recent sync.

## When Nightgrid is closed

- The timers run on the host.

## Limits and gotchas

- Deleting lots of files then syncing makes those files unrecoverable. Check `snapraid diff` before a sync after a big cleanup (helper scripts like snapraid-runner add a delete threshold).
- SnapRAID isn't realtime. Use ZFS or mdraid for data that changes constantly.
- Hardlinks across disks don't exist in mergerfs; *arr apps need `category.create=epmfs` or a single disk per category to hardlink. See [The *arr stack](kb:apps/arr-stack).

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Keep an eye on disks and pools](kb:howto/disks)
- [The *arr stack](kb:apps/arr-stack)
