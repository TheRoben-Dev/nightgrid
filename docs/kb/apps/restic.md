# restic backups with systemd timers

Back up a server's important folders with restic to a local disk, another server or S3-compatible storage, on a nightly timer, with pruning and checks, and watch it from Nightgrid.

![A nightly restic run to a local and an offsite repository.](fig:app-backup-repo)

## What you end up with

- Encrypted, deduplicated snapshots of `/etc`, `/opt/stacks` and your data
- A systemd service and timer that Nightgrid's backup board reads directly
- Monthly test restores with one click

## Before you start

- A repository location: a folder on another disk, `sftp:backup@backup-host:/srv/restic/<host>`, or an S3 bucket (MinIO, Backblaze B2, Wasabi).
- Databases dumped to a file before the backup runs (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)).

## Part 1 · Install and initialise

- [ ] `sudo apt install -y restic` (or the static binary from its releases for the newest version, then `sudo restic self-update`). {check: command:restic}
- [ ] Store the password where only root can read it:

```sh
sudo install -d -m 700 /etc/restic
sudo sh -c 'openssl rand -base64 32 > /etc/restic/password' && sudo chmod 600 /etc/restic/password
```

- [ ] `/etc/restic/env` (mode 600):

```sh
RESTIC_REPOSITORY=sftp:backup@backup-host:/srv/restic/myhost
RESTIC_PASSWORD_FILE=/etc/restic/password
```

- [ ] Initialise: `sudo bash -c 'set -a; . /etc/restic/env; restic init'`.
- [ ] Copy `/etc/restic/password` into your password manager now. Without it the backups are unreadable.

## Part 2 · Service and timer

- [ ] `/etc/systemd/system/restic-backup.service`:

```ini
[Unit]
Description=restic backup
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
EnvironmentFile=/etc/restic/env
ExecStart=/usr/bin/restic backup /etc /opt/stacks /srv/data --exclude-caches --one-file-system
ExecStartPost=/usr/bin/restic forget --keep-daily 7 --keep-weekly 4 --keep-monthly 6 --prune
```

- [ ] `/etc/systemd/system/restic-backup.timer`:

```ini
[Unit]
Description=nightly restic backup

[Timer]
OnCalendar=*-*-* 02:30:00
RandomizedDelaySec=15m
Persistent=true

[Install]
WantedBy=timers.target
```

- [ ] `sudo systemctl daemon-reload && sudo systemctl enable --now restic-backup.timer`, then try it: `sudo systemctl start restic-backup`. {check: backup-timer}
- [ ] Add a weekly `restic check --read-data-subset=5%` the same way.

## Part 3 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Backups**, pick the host, **Find jobs**: `restic-backup` shows up. Add it.
- [ ] **Test restore** restores one random file with the unit's own environment, so the password never leaves the host.
- [ ] Turn on **Backup failed or stale** in Settings → Alerts.

#### Check

- [ ] `sudo bash -c 'set -a; . /etc/restic/env; restic snapshots'` lists last night's snapshot, and the board is green.

## When Nightgrid is closed

- Backups, pruning and checks run on the host. Only the alerts and test restores need Nightgrid.

## Limits and gotchas

- `--prune` rewrites data and can take a while on big repositories; move it to a weekly unit if nightly runs get slow.
- Two hosts must not share one repository path with different passwords. One repo per host is simplest.
- SFTP repositories need key login from root to the backup host: put root's public key there.

## Related

- [Backups](kb:basics/backups)
- [Monitor backups and test restores](kb:howto/backups)
- [Borg backups with borgmatic](kb:apps/borgmatic)
- [Offsite copies with rclone](kb:apps/rclone)
