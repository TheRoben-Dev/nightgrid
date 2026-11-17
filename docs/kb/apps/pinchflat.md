# Pinchflat YouTube channel archiver

Subscribe to YouTube channels and playlists on your server: Pinchflat checks them on a schedule, downloads new videos with yt-dlp in the quality you choose, and names and tags them so Plex or Jellyfin show them as proper TV-style series.

## What you end up with

- Pinchflat with media profiles for Jellyfin/Plex naming
- Channels and playlists as sources, checked automatically
- Old episodes cleaned up after a number of days, if you want

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `pinchflat`:

```yaml
services:
  pinchflat:
    image: ghcr.io/kieraneglin/pinchflat:latest
    restart: unless-stopped
    environment:
      TZ: Etc/UTC
    volumes:
      - ./config:/config
      - /srv/media/youtube:/downloads
    ports:
      - "8945:8945"
```

- [ ] The download folder should be readable by your media server; match ownership with the user the container runs as (`PUID`/`PGID` or `user:` as the image docs describe).

## Part 2 · Media profile

- [ ] **Media Profiles → New**: pick the **Jellyfin** or **Plex** output template preset, resolution (1080p is a good default), and whether to embed subtitles, chapters and thumbnails.
- [ ] Turn on **NFO files** for Jellyfin so titles, dates and descriptions show up.

## Part 3 · Sources

- [ ] **Sources → New**: paste a channel or playlist URL, pick the media profile, and the **index frequency** (daily is plenty).
- [ ] **Download cutoff date** limits it to recent videos instead of a channel's whole history.
- [ ] **Retention** (days) deletes old videos automatically for news-style channels.

#### Check

- [ ] After the first index, new videos download, and a Jellyfin library pointing at `/srv/media/youtube` shows the channel as a series with seasons by year.

## In Nightgrid

- **⧉** on the container's row in [Docker](view:docker) opens it; add it to [Services](view:services) too.
- [Fleet](view:fleet) → **Disks** forecasts when the media disk fills; whole channels add up fast.

## Limits and gotchas

- Respect creators and the site's terms; this is for watching offline, not redistributing.
- YouTube changes break yt-dlp now and then; updating the image usually fixes it. A cookies file helps with age-restricted videos.
- Indexing very large channels takes a long time the first run; use a cutoff date.

## Related

- [MeTube video downloader](kb:apps/metube)
- [Tube Archivist video archive](kb:apps/tubearchivist)
- [Jellyfin media server](kb:apps/jellyfin)
