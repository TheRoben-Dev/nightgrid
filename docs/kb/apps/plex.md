# Plex Media Server

Plex in Docker with hardware transcoding, its apps on every TV, and a Nightgrid card showing what's playing.

## What you end up with

- Plex claimed to your account, libraries scanned
- Hardware transcoding (with Plex Pass) on an Intel iGPU or NVIDIA card
- Now playing on the Services board

## Part 1 · The stack

- [ ] Get a claim token (valid for a few minutes) from Plex's claim page while logged in.
- [ ] [Docker](view:docker) → **+ New stack** named `plex`:

```yaml
services:
  plex:
    image: plexinc/pms-docker:latest
    restart: unless-stopped
    network_mode: host
    environment:
      TZ: Etc/UTC
      PLEX_CLAIM: ${PLEX_CLAIM}
      PLEX_UID: 1000
      PLEX_GID: 1000
    volumes:
      - ./config:/config
      - ./transcode:/transcode
      - /srv/media:/data:ro
    devices:
      - /dev/dri:/dev/dri
```

- [ ] Start it within the claim token's lifetime, then open port 32400 on the host at `/web` and add libraries.
- [ ] Host networking lets local clients discover the server; without it, set **Custom server access URLs** in Plex's network settings.

## Part 2 · Transcoding

- [ ] Settings → Transcoder → **Use hardware acceleration when available** (requires Plex Pass).
- [ ] NVIDIA: install the NVIDIA container toolkit on the host and add `runtime: nvidia` plus `NVIDIA_VISIBLE_DEVICES: all` and `NVIDIA_DRIVER_CAPABILITIES: all`.

## Part 3 · In Nightgrid

- [ ] [Services](view:services) → **Add integration** → **Plex** with the server address and a token. The token is in any item's "View XML" link (`X-Plex-Token`); it goes to the keychain.
- [ ] The card shows current streams.

#### Check

- [ ] Plex's dashboard shows "(hw)" on a transcoding stream.

## Limits and gotchas

- Remote streaming away from home now needs Plex Pass or a Remote Watch Pass for most clients.
- Don't put `./config` on a network share; Plex's database is SQLite.

## Related

- [Jellyfin media server](kb:apps/jellyfin)
- [The *arr stack](kb:apps/arr-stack)
- [Proxmox media server](kb:guides/proxmox-media-server)
