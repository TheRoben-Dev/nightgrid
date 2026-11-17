# GoToSocial fediverse server

A lightweight ActivityPub server for yourself or a few friends: follow and talk with people on Mastodon and the rest of the fediverse from your own domain, on a server that runs comfortably in 200 MB of RAM with SQLite.

## What you end up with

- GoToSocial with SQLite behind your reverse proxy
- An admin account `@you@social.example.com`
- A Mastodon-compatible API, so phone apps like Tusky, Ivory or Mona work

## Before you start

- A domain and HTTPS. The **host name can never change** after the first start: accounts and posts are tied to it.
- GoToSocial has no web interface for writing posts; you use any Mastodon client app, or the minimal settings panel for your profile.

## Part 1 · The stack

- [ ] The container runs as uid 1000. In the stack folder: `sudo mkdir -p data && sudo chown 1000:1000 data`.
- [ ] [Docker](view:docker) → **+ New stack** named `gotosocial`:

```yaml
services:
  gotosocial:
    image: superseriousbusiness/gotosocial:latest
    restart: unless-stopped
    user: "1000:1000"
    environment:
      GTS_HOST: social.example.com
      GTS_DB_TYPE: sqlite
      GTS_DB_ADDRESS: /gotosocial/storage/sqlite.db
      GTS_LETSENCRYPT_ENABLED: "false"
      GTS_TRUSTED_PROXIES: "172.16.0.0/12,127.0.0.1/32"
      GTS_ACCOUNTS_REGISTRATION_OPEN: "false"
      TZ: Etc/UTC
    volumes:
      - ./data:/gotosocial/storage
    ports:
      - "127.0.0.1:8080:8080"
```

- [ ] Point the reverse proxy for `social.example.com` at `127.0.0.1:8080`. `GTS_TRUSTED_PROXIES` must include the address the proxy connects from, or rate limiting sees every visitor as the proxy.

## Part 2 · Your account

```bash
docker compose exec gotosocial /gotosocial/gotosocial admin account create \
  --username you --email you@example.com --password 'a long password'
docker compose exec gotosocial /gotosocial/gotosocial admin account promote --username you
docker compose restart gotosocial
```

- [ ] Log in to `https://social.example.com/settings` for profile, avatar and admin settings.
- [ ] In a Mastodon app, add the server `social.example.com` and sign in.

## Part 3 · Settle in

- [ ] Search for a friend's full handle (`@name@mastodon.social`) and follow them.
- [ ] **Settings → Admin → Federation**: GoToSocial can run in allowlist mode (only federate with servers you approve), or use domain blocks to keep bad servers out.

#### Check

- [ ] Someone on another server can find `@you@social.example.com`, follow you, and see a test post.

## In Nightgrid

- Add `https://social.example.com/api/v1/instance` to [Services](view:services).
- The whole server is the `data` folder (SQLite database and media). Stop the container or use `sqlite3 .backup` for a consistent copy, and keep it with your other backups.

## Limits and gotchas

- Media from other servers is cached; set `GTS_MEDIA_REMOTE_CACHE_DAYS` to keep the disk in check.
- Moving to another domain later isn't possible; you'd start a new account and use account migration from the old one.
- GoToSocial is still beta software; read release notes before upgrading and back up first.

## Related

- [Matrix chat with Synapse](kb:apps/matrix-synapse)
- [Caddy reverse proxy](kb:apps/caddy)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
