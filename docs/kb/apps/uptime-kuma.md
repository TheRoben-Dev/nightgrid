# Uptime Kuma

An always-on monitor on a server: HTTP, TCP, ping, DNS and Docker checks every minute, status pages, and notifications, even when your laptop is closed. Nightgrid reads its status page into the Services board.

## What you end up with

- 24/7 checks that don't depend on Nightgrid running
- A public or private status page
- Kuma's monitors on a Nightgrid card

## Part 1 · The stack

- [ ] Run it on a different host from the apps it watches (or better, a small VPS that watches from outside).
- [ ] [Docker](view:docker) → **+ New stack** named `uptime-kuma`:

```yaml
services:
  uptime-kuma:
    image: louislam/uptime-kuma:1
    restart: unless-stopped
    volumes:
      - ./data:/app/data
    ports:
      - "3001:3001"
```

- [ ] Open port 3001 and create the admin account.

## Part 2 · Monitors

- [ ] Add monitors for each app (HTTP keyword checks are better than plain status codes), DNS servers, and the router (ping).
- [ ] Notifications: ntfy, email, Discord, Telegram and many more; set one as default.
- [ ] Status Pages → New: pick the monitors to show and a slug.

## Part 3 · In Nightgrid

- [ ] [Services](view:services) → **Add integration** → **Uptime Kuma** with the status page URL. The card lists those monitors.
- [ ] Keep Nightgrid's own service checks too: they add 30-day history, certificate expiry and alert folding.

#### Check

- [ ] Stop a monitored container: Kuma notifies within a minute or two, with Nightgrid closed.

## Limits and gotchas

- Version 2 changes the database layout; read the migration notes before moving from the `:1` tag.
- Kuma on the same host as everything else can't tell you when that host is down.

## Related

- [Watch services and expiry dates](kb:howto/service-checks)
- [ntfy push notifications](kb:apps/ntfy)
- [Phone status page and snapshots](kb:howto/status-page)
