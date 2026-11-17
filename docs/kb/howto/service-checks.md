# Watch services and expiry dates

Put every web app on one board with up/down checks, response times and certificate days, and track everything else that runs out on a date: domains, API tokens and licences.

## What you end up with

- A **Services** board with 30 days of history per app
- Alerts when an app is down for about 90 seconds or a certificate is close to expiry
- One soonest-first list of everything that expires

## Part 1 · Add services

- [ ] [Services](view:services) → **Add service**: name, group, URL and optionally the healthy status codes (`200-399` by default).
- [ ] Already use Homepage? **Import Homepage** reads its `services.yaml` (see [Homepage dashboard](kb:apps/homepage)).
- [ ] Self-signed certificate on a LAN app: tick **Allow untrusted certificate** for that one check.

## Part 2 · Read a failure

- [ ] A failed check says why: *HTTP 502 · from nginx at 192.0.2.10 · "502 Bad Gateway"* means the proxy answered but the app behind it didn't.
- [ ] A connection error lists every address the name resolved to. Wrong IP? Check DNS (Pi-hole, AdGuard or your router).
- [ ] Checks try IPv6 and IPv4 side by side, like a browser, and retry once before calling a service down.

## Part 3 · Integrations

- [ ] **Add integration** adds richer cards: Plex (now playing), AMP (game servers), Uptime Kuma (status page monitors), Portainer (links), Pi-hole v6 and AdGuard Home (stats and controls). Tokens go to the OS keychain.

## Part 4 · Expiry watch

- [ ] At the bottom of Services. HTTPS services appear automatically; proxy certificates appear once you've opened Docker → **Routes**.
- [ ] **Add…** a certificate file on a host (read with `openssl` over SSH), a domain (type the date, or **Look up** with `whois` from a host), or an API token or licence with a typed date. Only the name and date are stored.
- [ ] Set **Warn days before** per item. Inside that window an **EXPIRING** alert fires (critical in the last 3 days).

#### Check

- [ ] Stop a test container: its service turns red and an alert shows in the status bar within about two minutes.

## When Nightgrid is closed

- Checks run while Nightgrid runs, including in the tray with the window closed. For checks that never sleep, also run [Uptime Kuma](kb:apps/uptime-kuma) on a server and add it as an integration.

## Limits and gotchas

- Checks run from this computer. A service only reachable on the LAN looks down when you're away unless the tailnet routes to it.
- A login page that answers 200 counts as up even when the app behind it is broken. Point the check at a health endpoint when the app has one.

## Related

- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Uptime Kuma](kb:apps/uptime-kuma)
- [Homepage dashboard](kb:apps/homepage)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
