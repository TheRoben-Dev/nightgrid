# MeTube video downloader

A web front end for yt-dlp: paste a link from YouTube or any of the hundreds of sites yt-dlp supports, pick quality and format, and the file lands in a folder on the server, ready for Jellyfin or Plex. A browser extension and an iOS shortcut send links straight to it.

## What you end up with

- MeTube on the LAN, saving into your media share
- Audio-only and video presets
- Playlists and channels downloaded in one go

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `metube`:

```yaml
services:
  metube:
    image: ghcr.io/alexta69/metube:latest
    restart: unless-stopped
    environment:
      UID: "1000"
      GID: "1000"
      DOWNLOAD_DIR: /downloads
      AUDIO_DOWNLOAD_DIR: /downloads/audio
      YTDL_OPTIONS: '{"writesubtitles": true, "subtitleslangs": ["en"], "embedthumbnail": true}'
    volumes:
      - /srv/media/youtube:/downloads
    ports:
      - "8081:8081"
```

- [ ] `UID`/`GID` should match the owner of the media folder so Jellyfin or Plex can read the files.
- [ ] `YTDL_OPTIONS` passes options straight to yt-dlp (subtitles, thumbnails, rate limits, output templates).

## Part 2 · Use it

- [ ] Open port 8081, paste a URL, choose **Quality** and **Format** (MP4 for wide compatibility, M4A/MP3 for audio), **Add**.
- [ ] Playlist or channel links queue every video. **Auto start** off lets you review the queue first.
- [ ] Browser extensions for Chrome and Firefox ("MeTube Downloader") add a right-click "Send to MeTube".

#### Check

- [ ] A downloaded video plays in Jellyfin after a library scan.

## In Nightgrid

- **⧉** on the container's row in [Docker](view:docker) opens it in a window. Downloads land on the host, so [Files](view:files) can move or rename them.
- Keep the image current: sites change often and only a fresh yt-dlp keeps working. [Docker](view:docker) → **Updates** shows when a new image is out.

## Limits and gotchas

- Respect the site's terms and copyright; download what you're allowed to keep.
- Sites break yt-dlp from time to time. "Sign in to confirm" or 403 errors usually mean wait for the next image or add cookies via `YTDL_OPTIONS`.
- For subscriptions that download new videos automatically, look at Tube Archivist or Pinchflat instead.

## Related

- [Tube Archivist video archive](kb:apps/tubearchivist)
- [Jellyfin media server](kb:apps/jellyfin)
- [The *arr stack](kb:apps/arr-stack)
