# Audiobookshelf

Stream audiobooks and podcasts with progress synced across phones and the web, from your own library.

## What you end up with

- Audiobookshelf at `books.example.com` with libraries for audiobooks and podcasts
- Phone apps that resume where you left off and download for offline listening

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `audiobookshelf`:

```yaml
services:
  audiobookshelf:
    image: ghcr.io/advplyr/audiobookshelf:latest
    restart: unless-stopped
    volumes:
      - /srv/media/audiobooks:/audiobooks
      - /srv/media/podcasts:/podcasts
      - ./config:/config
      - ./metadata:/metadata
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `books.example.com` to `audiobookshelf:80` (websockets must pass through; Caddy does this by default).
- [ ] Create the root user and add libraries.

## Part 2 · Library layout

- [ ] One folder per book: `Author/Series/Book Title/` with the audio files inside. Audiobookshelf reads metadata and can match covers online.

#### Check

- [ ] The phone app streams a book and the web UI shows the same progress.

## Limits and gotchas

- Back up `./config` and `./metadata` (users, progress, covers).

## Related

- [Navidrome music server](kb:apps/navidrome)
- [Jellyfin media server](kb:apps/jellyfin)
