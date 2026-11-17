# PostgreSQL and MariaDB in Docker

Run a database container for your apps, keep it off the network, back it up with dumps rather than copying live files, and upgrade major versions safely.

## What you end up with

- PostgreSQL or MariaDB in a compose stack, reachable only from its app
- Nightly dumps on a timer that your file backups pick up
- A tested way to restore and to upgrade

## Part 1 · Compose

- [ ] Most apps ship their own database in their compose file. Use that one, and keep it on the stack's internal network without a `ports:` entry.
- [ ] A standalone PostgreSQL:

```yaml
services:
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_USER: app
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: app
    volumes:
      - ./pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U app"]
      interval: 30s
```

- [ ] MariaDB: `image: mariadb:11`, `MARIADB_ROOT_PASSWORD`, `MARIADB_DATABASE`, `MARIADB_USER`, `MARIADB_PASSWORD`, data in `/var/lib/mysql`.
- [ ] Put the passwords in the stack's **Vault** in [Docker](view:docker).
- [ ] If something outside Docker must connect, publish as `127.0.0.1:5432:5432`, never `5432:5432`.

## Part 2 · Dumps

- [ ] PostgreSQL, as a [timer](kb:howto/schedules) at 02:00:

```sh
docker exec <stack>-db-1 pg_dumpall -U app | gzip > /srv/backups/db/pg-$(date +%F).sql.gz
```

- [ ] MariaDB:

```sh
docker exec <stack>-db-1 sh -c 'mariadb-dump --all-databases -uroot -p"$MARIADB_ROOT_PASSWORD"' | gzip > /srv/backups/db/maria-$(date +%F).sql.gz
```

- [ ] Keep 14 days: `find /srv/backups/db -name '*.sql.gz' -mtime +14 -delete` in the same timer.
- [ ] Let restic or borg back up `/srv/backups/db` after the dump has run.

## Part 3 · Restore test

- [ ] Restore into a throwaway container to prove the dump works:

```sh
docker run -d --name pgtest -e POSTGRES_PASSWORD=test postgres:17
gunzip -c /srv/backups/db/pg-<date>.sql.gz | docker exec -i pgtest psql -U postgres
docker rm -f pgtest
```

## Part 4 · Major upgrades

- [ ] PostgreSQL's data folder doesn't upgrade across major versions by itself. Dump, stop, move the old data folder aside, start the new version with an empty folder, restore the dump.
- [ ] MariaDB upgrades in place; set `MARIADB_AUTO_UPGRADE: "1"` and take a dump first.

#### Check

- [ ] [System](view:system) → **security** shows no database on all interfaces.

## Limits and gotchas

- Copying `pgdata` while the database runs gives a backup that may not start. Dumps or filesystem snapshots (ZFS) are safe.
- Don't share one database container between unrelated apps unless you're happy upgrading them together.

## Related

- [restic backups with systemd timers](kb:apps/restic)
- [Update containers without fear](kb:howto/container-updates)
- [Deploy a new compose stack](kb:howto/deploy-stack)
