# Navidrome music server

A light, fast music streaming server compatible with Subsonic apps, for your own FLAC and MP3 collection.

## What you end up with

- Your music library streamed to any Subsonic-compatible app (Symfonium, Feishin, Substreamer, play:Sub)
- Transcoding on the fly for mobile data

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `navidrome`:

```yaml
services:
  navidrome:
    image: deluan/navidrome:latest
    restart: unless-stopped
    user: 1000:1000
    environment:
      ND_SCANSCHEDULE: 1h
      ND_LOGLEVEL: info
      ND_BASEURL: ""
    volumes:
      - ./data:/data
      - /srv/media/music:/music:ro
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `music.example.com` to `navidrome:4533` and create the admin account on first visit.

## Part 2 · Clients

- [ ] Use the server URL and your login in any Subsonic app.
- [ ] Clean tags (MusicBrainz Picard or beets) make albums and artists group properly.

#### Check

- [ ] An album plays on the phone, and new files appear after the next scan.

## Limits and gotchas

- Back up `./data`; it holds playlists, users and play counts.

## Related

- [Audiobookshelf](kb:apps/audiobookshelf)
- [Jellyfin media server](kb:apps/jellyfin)
