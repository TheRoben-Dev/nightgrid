# Calibre-Web e-book library

Browse, read and send e-books from a Calibre library in the browser, with OPDS for e-reader apps and send-to-Kindle by email.

## What you end up with

- Your e-book library at `books.example.com` with users and shelves
- OPDS feed for KOReader, Moon+ Reader and similar
- Optional conversion and send-to-device by email

## Before you start

- A Calibre library (a folder with `metadata.db`). Make one in the Calibre desktop app, or start empty.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `calibre-web`:

```yaml
services:
  calibre-web:
    image: lscr.io/linuxserver/calibre-web:latest
    restart: unless-stopped
    environment:
      PUID: 1000
      PGID: 1000
      TZ: Etc/UTC
      DOCKER_MODS: linuxserver/mods:universal-calibre
    volumes:
      - ./config:/config
      - /srv/media/books:/books
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] The `universal-calibre` mod adds conversion; leave it out if you don't need it (smaller and faster to start).
- [ ] Route the site to `calibre-web:8083`. Log in with the default admin from the image's docs, change the password, and set the library path to `/books`.

## Part 2 · Use it

- [ ] Admin → Edit basic configuration → enable uploads if you want to add books from the browser.
- [ ] E-reader apps: OPDS at `/opds` on your site with your login.
- [ ] Email settings (see [Send mail from your servers](kb:apps/mail-relay)) enable "Send to e-reader".

#### Check

- [ ] A book opens in the web reader and appears in an OPDS client.

## Limits and gotchas

- Don't edit the library with the Calibre desktop app and Calibre-Web at the same time; `metadata.db` doesn't like two writers.

## Related

- [Audiobookshelf](kb:apps/audiobookshelf)
- [Paperless-ngx documents](kb:apps/paperless-ngx)
