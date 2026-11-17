# Backups

A backup you haven't restored is a hope, not a backup. The aim is simple: when a disk dies, a container corrupts its database or you delete the wrong folder, you can get the data back, and you know it works because you've tried.

![The 3-2-1 rule, with the restore test that proves it.](fig:backups-321)

## The 3-2-1 rule

- **3** copies of data that matters (the original and two backups),
- on **2** different kinds of storage (another disk, a NAS),
- **1** of them off-site (a friend's house, cloud storage).

RAID, ZFS mirrors and snapshots are not backups: they protect against a failed disk, not against deletion, ransomware or a fire.

## Tools

- **restic**: encrypted, deduplicated snapshots to a local folder, SFTP, S3-compatible storage and more. Simple and widely used.
- **borg** (with **borgmatic**): similar, very efficient, needs borg or SSH on the target.
- **rsync**: a plain mirror. Easy to read back, but no history and no encryption on its own.
- **Proxmox vzdump / Proxmox Backup Server**: whole VMs and containers.

A restic example:

```sh
export RESTIC_REPOSITORY=/mnt/backup/restic
export RESTIC_PASSWORD_FILE=/root/.restic-pass   # mode 600, never on the command line
restic init
restic backup /srv/appdata --exclude-caches
restic forget --keep-daily 7 --keep-weekly 4 --keep-monthly 6 --prune
```

Keep a copy of the password outside the server (a password manager). A lost restic password is a lost backup.

## What to back up

- App data folders and compose files (one folder per stack makes this easy).
- **Databases**: dump them first (`pg_dump`, `mysqldump`) or stop the container; copying live database files can give you a corrupt copy.
- `/etc`, crontabs and anything you configured by hand.
- Not: container images, caches, media you can re-download (unless re-downloading is the painful part).

## In Nightgrid

- Fleet → **BACKUPS** lists restic, borg and rsync jobs per host with their last run, size and result, flags stale ones and raises an alert when a job fails or goes quiet.
- **Test restore** restores a sample into a temporary folder on the host and checks it, so you know it works.
- Nightgrid's own data (hosts, settings, notes, optionally secrets) is exported from Settings → **DATA** → **BACKUP · PROFILE**, encrypted with age.

## Schedule a restore drill

Every few months, restore one real thing (a database, a photo folder) somewhere harmless and open it. Put the steps in a runbook note in Knowledge so the next time is quicker.

## Related

- [Docker and compose basics](kb:basics/docker-compose): where container data lives.
- [Family cloud and photos](kb:guides/family-cloud-photos): a full setup with test restores.
