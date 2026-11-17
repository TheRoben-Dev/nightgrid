# Umami web analytics

Privacy-friendly visitor stats for your own sites: page views, referrers, countries and devices, without cookies or sending data to a third party.

## What you end up with

- A dashboard per website with views, visitors and sources
- A small tracking script on your sites, served from your server
- No cookie banner needed for most setups

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `umami`, secrets in `.env` (`APP_SECRET` from `openssl rand -hex 32`):

```yaml
services:
  umami:
    image: ghcr.io/umami-software/umami:postgresql-latest
    restart: unless-stopped
    depends_on: [db]
    ports:
      - "3002:3000"
    environment:
      DATABASE_URL: postgresql://umami:${DB_PASSWORD}@db:5432/umami
      APP_SECRET: ${APP_SECRET}
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_DB: umami
      POSTGRES_USER: umami
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/postgresql/data
```

- [ ] Log in with `admin` / `umami` and change the password at once.

## Part 2 · Track a site

- [ ] Settings → Websites → Add, then copy the tracking code: a `script` tag with your Umami URL and the site's id. Paste it into your site's page head.
- [ ] Publish Umami on a public name through your reverse proxy so visitors' browsers can reach the script.

#### Check

- [ ] Open your site in a private window; the visit shows in Umami's realtime view.

## In Nightgrid

- Publish it with [Manage reverse proxy routes](kb:howto/proxy-routes) and check what's exposed afterwards on the [Network](view:network) map.
- Add it to [Services](view:services) for uptime and certificate expiry.

## Limits and gotchas

- Ad blockers block common analytics script names; Umami lets you rename the script with `TRACKER_SCRIPT_NAME`.
- It's for your own sites. Tracking other people's devices or traffic is a different (and usually unwelcome) thing.

## Related

- [Caddy reverse proxy](kb:apps/caddy)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
