# Add app cards: Plex, Jellyfin, the *arr apps and more

Bring other apps' live status into Nightgrid's Services board: what's playing on Plex, Jellyfin or Emby, the Sonarr and Radarr download queues, pending Overseerr requests, game servers in AMP, Uptime Kuma monitors and links into Portainer. Keys and passwords stay in the OS keychain.

![Integrations feeding the Services board.](fig:ng-integrations)

## What you end up with

- One card per app on the [Services](view:services) board with its health, a summary line, counts and a list (now playing, queue, pending requests)
- The app's everyday buttons on the card: scan libraries, RSS sync, search missing, approve a request
- Anything that changes or removes something asked first, in a dialog that says what will happen

## Before you start

- The app running and reachable from this computer by its address (a LAN IP, a tailnet name or a reverse-proxy name).
- Its API key or login. Each type's help text in the editor says where the app keeps it.

## Part 1 · Add an integration

- [ ] [Services](view:services) → **Add integration**. The **NEW INTEGRATION** dialog groups types under Media, Media automation, Network, Monitoring, Docker and Games; **Find an app…** filters them.
- [ ] Pick the app. Fill in **URL** (the placeholder shows its usual port) and the **API key**, **Password** or token it asks for. **Name** is optional.
- [ ] Self-signed or internal certificate? Tick **Allow untrusted certificate**.
- [ ] **Test** reads the app once without saving; **Add** saves it.

## Part 2 · Media servers

- [ ] **Plex**: the Plex token. Open any item in Plex Web, **…** → **Get Info** → **View XML**, and copy the `X-Plex-Token` value from the address. Shows now playing, transcodes and the server version. Server setup: [Plex Media Server](kb:apps/plex).
- [ ] **Jellyfin** (Dashboard → API Keys → **+**) and **Emby** (Settings → Advanced → API Keys): now playing, transcodes, library counts and **Scan libraries**. See [Jellyfin](kb:apps/jellyfin).
- [ ] **Tautulli** (Settings → Web Interface → API Key): Plex activity with quality, transcode decisions and bandwidth. Read-only.

## Part 3 · Media automation

- [ ] **Sonarr**, **Radarr** and **Lidarr** (each app's Settings → General → API Key): the download queue with progress, missing items and health warnings. **RSS sync** runs at once; **Search missing** asks first, because it searches every missing episode, movie or album and uses indexer API calls. **Remove** on a queue row stops the app tracking that download (the download client keeps it).
- [ ] **Prowlarr**: indexers, which ones are failing, and **Test indexers**.
- [ ] **Bazarr**: missing subtitles for episodes and movies, throttled providers and health issues.
- [ ] **Overseerr** and **Jellyseerr** (Settings → General → API Key): request counts and pending requests with **Approve** and **Decline** (decline asks first).
- [ ] Setting the stack up: [The *arr stack](kb:apps/arr-stack) and [Jellyseerr](kb:apps/jellyseerr).

## Part 4 · Games, monitoring and Docker

- [ ] **AMP**: the panel address, **Login name** and password of an AMP user allowed to manage instances. The card lists instances with players, and start, stop and restart (stop and restart confirm by name). More in [Game server host](kb:guides/game-server-host).
- [ ] **Uptime Kuma**: no key needed. Make a status page in Kuma and enter its **Status page slug**. The card lists that page's monitors. See [Uptime Kuma](kb:apps/uptime-kuma).
- [ ] **Portainer**: pick the **Docker host** it manages, for "Open in Portainer" links on that host's containers (see [Portainer alongside Nightgrid](kb:apps/portainer)).
- [ ] **Pi-hole** and **AdGuard Home** have their own guide: [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard).

#### Check

- [ ] Each card shows live data and a health state, not an error.

## Limits and gotchas

- **401** or **Unauthorized**: the key was copied with a space, or it was regenerated in the app. **…** on the card edits it: paste the key again and **Save**. **↻** reads it again; the app's name opens its web UI in your browser.
- **Certificate** errors with an internal CA: tick **Allow untrusted certificate** in the editor.
- Cards refresh while Services is open; they don't raise alerts. Add a plain service check on the same URL for alerts and uptime history (see [Watch services and expiry dates](kb:howto/service-checks)).
- Keys and passwords are included in profile exports only when you tick **Include saved passwords and tokens**.

## Related

- [Watch services and expiry dates](kb:howto/service-checks)
- [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
