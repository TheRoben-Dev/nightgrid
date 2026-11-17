# Seafile file sync

Fast file sync and sharing with desktop and mobile clients, built around "libraries" that sync block by block. It does one thing (files) and does it quicker and lighter than Nextcloud, with optional client-side encryption per library.

## What you end up with

- Seafile 12 Community Edition with MariaDB and Memcached
- Desktop clients syncing libraries, and share links for others
- An encrypted library the server can't read

## Before you start

- A DNS name (`files.example.com`) and HTTPS, either from Seafile's bundled Caddy or your own reverse proxy.
- 2 GB RAM. Data lives in a folder you choose; put it on the big disk or pool.

## Part 1 · Get the official files

- [ ] Seafile 12 ships its compose as several files plus an `.env`. In a terminal on the host:

```bash
sudo mkdir -p /opt/stacks/seafile && cd /opt/stacks/seafile
sudo wget -O .env https://manual.seafile.com/12.0/repo/docker/ce/env
sudo wget https://manual.seafile.com/12.0/repo/docker/ce/seafile-server.yml
sudo wget https://manual.seafile.com/12.0/repo/docker/caddy.yml
```

- [ ] If a link has moved, the Seafile admin manual's Docker → Community Edition page has the current ones.

## Part 2 · Fill in `.env`

- [ ] Set at least these keys:

```ini
SEAFILE_VOLUME=/srv/seafile/data
SEAFILE_MYSQL_VOLUME=/srv/seafile/mysql
INIT_SEAFILE_MYSQL_ROOT_PASSWORD=<long random>
SEAFILE_MYSQL_DB_PASSWORD=<long random>
JWT_PRIVATE_KEY=<openssl rand -hex 32>
SEAFILE_SERVER_HOSTNAME=files.example.com
SEAFILE_SERVER_PROTOCOL=https
INIT_SEAFILE_ADMIN_EMAIL=you@example.com
INIT_SEAFILE_ADMIN_PASSWORD=<long random>
TIME_ZONE=Etc/UTC
```

- [ ] Already running a reverse proxy? Remove `caddy.yml` from the `COMPOSE_FILE` line in `.env` and point your proxy at the Seafile container's port 80 instead (publish it on `127.0.0.1` only).
- [ ] `docker compose up -d`, then open `https://files.example.com`. The `INIT_` values only apply on the first start.

## Part 3 · Clients and sharing

- [ ] Install the desktop **Seafile client** (syncs whole libraries) or **SeaDrive** (shows everything as a virtual drive and downloads on demand).
- [ ] **New library** → tick **Encrypt** and set a password for private data. The password never reaches the server; lose it and the library is gone.
- [ ] Share links can have a password and an expiry date; upload links let others send you files.

#### Check

- [ ] A file saved on one computer appears on another within seconds, and the web UI shows its history under **History**.

## In Nightgrid

- [Docker](view:docker) shows the stack once it's running; add `https://files.example.com` to [Services](view:services).
- Back up `SEAFILE_VOLUME` and a MariaDB dump together; restoring one without the other loses files. See [PostgreSQL and MariaDB in Docker](kb:apps/databases).

## Limits and gotchas

- Files are stored as blocks, not plain files on disk. You can't browse the data folder directly; use the web UI, the client or `seaf-fuse`.
- Deleted files stay in the library's trash and history; set **History** retention per library or the disk keeps growing. Run garbage collection after big cleanups.
- Older guides use other variable names (Seafile 11 and earlier); stick with the files that match your version.

## Related

- [Nextcloud](kb:apps/nextcloud)
- [Syncthing](kb:apps/syncthing)
- [Family cloud and photos](kb:guides/family-cloud-photos)
