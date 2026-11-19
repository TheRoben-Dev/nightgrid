# Tautulli Plex statistics

History and stats for a Plex server: who watched what, when, on which device, and whether it transcoded. Plus notifications for new media and playback events.

![Tautulli keeping Plex's watch history.](fig:app-watch-stats)

## What you end up with

- Watch history per user and per title
- Transcode vs direct play reasons, to fix clients that make the server work hard
- Newsletters and notifications (optional)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `tautulli`, on any host that can reach Plex:

```yaml
services:
  tautulli:
    image: lscr.io/linuxserver/tautulli:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
    volumes:
      - ./config:/config
    ports:
      - "8181:8181"
```

## Part 2 · Connect to Plex

- [ ] Open port 8181; the setup wizard signs in with your Plex account and finds the server. Use the server's LAN address, not the plex.direct name, if it's on the same network.
- [ ] Settings → Web Interface: set a username and password.

## Part 3 · Find transcodes

- [ ] History → filter "Transcode": the stream info shows why (audio codec, subtitles burned in, bandwidth limit).
- [ ] Image-based subtitles (PGS) and unsupported audio on a TV app are the usual causes; change the client setting or add a compatible audio track.

#### Check

- [ ] Play something on any client: Activity on Tautulli's home page shows it live.

## In Nightgrid

- [Services](view:services) → **Add integration** → **Tautulli** with its API key (Settings → Web Interface → API Key) shows Plex activity with quality, transcode decisions and bandwidth on a card; Tautulli keeps the history behind it. **Plex** as an integration shows what's playing without Tautulli.
- A transcode storm shows up as CPU or GPU load on the Plex host in [Overview](view:overview); Tautulli tells you which stream caused it.

## Limits and gotchas

- History starts when Tautulli is installed; it can import some older history from Plex's own database (Settings → Import & Backups).
- Notifications go to outside services you configure; nothing is sent by default.

## Related

- [Plex Media Server](kb:apps/plex)
- [Jellyfin media server](kb:apps/jellyfin)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
