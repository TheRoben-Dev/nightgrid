# Music Assistant

One music library from many sources (local files, Jellyfin, Plex, Subsonic servers, streaming services) played on every speaker in the house: Sonos, Chromecast, AirPlay, DLNA, Squeezebox and Home Assistant media players.

## What you end up with

- All your music sources merged into one library
- Playback to any speaker, grouped or one at a time
- Players in Home Assistant for automations ("play the morning playlist in the kitchen")

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `music-assistant`:

```yaml
services:
  music-assistant:
    image: ghcr.io/music-assistant/server:latest
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./data:/data
      - /srv/music:/media:ro
    cap_add: [SYS_ADMIN, DAC_READ_SEARCH]
    security_opt: [apparmor:unconfined]
```

- [ ] `network_mode: host` is required: speakers are found by mDNS and UPnP broadcasts. The web UI is on port 8095.
- [ ] The extra capabilities are only needed for mounting SMB or NFS music shares from inside the app; drop them if the music is a local folder.

## Part 2 · Providers

- [ ] Settings → Music providers: **Filesystem (local disk)** with path `/media`, plus Jellyfin, Plex or Subsonic with their URLs and tokens.
- [ ] Settings → Player providers: enable the speaker types you have (Chromecast, AirPlay, Sonos, DLNA, Squeezelite).

## Part 3 · Home Assistant

- [ ] In Home Assistant: Add integration → **Music Assistant**, with the server's address. Each speaker appears as a media player.

#### Check

- [ ] Play an album to two speakers grouped: both start in sync.

## In Nightgrid

- Watch the container in [Docker](view:docker); library scans of large collections show as CPU and disk load on [Overview](view:overview).
- The music folder is read-only to it; manage files in [Files](view:files) or your *arr stack.

## Limits and gotchas

- Host networking means it can't share a port with anything else on the host (8095, plus streaming ports).
- Speakers on another VLAN need mDNS forwarding between VLANs, or they won't be found.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Navidrome music server](kb:apps/navidrome)
- [Smart home hub](kb:guides/smart-home-hub)
