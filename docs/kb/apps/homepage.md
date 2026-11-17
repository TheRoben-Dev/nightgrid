# Homepage dashboard

A fast start page for your homelab with tiles for every app, live widgets, and a `services.yaml` that Nightgrid can import as service checks.

## What you end up with

- One page with all your apps, grouped, with status and widgets
- The same list in Nightgrid's Services board, imported in one click

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `homepage`:

```yaml
services:
  homepage:
    image: ghcr.io/gethomepage/homepage:latest
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      HOMEPAGE_ALLOWED_HOSTS: home.example.lan:3000,home.example.com
    volumes:
      - ./config:/app/config
```

- [ ] `HOMEPAGE_ALLOWED_HOSTS` must list every host name (and port) you open it with, or it refuses the request.

## Part 2 · Services

- [ ] Edit `./config/services.yaml` in [Files](view:files):

```yaml
- Media:
    - Jellyfin:
        href: https://jellyfin.example.com
        description: Movies and TV
        icon: jellyfin.png
- Infrastructure:
    - Pi-hole:
        href: https://dns.example.com/admin
        icon: pi-hole.png
```

- [ ] Widgets (API stats per app) go under each service with `widget:`; their keys belong in environment variables (`{{HOMEPAGE_VAR_...}}`), not in the file.

## Part 3 · Import into Nightgrid

- [ ] [Services](view:services) → **Import Homepage**, point it at the config folder or `services.yaml` on the host. Nightgrid adds a check per service with its group.
- [ ] Re-import after adding apps; existing services are kept.

#### Check

- [ ] The Services board shows the same groups as Homepage, with checks running.

## Limits and gotchas

- Homepage's Docker integration needs the Docker socket; mount it read-only or through a socket proxy.
- Homepage's own status dots and Nightgrid's checks are independent; Nightgrid adds history and alerts.

## Related

- [Watch services and expiry dates](kb:howto/service-checks)
- [Uptime Kuma](kb:apps/uptime-kuma)
