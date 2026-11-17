# Komga comics and manga server

A media server for comics, manga and graphic novels (CBZ, CBR, PDF, EPUB): series, read progress, a web reader, and OPDS for reader apps on phones and tablets.

## What you end up with

- Your comics organised into libraries and series with covers
- Read progress kept per user across devices
- OPDS and the Komga API for apps like Paperback, Tachiyomi forks or Panels

## Part 1 · The stack

- [ ] Comics in folders per series, for example `/srv/comics/Series Name/Series Name 001.cbz`.
- [ ] [Docker](view:docker) → **+ New stack** named `komga`:

```yaml
services:
  komga:
    image: gotson/komga:latest
    restart: unless-stopped
    user: "1000:1000"
    environment:
      TZ: Etc/UTC
    volumes:
      - ./config:/config
      - /srv/comics:/data:ro
    ports:
      - "25600:25600"
```

## Part 2 · Set up

- [ ] Open port 25600, create the admin account.
- [ ] Libraries → **+** → root folder `/data`. Scanning reads metadata from `ComicInfo.xml` inside the archives when present.
- [ ] Add users with access to the libraries they should see; age ratings can restrict content.

#### Check

- [ ] Open a book in the web reader, read a few pages, open it on another device: it resumes where you left off.

## In Nightgrid

- The library is read-only to Komga (`:ro`), so add and rename files with [Files](view:files) and run a library scan after.
- Add it to [Services](view:services) for uptime and certificate checks.

## Limits and gotchas

- Run it with a JVM memory limit on small hosts (`JAVA_TOOL_OPTIONS: -Xmx1g`).
- For ebooks and audiobooks, Kavita or Audiobookshelf may fit better; Komga is strongest for comics.

## Related

- [Kavita comics and manga reader](kb:apps/kavita)
- [Calibre-Web e-book library](kb:apps/calibre-web)
- [Audiobookshelf](kb:apps/audiobookshelf)
