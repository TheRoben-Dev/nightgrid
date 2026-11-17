# Offsite copies with rclone

Copy backups or important folders to cloud storage (S3, Backblaze B2, Google Drive, OneDrive, another SFTP server) with rclone, encrypted on the way, on a timer.

![An rclone crypt remote in front of cloud storage.](fig:app-rclone-crypt)

## What you end up with

- An encrypted offsite copy that the provider can't read
- A nightly sync that keeps deleted files for 30 days
- The job on Nightgrid's backup board with stale alerts

## Part 1 · Install and configure

- [ ] `sudo apt install -y rclone`, or the newer binary from rclone's site.
- [ ] Create the provider remote, then a `crypt` remote on top of it:

```sh
sudo rclone config
```

- [ ] In the wizard: `n` → name `b2` → your provider → credentials. Then `n` → name `offsite` → type `crypt` → remote `b2:my-bucket/backups` → generate passwords. Save both crypt passwords in your password manager.
- [ ] The config lands in `/root/.config/rclone/rclone.conf`. Keep it mode 600.

## Part 2 · Sync on a timer

- [ ] Test it: `sudo rclone sync /srv/backups offsite:current --backup-dir offsite:deleted/$(date +%F) --dry-run`.
- [ ] In [System](view:system) → Schedules → **+ Timer**, name `rclone-offsite`, schedule `*-*-* 05:00:00`, command:

```sh
/usr/bin/rclone sync /srv/backups offsite:current --backup-dir offsite:deleted/$(date +%F) --transfers 4 --log-level INFO
```

- [ ] Delete old kept copies monthly with a second timer: `rclone delete offsite:deleted --min-age 30d`.

## Part 3 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Backups** → **+ Add job**: the `rclone-offsite` timer. Its last result and output show on the board.

#### Check

- [ ] `sudo rclone ls offsite:current | head` lists files, and the provider's web UI shows only scrambled names.

## Limits and gotchas

- `sync` deletes on the destination what's gone from the source. `--backup-dir` is what keeps a deleted file recoverable.
- Copying a restic or borg repository offsite works; rclone doesn't need to understand it.
- Consumer clouds (Drive, OneDrive) throttle large uploads; S3-style storage is more predictable.

## Related

- [restic backups with systemd timers](kb:apps/restic)
- [Backups](kb:basics/backups)
- [Monitor backups and test restores](kb:howto/backups)
