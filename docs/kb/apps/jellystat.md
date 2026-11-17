# Jellystat for Jellyfin statistics

Watch history and statistics for Jellyfin: who watched what, most played titles, active devices and libraries' growth. The Jellyfin counterpart to Tautulli.

![Jellystat keeping Jellyfin's watch history.](fig:app-watch-stats)

## What you end up with

- Watch history per user, title and device
- Library and playback statistics over time
- A dashboard separate from Jellyfin itself

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `jellystat`, with secrets in `.env` (`JWT_SECRET` from `openssl rand -hex 32`):

```yaml
services:
  jellystat-db:
    image: postgres:16
    restart: unless-stopped
    environment:
      POSTGRES_USER: jellystat
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/postgresql/data
  jellystat:
    image: cyfershepard/jellystat:latest
    restart: unless-stopped
    depends_on: [jellystat-db]
    environment:
      POSTGRES_USER: jellystat
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_IP: jellystat-db
      POSTGRES_PORT: "5432"
      JWT_SECRET: ${JWT_SECRET}
      TZ: Etc/UTC
    volumes:
      - ./backup:/app/backend/backup-data
    ports:
      - "3000:3000"
```

## Part 2 · Connect Jellyfin

- [ ] Open port 3000, create the Jellystat login.
- [ ] In Jellyfin: Dashboard → API Keys → **+** for Jellystat. Enter Jellyfin's URL and that key in Jellystat.
- [ ] Settings → Tasks: run a full sync once; afterwards it syncs on its own. Install the Jellyfin **Playback Reporting** plugin and import its history to get older plays.

#### Check

- [ ] Play something in Jellyfin: it shows under Activity within a minute.

## In Nightgrid

- Transcodes show as load on the Jellyfin host in [Overview](view:overview); Jellystat tells you which user and client.
- Back up the database with `pg_dump` like other Postgres stacks.

## Limits and gotchas

- Port 3000 clashes with many apps; change the host side if needed.
- Pin `postgres` to a major version; upgrading it needs a dump and restore.

## Related

- [Jellyfin media server](kb:apps/jellyfin)
- [Tautulli Plex statistics](kb:apps/tautulli)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
