# PhotoPrism photo library

Browse and search a photo library you already have on disk, by place, face, object and date, without moving the files. A good fit when your photos live on a NAS and you want them indexed, not imported.

## What you end up with

- Your existing photo folders indexed in place (read-only if you like)
- Search by people, places, labels and colours
- Albums and share links

## Before you start

- At least 4 GB of RAM (indexing and face recognition are hungry) and swap
- The photo folder mounted on the host, for example `/srv/photos`

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `photoprism`, with the passwords in `.env`:

```yaml
services:
  photoprism:
    image: photoprism/photoprism:latest
    restart: unless-stopped
    depends_on: [mariadb]
    ports:
      - "2342:2342"
    environment:
      PHOTOPRISM_ADMIN_USER: admin
      PHOTOPRISM_ADMIN_PASSWORD: ${ADMIN_PASSWORD}
      PHOTOPRISM_SITE_URL: http://photos.lan:2342/
      PHOTOPRISM_DATABASE_DRIVER: mysql
      PHOTOPRISM_DATABASE_SERVER: mariadb:3306
      PHOTOPRISM_DATABASE_NAME: photoprism
      PHOTOPRISM_DATABASE_USER: photoprism
      PHOTOPRISM_DATABASE_PASSWORD: ${DB_PASSWORD}
      PHOTOPRISM_READONLY: "true"
    volumes:
      - /srv/photos:/photoprism/originals:ro
      - ./storage:/photoprism/storage
  mariadb:
    image: mariadb:11
    restart: unless-stopped
    command: --innodb-buffer-pool-size=512M --transaction-isolation=READ-COMMITTED --character-set-server=utf8mb4 --collation-server=utf8mb4_unicode_ci
    environment:
      MARIADB_DATABASE: photoprism
      MARIADB_USER: photoprism
      MARIADB_PASSWORD: ${DB_PASSWORD}
      MARIADB_RANDOM_ROOT_PASSWORD: "1"
    volumes:
      - ./database:/var/lib/mysql
```

- [ ] `PHOTOPRISM_READONLY` with a `:ro` mount means PhotoPrism can never change your originals. Drop both if you want uploads.

## Part 2 · Index

- [ ] Log in, then Library → Index → **Start**. The first run takes hours for a big library; thumbnails go into `./storage`.
- [ ] Faces: People → assign names to the clusters it found; later photos are matched automatically.

#### Check

- [ ] Search for a place or a person you named: matching photos appear, and the originals folder is unchanged.

## In Nightgrid

- Watch the indexing load on [Overview](view:overview); `./storage` can reach a large fraction of the library's size, which [Fleet](view:fleet) → **Disks** forecasts.
- Back up the database with a dump and `./storage` with the stack; the originals have their own backup.

## Limits and gotchas

- Hardware video transcoding needs extra device mappings; without it, video previews use the CPU.
- Some features (like extra map styles) are in the paid tiers; browsing, search, faces and albums are free.
- For phone backup and a Google Photos style app, [Immich](kb:apps/immich) fits better; PhotoPrism shines at indexing an existing archive.

## Related

- [Immich photo backup](kb:apps/immich)
- [Family cloud and photos](kb:guides/family-cloud-photos)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
