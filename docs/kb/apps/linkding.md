# linkding bookmarks

A fast, minimal bookmark manager: tags, search, read-later, browser extensions, and optional archived snapshots of each page.

## What you end up with

- Bookmarks with tags and notes, searchable from any browser
- Browser extensions for Firefox and Chrome
- Bulk import from your browser's bookmark export

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `linkding`:

```yaml
services:
  linkding:
    image: sissbruecker/linkding:latest
    restart: unless-stopped
    environment:
      LD_SUPERUSER_NAME: admin
      LD_SUPERUSER_PASSWORD: ${ADMIN_PASSWORD}
    volumes:
      - ./data:/etc/linkding/data
    ports:
      - "9090:9090"
```

- [ ] Put `ADMIN_PASSWORD` in the stack's `.env`, deploy, open port 9090 and log in.

## Part 2 · Use it everywhere

- [ ] Settings → Integrations: install the browser extension and paste the API token shown there.
- [ ] Settings → Import: upload the HTML export from your browser.
- [ ] Use `!unread` and tag searches (`#homelab`) in the search box.

#### Check

- [ ] Add a page from the extension; it appears with title and description filled in.

## In Nightgrid

- One small SQLite database in `./data`: back up the stack folder.
- Add it to [Services](view:services); if you publish it through your proxy, use [Manage reverse proxy routes](kb:howto/proxy-routes).

## Limits and gotchas

- The `-plus` image tag adds page snapshots with a headless browser: much bigger, and the snapshots fill disk.
- After the first start you can remove `LD_SUPERUSER_PASSWORD` from `.env`; it's only used to create the user.

## Related

- [Karakeep bookmarks](kb:apps/karakeep)
- [FreshRSS feed reader](kb:apps/freshrss)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
