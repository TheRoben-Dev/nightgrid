# Copy and mirror with rsync

The tool for moving lots of files between disks and servers: resumes where it stopped, copies only what changed, and keeps permissions and timestamps.

![rsync over SSH, sending only the changes.](fig:lx-rsync)

## What you end up with

- Safe one-off copies between disks and hosts
- A mirror job that only sends changes
- A dry run habit that catches a wrong path before it deletes anything

## Part 1 · The flags that matter

```sh
rsync -aHAX --info=progress2 /srv/photos/ /mnt/new-disk/photos/
```

- [ ] `-a` keeps permissions, owners, times and symlinks. `-H` hard links, `-A` ACLs, `-X` extended attributes.
- [ ] The **trailing slash** on the source means "the contents of". Without it you get `/mnt/new-disk/photos/photos`.

## Part 2 · Between hosts

```sh
rsync -aH --info=progress2 -e ssh /srv/photos/ user@nas:/tank/photos/
```

- [ ] Add `--partial` for big files over flaky links: a broken transfer continues instead of starting over.
- [ ] Owners are kept only when the receiving side runs as root: `--rsync-path="sudo rsync"` with a sudo rule for rsync.

## Part 3 · Mirror, carefully

- [ ] `--delete` removes files on the destination that aren't in the source. Always do a dry run first:

```sh
rsync -aH --delete --dry-run --itemize-changes /srv/photos/ user@nas:/tank/photos/ | grep deleting | head
```

- [ ] Then the same command without `--dry-run`.
- [ ] Exclude caches and temp files: `--exclude='.cache/' --exclude='*.tmp'`.

## Part 4 · Run it on a schedule

- [ ] A nightly mirror as a systemd timer or a cron line (in crontab, escape `%` as `\%`). Log what happened:

```sh
rsync -aH --delete /srv/photos/ user@nas:/tank/photos/ --log-file=/var/log/rsync-photos.log
```

#### Check

- [ ] A second run right after the first transfers nothing (`--stats` shows 0 files transferred).

## In Nightgrid

- [System](view:system) → **Schedules** creates the timer for you and shows its last run (see [Schedule jobs with cron and timers](kb:howto/schedules)).
- A mirror is not a backup: deleted or encrypted files are mirrored too. Pair it with [restic](kb:apps/restic) and watch it in [Fleet](view:fleet) → **Backups**.

## Limits and gotchas

- rsync compares size and modification time by default; `--checksum` reads everything and is slow, use it to verify, not every night.
- Copying between filesystems with different features (ext4 to exFAT) loses owners and permissions and may complain about each file; use `-rt` instead of `-a` there.
- A typo'd destination with `--delete` can wipe the wrong folder. Dry run first, every time.

## Related

- [Backups](kb:basics/backups)
- [restic backups with systemd timers](kb:apps/restic)
- [Offsite copies with rclone](kb:apps/rclone)
