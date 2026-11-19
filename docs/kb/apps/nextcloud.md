# Nextcloud

Files, calendars, contacts and office documents on your own server. This guide uses the official image with PostgreSQL and Redis behind your reverse proxy.

![What a healthy Nextcloud install is made of.](fig:app-nextcloud)

## What you end up with

- Nextcloud at `cloud.example.com` with desktop and phone sync
- Background jobs, caching and correct proxy settings so the admin overview is clean
- Database dumps and data folder in your backups

## Before you start

- 2 GB RAM free and a disk with room for everyone's files (a ZFS dataset is ideal).
- A reverse proxy with HTTPS (see [Caddy reverse proxy](kb:apps/caddy)).
- Prefer an all-in-one appliance? Nextcloud AIO manages its own containers and backups; it's simpler but harder to customise. This guide is the manual route.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `nextcloud`:

```yaml
services:
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_DB: nextcloud
      POSTGRES_USER: nextcloud
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/postgresql/data
  redis:
    image: redis:7-alpine
    restart: unless-stopped
  app:
    image: nextcloud:31-apache
    restart: unless-stopped
    depends_on: [db, redis]
    environment:
      POSTGRES_HOST: db
      POSTGRES_DB: nextcloud
      POSTGRES_USER: nextcloud
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      REDIS_HOST: redis
      NEXTCLOUD_TRUSTED_DOMAINS: cloud.example.com
      OVERWRITEPROTOCOL: https
      TRUSTED_PROXIES: 172.16.0.0/12
      PHP_UPLOAD_LIMIT: 10G
    volumes:
      - ./html:/var/www/html
      - /tank/nextcloud:/var/www/html/data
    networks: [default, proxy]
  cron:
    image: nextcloud:31-apache
    restart: unless-stopped
    entrypoint: /cron.sh
    depends_on: [app]
    volumes:
      - ./html:/var/www/html
      - /tank/nextcloud:/var/www/html/data
networks:
  proxy:
    external: true
```

- [ ] `DB_PASSWORD` goes in the **Vault**.
- [ ] Route `cloud.example.com` to `app:80` in your proxy. For Caddy, add the CalDAV/CardDAV redirects:

```
cloud.example.com {
    redir /.well-known/carddav /remote.php/dav 301
    redir /.well-known/caldav /remote.php/dav 301
    reverse_proxy app:80
}
```

## Part 2 · First run and tuning

- [ ] Open the site and create the admin account.
- [ ] Administration settings → Basic settings: Background jobs → **Cron** (the `cron` container runs it).
- [ ] Fix the overview warnings with `occ` (in a [Terminal](view:terminal)):

```sh
docker exec -u www-data nextcloud-app-1 php occ maintenance:repair --include-expensive
docker exec -u www-data nextcloud-app-1 php occ db:add-missing-indices
docker exec -u www-data nextcloud-app-1 php occ config:system:set maintenance_window_start --type=integer --value=1
```

- [ ] Add a snippet in [Automation](view:automation) for `occ` with a `{{args}}` variable; you'll use it often.

## Part 3 · Backups and updates

- [ ] Dump the database nightly (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)) and back up the data folder and `./html/config`.
- [ ] [Services](view:services) → **Add integration** → **Nextcloud** with an admin login and an app password: users, free space, load and pending app updates on a card (see [Add app cards](kb:howto/integrations)).
- [ ] Update one major version at a time (30 → 31 → 32). Change the tag, pull, up, then watch logs until the upgrade finishes. Nightgrid's stack snapshot makes rollback possible before the database migrates; after that, restore the dump.

#### Check

- [ ] Administration overview shows no errors, and the desktop client syncs a test file.

## Limits and gotchas

- The data folder must be owned by uid 33 (`www-data`): `sudo chown -R 33:33 /tank/nextcloud`.
- Skipping a major version on upgrade is not supported and fails.
- Office editing needs Collabora or OnlyOffice as an extra container.

## Related

- [Family cloud and photos](kb:guides/family-cloud-photos)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
- [Syncthing](kb:apps/syncthing)
- [Caddy reverse proxy](kb:apps/caddy)
