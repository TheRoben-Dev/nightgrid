# Phone status page and snapshots

Check your hosts, alerts and services from your phone through a read-only page served by Nightgrid, or save the same page as one HTML file.

## What you end up with

- A secret link that shows hosts, alerts, service checks and 90-day uptime bars
- A home-screen shortcut on your phone that refreshes every 30 seconds
- Snapshot files to attach to a ticket or archive

## Part 1 · Serve it

- [ ] **Settings → Status page · Snapshot → Serve the phone status page** (off by default).
- [ ] Pick where it listens: **LAN address**, **Tailscale only** (best: works anywhere your phone is on the tailnet), or **every interface** (avoid on machines with a public address).
- [ ] **Copy** the link and open it on your phone; add it to the home screen.
- [ ] **New link** makes the old one stop working.

## Part 2 · Snapshots

- [ ] **Save snapshot…** writes the page as one HTML file. Host addresses are only included if you tick them.

#### Check

- [ ] Stop a test service: the phone page shows it red within a minute.

## When Nightgrid is closed

- The page is served by Nightgrid, so it's down while Nightgrid is closed. Keep Nightgrid in the tray, or run [Uptime Kuma](kb:apps/uptime-kuma) for an always-on status page.

## Limits and gotchas

- Anyone with the link can read the page. Don't post it publicly; use **New link** if it leaks.
- It's read-only by design: no buttons on the page change anything.

## Related

- [Uptime history and monthly reports](kb:howto/uptime-reports)
- [Uptime Kuma](kb:apps/uptime-kuma)
- [Reach your servers away from home](kb:howto/remote-access)
