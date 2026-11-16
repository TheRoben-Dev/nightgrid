# BookStack wiki

A tidy wiki organised as shelves, books, chapters and pages, for household manuals, homelab documentation or a small team.

## What you end up with

- BookStack at `wiki.example.com` with MariaDB
- WYSIWYG or Markdown editing, search and page revisions

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `bookstack`:

```yaml
services:
  db:
    image: mariadb:11
    restart: unless-stopped
    environment:
      MARIADB_ROOT_PASSWORD: ${DB_ROOT_PASSWORD}
      MARIADB_DATABASE: bookstack
      MARIADB_USER: bookstack
      MARIADB_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/mysql
  bookstack:
    image: lscr.io/linuxserver/bookstack:latest
    restart: unless-stopped
    depends_on: [db]
    environment:
      PUID: 1000
      PGID: 1000
      APP_URL: https://wiki.example.com
      APP_KEY: ${APP_KEY}
      DB_HOST: db
      DB_DATABASE: bookstack
      DB_USERNAME: bookstack
      DB_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./config:/config
    networks: [default, proxy]
networks:
  proxy:
    external: true
```

- [ ] Generate `APP_KEY`: `docker run -it --rm --entrypoint /bin/bash lscr.io/linuxserver/bookstack:latest appkey`. Put it and the passwords in the **Vault**.
- [ ] Route `wiki.example.com` to `bookstack:80`. Log in with the default admin from the image's docs and change it immediately.

## Part 2 · Nightgrid's Knowledge vs BookStack

- [ ] Nightgrid's [Knowledge](view:notes) is for runbooks tied to hosts and alerts, with **Send to terminal**. BookStack suits longer documentation others read in a browser. Link between them freely.

#### Check

- [ ] A page saves and appears in search.

## Limits and gotchas

- Back up with a database dump plus `./config` (uploads live there).

## Related

- [Write runbooks for alerts](kb:howto/runbooks)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
