# Kiwix offline Wikipedia

Serve Wikipedia, Stack Exchange, documentation sets and more from offline ZIM files on your LAN. Useful for travel, cabins, classrooms and outages.

## Part 1 · Get content

- [ ] Download ZIM files from the Kiwix library (Wikipedia comes in sizes from "top articles, no images" at a few hundred MB to the full set at around 100 GB).
- [ ] Put them in `/srv/kiwix`.

## Part 2 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `kiwix`:

```yaml
services:
  kiwix:
    image: ghcr.io/kiwix/kiwix-serve:latest
    restart: unless-stopped
    command: "*.zim"
    volumes:
      - /srv/kiwix:/data:ro
    ports:
      - "8090:8080"
```

- [ ] Open port 8090 on the host.

#### Check

- [ ] The library page lists every ZIM file and search works.

## Limits and gotchas

- Large ZIMs want fast disks for search; keep them on SSD if you can.
- Update by downloading the newer ZIM and restarting the container.

## Related

- [SearXNG private search](kb:apps/searxng)
