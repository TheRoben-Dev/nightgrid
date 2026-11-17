# Kavita comics and manga reader

A fast reader for comics, manga and e-books with series metadata, reading progress and OPDS.

![Kavita serving your books and comics.](fig:app-media-server)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `kavita`:

```yaml
services:
  kavita:
    image: jvmilazz0/kavita:latest
    restart: unless-stopped
    volumes:
      - /srv/media/comics:/comics:ro
      - /srv/media/manga:/manga:ro
      - ./config:/kavita/config
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `comics.example.com` to `kavita:5000`, create the admin, add libraries.

## Part 2 · Layout

- [ ] One folder per series, files named with volume and chapter numbers (`Series Name Vol.01 Ch.001.cbz`). Kavita's docs describe the naming it understands.

#### Check

- [ ] A series shows with covers and opens in the reader.

## Limits and gotchas

- Back up `./config` (database and progress).

## Related

- [Calibre-Web e-book library](kb:apps/calibre-web)
- [Audiobookshelf](kb:apps/audiobookshelf)
