# Paperless-ngx documents

Scan or drop paper documents into a folder and get them OCR'd, tagged and searchable. Receipts, letters, warranties and contracts, all in one archive.

## What you end up with

- A consume folder (also reachable over SMB from your scanner) that imports automatically
- Full-text search, tags, correspondents and document types learned from your corrections
- An export you can back up and restore anywhere

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `paperless`:

```yaml
services:
  broker:
    image: redis:7-alpine
    restart: unless-stopped
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_DB: paperless
      POSTGRES_USER: paperless
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./pgdata:/var/lib/postgresql/data
  webserver:
    image: ghcr.io/paperless-ngx/paperless-ngx:latest
    restart: unless-stopped
    depends_on: [db, broker]
    ports:
      - "127.0.0.1:8000:8000"
    environment:
      PAPERLESS_REDIS: redis://broker:6379
      PAPERLESS_DBHOST: db
      PAPERLESS_DBPASS: ${DB_PASSWORD}
      PAPERLESS_URL: https://docs.example.com
      PAPERLESS_SECRET_KEY: ${SECRET_KEY}
      PAPERLESS_OCR_LANGUAGE: eng
      PAPERLESS_TIME_ZONE: Etc/UTC
      USERMAP_UID: 1000
      USERMAP_GID: 1000
    volumes:
      - ./data:/usr/src/paperless/data
      - ./media:/usr/src/paperless/media
      - ./export:/usr/src/paperless/export
      - /srv/scans:/usr/src/paperless/consume
```

- [ ] `DB_PASSWORD` and `SECRET_KEY` (a long random string) go in the **Vault**.
- [ ] Create the admin user:

```sh
docker compose -f /opt/stacks/paperless/compose.yaml exec webserver createsuperuser
```

- [ ] Route `docs.example.com` to port 8000.

## Part 2 · Feeding it

- [ ] Share `/srv/scans` over SMB (see [Samba file shares](kb:apps/samba)) and point your scanner's "scan to folder" at it.
- [ ] Phone: the Paperless mobile apps upload directly; or email documents in with a mail rule.
- [ ] Set `PAPERLESS_OCR_LANGUAGE` to your languages (for example `deu+eng`); the image includes common ones.

## Part 3 · Backups

- [ ] A [timer](kb:howto/schedules) at 01:30 running the exporter:

```sh
docker compose -f /opt/stacks/paperless/compose.yaml exec -T webserver document_exporter ../export --delete
```

- [ ] Back up `./export` with restic or borg; it restores into any fresh install with `document_importer`.

#### Check

- [ ] A PDF dropped in `/srv/scans` disappears from the folder and appears searchable in the UI.

## Limits and gotchas

- Files in the consume folder must be readable by `USERMAP_UID`.
- Consumption from network shares sometimes misses inotify events; set `PAPERLESS_CONSUMER_POLLING: 60` if new files sit there.

## Related

- [Samba file shares](kb:apps/samba)
- [Stirling-PDF](kb:apps/stirling-pdf)
- [restic backups with systemd timers](kb:apps/restic)
