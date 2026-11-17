# Outline team wiki

A fast, good-looking knowledge base with real-time collaborative editing, nested documents, search and Markdown shortcuts. It doesn't have its own passwords: users log in through an identity provider, so it pairs well with Authentik, Keycloak or any OIDC server you already run.

## What you end up with

- Outline with PostgreSQL and Redis, files stored on local disk
- Login through your OIDC provider
- Collections for homelab docs, runbooks and family notes

## Before you start

- An OIDC provider (see [Authentik single sign-on](kb:apps/authentik) or [Keycloak identity server](kb:apps/keycloak)) and a client created for Outline with the redirect URI `https://wiki.example.com/auth/oidc.callback`.
- A DNS name and HTTPS reverse proxy. Outline refuses to work well on plain HTTP outside localhost.

## Part 1 · The stack

- [ ] Create two secrets with `openssl rand -hex 32` (one for `SECRET_KEY`, one for `UTILS_SECRET`).
- [ ] [Docker](view:docker) → **+ New stack** named `outline`. Put the secrets, `DB_PASSWORD` and the OIDC client secret in `.env`:

```yaml
services:
  outline:
    image: outlinewiki/outline:latest
    restart: unless-stopped
    depends_on: [postgres, redis]
    environment:
      URL: https://wiki.example.com
      PORT: "3000"
      SECRET_KEY: ${SECRET_KEY}
      UTILS_SECRET: ${UTILS_SECRET}
      DATABASE_URL: postgres://outline:${DB_PASSWORD}@postgres:5432/outline
      PGSSLMODE: disable
      REDIS_URL: redis://redis:6379
      FILE_STORAGE: local
      FILE_STORAGE_LOCAL_ROOT_DIR: /var/lib/outline/data
      OIDC_CLIENT_ID: outline
      OIDC_CLIENT_SECRET: ${OIDC_SECRET}
      OIDC_AUTH_URI: https://auth.example.com/application/o/authorize/
      OIDC_TOKEN_URI: https://auth.example.com/application/o/token/
      OIDC_USERINFO_URI: https://auth.example.com/application/o/userinfo/
      OIDC_DISPLAY_NAME: Homelab login
    volumes:
      - ./data:/var/lib/outline/data
    ports:
      - "127.0.0.1:3000:3000"

  postgres:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: outline
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: outline
    volumes:
      - ./db:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    restart: unless-stopped
```

- [ ] The three `OIDC_*_URI` values above are Authentik's; for Keycloak they are under `https://auth.example.com/realms/<realm>/protocol/openid-connect/` (`auth`, `token`, `userinfo`).
- [ ] The container runs as uid 1001, so before the first start: `sudo mkdir -p data && sudo chown 1001:1001 data` in the stack folder.
- [ ] Point the reverse proxy at `127.0.0.1:3000`. Pin a version tag instead of `latest` once it works.

## Part 2 · First steps

- [ ] Log in: the first user becomes the admin.
- [ ] **Settings → Security**: limit sign-ups to your email domain or turn off new sign-ups, and set the default role for new users (Viewer is a safe start).
- [ ] Create collections such as **Homelab**, **Runbooks** and **Family**, with permissions per collection.

#### Check

- [ ] Two browsers editing the same document see each other's cursors, and an image pasted into a document survives a container restart.

## In Nightgrid

- Add the wiki to [Services](view:services) and mark it as depending on your identity provider in **Settings → Alerts → Dependencies**.
- Runbooks that should open from an alert belong in Nightgrid's [Knowledge](view:notes) notes, which DRIFT reads; Outline is for longer docs your household or team shares.

## Limits and gotchas

- No OIDC (or Slack/Google/Microsoft login) means no way in. Keep the identity provider backed up.
- Back up the database dump and the `data` folder together.
- Outline's license (BSL) allows self-hosting for your own use; read it if you plan to offer it to others as a service.

## Related

- [BookStack wiki](kb:apps/bookstack)
- [Wiki.js documentation wiki](kb:apps/wikijs)
- [Write runbooks for alerts](kb:howto/runbooks)
