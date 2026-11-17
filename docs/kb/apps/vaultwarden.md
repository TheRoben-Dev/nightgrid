# Vaultwarden password manager

A lightweight server compatible with the Bitwarden apps and browser extensions. Your vault, on your hardware.

![Vaultwarden behind HTTPS, with backups.](fig:app-vaultwarden)

## What you end up with

- Bitwarden apps on every device syncing to your server
- Sign-ups closed, an admin page protected by a hashed token
- Nightly backups of the vault, tested

## Before you start

- HTTPS is required by the web vault and apps. Use a reverse proxy with a real certificate (see [Caddy reverse proxy](kb:apps/caddy)).
- Decide how devices reach it away from home: a tailnet (recommended) or the public internet.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `vaultwarden`:

```yaml
services:
  vaultwarden:
    image: vaultwarden/server:latest
    restart: unless-stopped
    environment:
      DOMAIN: https://vault.example.com
      SIGNUPS_ALLOWED: "true"
      ADMIN_TOKEN: ${ADMIN_TOKEN}
    volumes:
      - ./data:/data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Make a hashed admin token and put the whole hash in the **Vault** as `ADMIN_TOKEN`:

```sh
docker run --rm -it vaultwarden/server /vaultwarden hash
```

- [ ] In the stack's `.env`, `$` signs in the hash must be doubled (`$$`) for compose. The Vault's **Deploy .env…** shows the final value.
- [ ] Route `vault.example.com` to `vaultwarden:80` in your proxy.

## Part 2 · Accounts

- [ ] Create your account in the web vault, then set `SIGNUPS_ALLOWED: "false"` and recreate the stack.
- [ ] Invite family from the admin page (`/admin`) instead. Invitations need SMTP settings (see [Send mail from your servers](kb:apps/mail-relay)).
- [ ] Turn on two-step login for every account.

## Part 3 · Backups

- [ ] The vault is `./data/db.sqlite3` plus `attachments`, `sends`, `rsa_key*` and `config.json`. Back up a consistent copy with a [timer](kb:howto/schedules) (install `sqlite3` on the host first):

```sh
sqlite3 /opt/stacks/vaultwarden/data/db.sqlite3 ".backup '/srv/backups/vaultwarden/db-$(date +%F).sqlite3'"
```

- [ ] Include the rest of `./data` in restic or borg, and add the job in [Fleet](view:fleet) → **Backups**.
- [ ] Export an encrypted vault from the web vault once in a while and keep it offline.

## Part 4 · In Nightgrid

- [ ] Add `vault.example.com` to [Services](view:services) with expiry watch.
- [ ] [System](view:system) → **security**: port 80 of the container must not be published; only the proxy reaches it.

#### Check

- [ ] A browser extension logs in, and the backup board is green.

## Limits and gotchas

- Clients keep an offline copy, so a short outage is harmless; a lost server without backups is not.
- Exposing Vaultwarden publicly works but makes it a target. A tailnet-only setup keeps the login page off the internet.
- Pin the image version after setup and read release notes before updates.

## Related

- [Caddy reverse proxy](kb:apps/caddy)
- [Monitor backups and test restores](kb:howto/backups)
- [Reach your servers away from home](kb:howto/remote-access)
