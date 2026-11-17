# FreshRSS feed reader

Follow blogs, news and release feeds in one self-hosted reader that syncs with mobile apps.

## What you end up with

- FreshRSS at `rss.example.com` refreshing feeds every 20 minutes
- Mobile and desktop apps through the Google Reader or Fever API

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `freshrss`:

```yaml
services:
  freshrss:
    image: freshrss/freshrss:latest
    restart: unless-stopped
    environment:
      TZ: Etc/UTC
      CRON_MIN: "1,21,41"
    volumes:
      - ./data:/var/www/FreshRSS/data
      - ./extensions:/var/www/FreshRSS/extensions
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `rss.example.com` to `freshrss:80` and finish the web installer (SQLite is fine for one person).

## Part 2 · Apps

- [ ] Settings → Authentication: allow API access. Profile → set an API password.
- [ ] Apps such as Reeder, NetNewsWire, ReadYou or FeedMe connect with the Google Reader API URL shown on the profile page.

## Part 3 · Release feeds for your homelab

- [ ] Subscribe to the releases of what you run: GitHub projects publish an Atom feed at `/releases.atom` on the project's page. That's an easy "what changed" list next to Nightgrid's image update checks.

#### Check

- [ ] Feeds refresh on their own, and the mobile app shows unread counts.

## Limits and gotchas

- Back up `./data`; it holds the database and users.

## Related

- [Update containers without fear](kb:howto/container-updates)
- [SearXNG private search](kb:apps/searxng)
