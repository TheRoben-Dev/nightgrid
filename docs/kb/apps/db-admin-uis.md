# Database web UIs: Adminer and pgAdmin

Browse tables, run queries and fix a broken row in the PostgreSQL or MariaDB containers your apps use, from a browser, without publishing a database port. Adminer is one tiny PHP page for every database type; pgAdmin is the full PostgreSQL tool.

## What you end up with

- Adminer (and optionally pgAdmin) on loopback only, opened through Nightgrid's tunnel
- Access to app databases over their own Docker networks
- A safe habit: dump before you edit

## Part 1 · Find the database's network

- [ ] App stacks usually put their database on the stack's default network, named `<stack>_default`. List them with `docker network ls`.
- [ ] The database's host name on that network is its service name (`db`, `postgres`, `mariadb`). The user and password are in that stack's `.env`.

## Part 2 · Adminer

- [ ] [Docker](view:docker) → **+ New stack** named `dbadmin`. Join each app network you want to reach:

```yaml
services:
  adminer:
    image: adminer:latest
    restart: unless-stopped
    environment:
      ADMINER_DESIGN: dracula
    ports:
      - "127.0.0.1:8081:8080"
    networks:
      - default
      - immich_default
      - nextcloud_default

networks:
  immich_default:
    external: true
  nextcloud_default:
    external: true
```

- [ ] **⧉** on the container's row opens it in a Nightgrid window through an SSH tunnel, since the port is bound to `127.0.0.1`.
- [ ] Log in with system **PostgreSQL** or **MySQL**, server set to the database's service name on that network (`database` in Immich's stack, `db` in many others), and the app's user and password.

## Part 3 · pgAdmin (optional)

- [ ] For heavier PostgreSQL work (query plans, roles, backups from the UI), add to the same stack:

```yaml
  pgadmin:
    image: dpage/pgadmin4:latest
    restart: unless-stopped
    environment:
      PGADMIN_DEFAULT_EMAIL: admin@example.com
      PGADMIN_DEFAULT_PASSWORD: ${PGADMIN_PASSWORD}
    volumes:
      - ./pgadmin:/var/lib/pgadmin
    ports:
      - "127.0.0.1:5050:80"
    networks:
      - default
      - immich_default
```

- [ ] The data folder must belong to uid 5050: `sudo chown -R 5050:5050 pgadmin`. Register each server under **Servers → Register** with the service name as host.

## Part 4 · Edit safely

- [ ] Before changing anything, take a dump from the database container (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)).
- [ ] Stop the app container while you change its data, so it doesn't overwrite your edit from its cache.

#### Check

- [ ] Adminer lists the app's tables, and `curl http://<host-lan-ip>:8081` from another machine is refused.

## In Nightgrid

- These tools hold no data of their own except pgAdmin's saved servers; you don't need them in backups.
- Stop the `dbadmin` stack in [Docker](view:docker) when you're done; there's no reason to leave a database UI running.

## Limits and gotchas

- An external network must exist before the stack starts; if an app stack is down and removed, `dbadmin` won't start until you drop that network from its list.
- Never publish these UIs to the internet. Even with a password they're a direct path into every database they can reach.
- Some apps (Immich, Nextcloud) change their schema on upgrade; editing tables by hand can break the next upgrade.

## Related

- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
- [Open app web UIs inside Nightgrid](kb:howto/web-uis)
- [Docker networks: bridge, shared proxy network and macvlan](kb:linux/docker-networks)
