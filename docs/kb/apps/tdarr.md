# Tdarr library transcoding

Re-encode a media library automatically (for example H.264 to HEVC or AV1 to save space), with workers on any machine that has a GPU.

![A Tdarr server with worker nodes.](fig:app-tdarr)

## What you end up with

- A Tdarr server scanning your library
- A node using the GPU for transcodes, with health checks of every file

## Before you start

- Back up or test on a copy first: transcoding replaces files.
- A GPU: Intel Quick Sync, NVIDIA NVENC or AMD VAAPI.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `tdarr`:

```yaml
services:
  tdarr:
    image: ghcr.io/haveagitgat/tdarr:latest
    restart: unless-stopped
    environment:
      TZ: Etc/UTC
      PUID: 1000
      PGID: 1000
      serverIP: 0.0.0.0
      serverPort: 8266
      webUIPort: 8265
      internalNode: "true"
      nodeName: main
    ports:
      - "127.0.0.1:8265:8265"
      - "8266:8266"
    volumes:
      - ./server:/app/server
      - ./configs:/app/configs
      - ./logs:/app/logs
      - /srv/media:/media
      - /srv/tdarr-cache:/temp
    devices:
      - /dev/dri:/dev/dri
```

- [ ] Open the UI through a saved tunnel in [Tunnels](view:tunnels).

## Part 2 · Flows

- [ ] Add a library pointing at `/media/tv` with the transcode cache `/temp`.
- [ ] Build a flow: skip files already in the target codec, transcode with the hardware encoder, replace original only if smaller.
- [ ] Start with a small folder and check results by eye.

## Part 3 · In Nightgrid

- [ ] The host monitor shows GPU load; [Fleet](view:fleet) → **Disks** shows space coming back.
- [ ] Set the cache on fast local disk and watch it in the disk forecast.

#### Check

- [ ] Transcoded files play in Jellyfin or Plex with the expected codec.

## Limits and gotchas

- Hardware encoders produce larger files than CPU encoders at the same quality; that's the speed trade-off.
- Pause Tdarr during scrubs and backups; both hammer the disks.

## Related

- [Jellyfin media server](kb:apps/jellyfin)
- [The *arr stack](kb:apps/arr-stack)
