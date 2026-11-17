# Duplicati backups with a web UI

Encrypted, deduplicated backups to almost any destination (another server over SFTP, S3, Backblaze B2, a USB disk, WebDAV, Google Drive and many more), set up from a browser. A friendly choice for a machine whose owner won't touch a terminal.

![Duplicati backing up to a local and an offsite target.](fig:app-backup-repo)

## What you end up with

- Scheduled, encrypted, versioned backups configured in a web UI
- Retention like "keep daily for a week, weekly for a month"
- Restores by browsing to a file and a date

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `duplicati`, with secrets in `.env`:

```yaml
services:
  duplicati:
    image: lscr.io/linuxserver/duplicati:latest
    restart: unless-stopped
    environment:
      PUID: "0"
      PGID: "0"
      TZ: Etc/UTC
      SETTINGS_ENCRYPTION_KEY: ${SETTINGS_KEY}
      DUPLICATI__WEBSERVICE_PASSWORD: ${UI_PASSWORD}
    volumes:
      - ./config:/config
      - /srv:/source/srv:ro
      - /opt/stacks:/source/stacks:ro
    ports:
      - "8200:8200"
```

- [ ] `PUID 0` lets it read files owned by any user; the source mounts are read-only so it can't change them.

## Part 2 · A backup job

- [ ] Add backup → name, and a **passphrase** for encryption (store it in your password manager; without it nothing can be restored).
- [ ] Destination: SFTP to another host, S3-compatible storage (Garage, MinIO, B2), or a mounted disk. **Test connection**.
- [ ] Source: tick folders under `/source`. Exclude caches and temp folders.
- [ ] Schedule: daily at night. Options → retention: **Smart backup retention** is a sensible default.

## Part 3 · Restore test

- [ ] Restore → pick the job, a date, a file, restore to a different folder. Do this once now, not when you need it.

#### Check

- [ ] The job shows a successful run, and the restored file matches the original.

## In Nightgrid

- [Fleet](view:fleet) → **Backups** reads restic, borg and rsync jobs; for Duplicati, watch its web UI or let it send email reports. Add the web UI to [Services](view:services).
- Databases need a dump before the backup runs; see [Back up databases the right way](kb:apps/databases).

## Limits and gotchas

- Duplicati's local database can get corrupted; the **Recreate** repair works but is slow on big backups. restic or Kopia are sturdier for large servers.
- Keep at least one copy off-site and one you've restored from.

## Related

- [Backups](kb:basics/backups)
- [Monitor backups and test restores](kb:howto/backups)
- [Kopia backups](kb:apps/kopia)
