# Wallabag read-it-later

Save articles to read later, stripped down to the text and images, on your own server. Browser extensions and phone apps send pages to it, and the apps download articles for offline reading.

## What you end up with

- Wallabag with its built-in SQLite database behind your reverse proxy
- The browser extension and phone app connected
- Public registration off, so only your accounts exist

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `wallabag`:

```yaml
services:
  wallabag:
    image: wallabag/wallabag:latest
    restart: unless-stopped
    environment:
      SYMFONY__ENV__DOMAIN_NAME: https://read.example.com
      SYMFONY__ENV__FOSUSER_REGISTRATION: "false"
      SYMFONY__ENV__SERVER_NAME: "Wallabag"
    volumes:
      - ./data:/var/www/wallabag/data
      - ./images:/var/www/wallabag/web/assets/images
    ports:
      - "127.0.0.1:8085:80"
```

- [ ] Point the reverse proxy for `read.example.com` at `127.0.0.1:8085`. `SYMFONY__ENV__DOMAIN_NAME` must be the exact public address or styles and images break.
- [ ] Log in with `wallabag` / `wallabag`, then change the password under **Config → User information** and **Password**. Add your own user under **Users management** if you prefer a different name.

## Part 2 · Clients

- [ ] Install the wallabagger extension for Firefox or Chrome, or the official wallabag app on Android and iOS.
- [ ] Apps need an API client: **API clients management → Create a new client**, then give the app the client ID, secret, your username and password.
- [ ] Older saved lists can come in under **Import**, which reads exports from several other services and from another wallabag.

#### Check

- [ ] Saving a page from the browser extension shows it in the web UI within seconds, and the phone app shows it after a sync.

## In Nightgrid

- **⧉** in [Docker](view:docker) opens it in a window; add the site to [Services](view:services).
- Back up the `data` folder (the SQLite database) and `images` together, ideally with the stack stopped, and watch the job in [Fleet](view:fleet) → **Backups**.

## Limits and gotchas

- Some sites block fetching or put articles behind scripts; wallabag then saves only a teaser. The browser extension can send the page as you see it, which helps.
- SQLite is fine for one or a few users. For more, wallabag also supports PostgreSQL and MariaDB via the `SYMFONY__ENV__DATABASE_*` variables.
- Large imports are slow without the optional Redis or RabbitMQ worker; import in chunks.

## Related

- [Karakeep bookmarks](kb:apps/karakeep)
- [Miniflux feed reader](kb:apps/miniflux)
- [linkding bookmarks](kb:apps/linkding)
