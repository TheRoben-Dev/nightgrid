# SearXNG private search

A metasearch engine that queries many search engines for you without profiling, usable as your browser's default search.

![SearXNG querying engines on your behalf.](fig:app-searxng)

## What you end up with

- Search at `search.example.com` with results from several engines
- No tracking, no ads, and your choice of engines

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `searxng`:

```yaml
services:
  searxng:
    image: searxng/searxng:latest
    restart: unless-stopped
    environment:
      SEARXNG_BASE_URL: https://search.example.com/
    volumes:
      - ./config:/etc/searxng
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Start once to generate `./config/settings.yml`, then set a random `secret_key` in it (`openssl rand -hex 32`) and restart.
- [ ] Route `search.example.com` to `searxng:8080`.

## Part 2 · Use it

- [ ] Add it as a search engine in your browser (most browsers offer this after one search).
- [ ] Preferences → engines: turn off ones you don't want.

#### Check

- [ ] A search returns results with engine names beside them.

## Limits and gotchas

- Public instances get rate-limited by the engines; keep yours private (tailnet or a login in front, see [Authelia single sign-on](kb:apps/authelia)).
- The optional limiter needs a Valkey/Redis container; it's only useful for public instances.

## Related

- [FreshRSS feed reader](kb:apps/freshrss)
- [Authelia single sign-on](kb:apps/authelia)
