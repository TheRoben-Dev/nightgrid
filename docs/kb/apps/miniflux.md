# Miniflux feed reader

A minimalist, very fast RSS reader written in Go with PostgreSQL behind it. Keyboard driven, readable on phones, and with a Fever and Google Reader API for mobile apps.

## What you end up with

- Feeds and categories in a clean web reader
- Full-content fetching for feeds that only send a summary
- An API for apps like Reeder, NetNewsWire or ReadYou

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `miniflux`, passwords in `.env`:

```yaml
services:
  miniflux:
    image: miniflux/miniflux:latest
    restart: unless-stopped
    depends_on:
      db:
        condition: service_healthy
    ports:
      - "8084:8080"
    environment:
      DATABASE_URL: postgres://miniflux:${DB_PASSWORD}@db/miniflux?sslmode=disable
      RUN_MIGRATIONS: "1"
      CREATE_ADMIN: "1"
      ADMIN_USERNAME: admin
      ADMIN_PASSWORD: ${ADMIN_PASSWORD}
      BASE_URL: http://rss.lan:8084
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_USER: miniflux
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: miniflux
    volumes:
      - ./db:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD", "pg_isready", "-U", "miniflux"]
      interval: 10s
```

## Part 2 · Use it

- [ ] Log in, Feeds → Import an OPML file from your old reader, or add feeds one by one.
- [ ] Per feed: **Fetch original content** for summary-only feeds.
- [ ] Settings → Integrations → enable the Fever or Google Reader API for mobile apps (with its own password).
- [ ] Keys: `j`/`k` next and previous, `v` open original, `m` toggle read, `?` all shortcuts.

#### Check

- [ ] Refresh a feed: new entries appear with full text where you turned it on.

## In Nightgrid

- Back up the database with `pg_dump`, not by copying `./db` while it runs: [Back up databases the right way](kb:apps/databases).
- Add it to [Services](view:services).

## Limits and gotchas

- Upgrading PostgreSQL's major version (16 to 17) needs a dump and restore; pin the version in compose.
- After the first start, `CREATE_ADMIN` and the admin password can be removed from `.env`.

## Related

- [FreshRSS feed reader](kb:apps/freshrss)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
- [linkding bookmarks](kb:apps/linkding)
