# Mattermost team chat

A self-hosted Slack alternative: channels, threads, direct messages, file sharing, search and webhooks, with desktop and mobile apps. Good for a team, a club or a homelab "ops" channel where scripts post alerts.

## What you end up with

- Mattermost Team Edition with PostgreSQL behind your reverse proxy
- A team with channels, and sign-up by invitation only
- An incoming webhook your scripts can post to

## Before you start

- HTTPS and a name like `chat.example.com`. The mobile apps require HTTPS.
- 2 GB RAM for a small team.

## Part 1 · The stack

- [ ] Mattermost runs as uid 2000. Create its folders first, in the stack folder:

```bash
sudo mkdir -p config data logs plugins client-plugins
sudo chown -R 2000:2000 config data logs plugins client-plugins
```

- [ ] [Docker](view:docker) → **+ New stack** named `mattermost`, with `DB_PASSWORD` in `.env`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: mmuser
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: mattermost
    volumes:
      - ./db:/var/lib/postgresql/data

  mattermost:
    image: mattermost/mattermost-team-edition:latest
    restart: unless-stopped
    depends_on: [db]
    environment:
      TZ: Etc/UTC
      MM_SQLSETTINGS_DRIVERNAME: postgres
      MM_SQLSETTINGS_DATASOURCE: postgres://mmuser:${DB_PASSWORD}@db:5432/mattermost?sslmode=disable&connect_timeout=10
      MM_SERVICESETTINGS_SITEURL: https://chat.example.com
    volumes:
      - ./config:/mattermost/config
      - ./data:/mattermost/data
      - ./logs:/mattermost/logs
      - ./plugins:/mattermost/plugins
      - ./client-plugins:/mattermost/client/plugins
    ports:
      - "127.0.0.1:8065:8065"
```

- [ ] Point the reverse proxy at `127.0.0.1:8065` with WebSocket support (Caddy and Traefik pass WebSockets by default). Pin a version tag once it works.

## Part 2 · First steps

- [ ] Open the site: the first account becomes the system admin. Create a team.
- [ ] **System Console → Authentication → Signup**: turn off open account creation; invite people by link or email instead.
- [ ] **System Console → Environment → SMTP** with your mail relay, so invitations and notifications by email work.

## Part 3 · A webhook for alerts

- [ ] **Integrations → Incoming Webhooks → Add**, pick a channel such as `#homelab`. Copy the URL.
- [ ] Post to it from any script or cron job:

```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"text":"Backup on nas-01 finished"}' \
  https://chat.example.com/hooks/<id>
```

#### Check

- [ ] The phone app connects to `https://chat.example.com`, and the test message appears in the channel.

## In Nightgrid

- Add the site to [Services](view:services). Nightgrid's own alerts stay in Nightgrid; add a `curl` step to [Automation](view:automation) playbooks or scripts that should tell the team when they finish.
- Back up a database dump plus the `config` and `data` folders.

## Limits and gotchas

- Team Edition has no SSO with OIDC/SAML (those are paid features); GitLab login works. For SSO-first chat, look at Matrix with your identity provider.
- Push notifications to phones go through Mattermost's push proxy by default (message content can be hidden in **Notifications → Push Notification Contents**).
- Upgrades must not skip required migration versions; read the upgrade notes before jumping several releases.

## Related

- [Matrix chat with Synapse](kb:apps/matrix-synapse)
- [Send mail from your servers](kb:apps/mail-relay)
- [ntfy push notifications](kb:apps/ntfy)
