# RomM retro game library

A web library for the ROMs you've dumped from your own cartridges and discs: it scans folders per console, pulls cover art and descriptions, and plays many systems right in the browser with EmulatorJS. Handhelds and emulation frontends can download from it too.

## What you end up with

- RomM with MariaDB, scanning a `roms` folder sorted by platform
- Covers and metadata from IGDB and ScreenScraper
- In-browser play for supported consoles, and saves kept per user

## Before you start

- Your own game dumps and BIOS files. Downloading ROMs of games you don't own is illegal in most countries.
- Optional but recommended: a free Twitch developer app (client ID and secret) for IGDB metadata, and a ScreenScraper account.

## Part 1 · Folder layout

- [ ] One folder per platform, named with RomM's platform slugs:

```text
/srv/games/library/
  roms/
    gba/
    snes/
    n64/
    psx/
  bios/
    psx/
```

- [ ] Multi-disc games go in a subfolder per game. The RomM docs list the slug for every platform.

## Part 2 · The stack

- [ ] Create a secret with `openssl rand -hex 32`. [Docker](view:docker) → **+ New stack** named `romm`, with the secret and passwords in `.env`:

```yaml
services:
  romm:
    image: rommapp/romm:latest
    restart: unless-stopped
    depends_on: [db]
    environment:
      DB_HOST: db
      DB_NAME: romm
      DB_USER: romm
      DB_PASSWD: ${DB_PASSWORD}
      ROMM_AUTH_SECRET_KEY: ${ROMM_SECRET}
      IGDB_CLIENT_ID: ${IGDB_ID}
      IGDB_CLIENT_SECRET: ${IGDB_SECRET}
      SCREENSCRAPER_USER: ${SS_USER}
      SCREENSCRAPER_PASSWORD: ${SS_PASSWORD}
    volumes:
      - /srv/games/library:/romm/library
      - ./resources:/romm/resources
      - ./assets:/romm/assets
      - ./config:/romm/config
    ports:
      - "8080:8080"

  db:
    image: mariadb:11
    restart: unless-stopped
    environment:
      MARIADB_ROOT_PASSWORD: ${DB_ROOT_PASSWORD}
      MARIADB_DATABASE: romm
      MARIADB_USER: romm
      MARIADB_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/mysql
```

- [ ] Open port 8080 and create the admin account in the setup wizard.

## Part 3 · Scan and play

- [ ] **Scan** → pick platforms and metadata sources → **Scan**. The first scan of a large library takes a while.
- [ ] Unmatched games: open the game → **Match** and search by title.
- [ ] Supported platforms show a **Play** button that runs the game in the browser; BIOS files from `bios/<platform>` are used automatically.
- [ ] Add users for the household under **Administration**, with viewer or editor roles.

#### Check

- [ ] A GBA game shows its cover and plays in the browser with a controller connected.

## In Nightgrid

- **⧉** on the container's row in [Docker](view:docker) opens the library in a Nightgrid window; add it to [Services](view:services) for an uptime check.
- `assets` holds uploaded saves and states: include it in backups with the database. The ROM folder itself only needs backing up if you can't re-dump it.

## Limits and gotchas

- Browser play suits 8-bit to PS1-era consoles; heavier systems need a real emulator.
- Folder names must match the platform slugs or games land in the wrong platform.
- Metadata sources rate-limit; big first scans may need a second run.

## Related

- [Game server host](kb:guides/game-server-host)
- [Samba file shares](kb:apps/samba)
- [Kavita comics and manga reader](kb:apps/kavita)
