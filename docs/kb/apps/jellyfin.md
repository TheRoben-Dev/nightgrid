# Jellyfin media server

Stream your own movies, shows and music to TVs, phones and browsers, with hardware transcoding on an Intel or AMD GPU. Free and open source, no account required.

## What you end up with

- Jellyfin with your libraries organised and artwork fetched
- Hardware transcoding through `/dev/dri`
- Remote viewing over your tailnet, without exposing it publicly

## Before you start

- Media organised in folders like `/srv/media/movies/Title (Year)/Title (Year).mkv` and `/srv/media/tv/Show/Season 01/`.
- For transcoding: an Intel CPU with Quick Sync or an AMD GPU, and the host's `render` group id (`getent group render | cut -d: -f3`).

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `jellyfin`:

```yaml
services:
  jellyfin:
    image: jellyfin/jellyfin:latest
    restart: unless-stopped
    user: 1000:1000
    group_add:
      - "993"
    ports:
      - "8096:8096"
    volumes:
      - ./config:/config
      - ./cache:/cache
      - /srv/media:/media:ro
    devices:
      - /dev/dri:/dev/dri
```

- [ ] Replace `993` with your `render` group id.
- [ ] Open port 8096 on the host and run the setup wizard: add libraries pointing at `/media/movies` and `/media/tv`.

## Part 2 · Transcoding

- [ ] Dashboard → Playback → Transcoding: **Intel QuickSync (QSV)** or **VAAPI** for Intel, **VAAPI** for AMD. Device `/dev/dri/renderD128`.
- [ ] Tick the codecs your GPU decodes (HEVC, VP9, AV1 on newer chips) and enable hardware encoding.
- [ ] Play something in a browser at a lower quality and check the dashboard says "Transcoding (hardware)".

## Part 3 · Remote access and Nightgrid

- [ ] Away from home, use the tailnet. Jellyfin clients connect to the host's tailnet or LAN address (with a subnet router).
- [ ] [Services](view:services) → **Add service** with port 8096's health path `/health` on the host's address.
- [ ] The monitor shows GPU busy % for AMD and NVIDIA GPUs while transcoding.

#### Check

- [ ] A phone on mobile data plays a film over the tailnet.

## Limits and gotchas

- Intel low-power encoding needs the GuC/HuC firmware on some generations; Jellyfin's docs list which.
- Storing `./config` on a network share makes Jellyfin's database slow and fragile; keep it on local disk.
- Public exposure through Cloudflare Tunnel breaks its terms for video; use a tailnet.

## Related

- [Plex Media Server](kb:apps/plex)
- [The *arr stack](kb:apps/arr-stack)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
