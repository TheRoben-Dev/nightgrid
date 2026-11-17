# Homarr dashboard

A drag-and-drop start page for the household: tiles for every app with live status pings, widgets for calendars, weather, media requests and downloads, and boards per user, all edited in the browser with no YAML.

## What you end up with

- Homarr 1.x with its own users and boards
- App tiles that show whether each service is up
- Integrations for Jellyfin/Plex, the *arr apps, Pi-hole or AdGuard and download clients

## Part 1 · The stack

- [ ] Create a key with `openssl rand -hex 32`. [Docker](view:docker) → **+ New stack** named `homarr`, with the key in `.env`:

```yaml
services:
  homarr:
    image: ghcr.io/homarr-labs/homarr:latest
    restart: unless-stopped
    environment:
      SECRET_ENCRYPTION_KEY: ${HOMARR_KEY}
    volumes:
      - ./appdata:/appdata
    ports:
      - "7575:7575"
```

- [ ] `SECRET_ENCRYPTION_KEY` encrypts the API keys you give integrations. Keep it with your other secrets; a new key makes saved integrations unreadable.
- [ ] Docker container tiles need the socket (`/var/run/docker.sock:/var/run/docker.sock:ro`). That's root-equivalent access to the host; leave it out unless you want those tiles.

## Part 2 · Onboarding

- [ ] Open port 7575: the wizard creates the admin user and the first board.
- [ ] **Management → Apps**: add each service with name, icon (searchable icon library) and URL. Status pings check the URL from the Homarr container.
- [ ] **Management → Integrations**: add Jellyfin, Sonarr, Radarr, qBittorrent, Pi-hole and so on with their API keys, then add the matching widgets to the board.
- [ ] Create users or groups for the family and give them their own board, or a read-only view of yours.

#### Check

- [ ] Stop one app's container: its tile turns red within a minute, and green again after you start it.

## In Nightgrid

- Homarr is for the people who use your services; Nightgrid is for running them. Add Homarr itself to [Services](view:services) so you know when the start page is down.
- Back up the `appdata` folder; it holds the database with all boards and integrations.

## Limits and gotchas

- Homarr 1.x is a rewrite. Boards from the old 0.x releases need the built-in import, not a straight volume swap.
- Status pings from inside the container may fail for names only your LAN DNS knows. Use IP addresses or make sure the container uses your local DNS.
- Publishing Homarr to the internet exposes a list of your apps; keep it on the LAN or behind single sign-on.

## Related

- [Homepage dashboard](kb:apps/homepage)
- [The *arr stack](kb:apps/arr-stack)
- [Add Plex, AMP, Uptime Kuma and Portainer cards](kb:howto/integrations)
