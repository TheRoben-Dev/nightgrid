# Diun image update notifications

Get told when new versions of the images you run are published, without anything updating by itself. Pairs with Nightgrid's snapshot-first updates.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `diun` on each Docker host:

```yaml
services:
  diun:
    image: crazymax/diun:latest
    restart: unless-stopped
    command: serve
    environment:
      TZ: Etc/UTC
      DIUN_WATCH_SCHEDULE: "0 */6 * * *"
      DIUN_PROVIDERS_DOCKER: "true"
      DIUN_PROVIDERS_DOCKER_WATCHBYDEFAULT: "true"
      DIUN_NOTIF_NTFY_ENDPOINT: https://ntfy.example.com
      DIUN_NOTIF_NTFY_TOPIC: updates
      DIUN_NOTIF_NTFY_TOKEN: ${NTFY_TOKEN}
    volumes:
      - ./data:/data
      - /var/run/docker.sock:/var/run/docker.sock:ro
```

## Part 2 · Act on it

- [ ] When a notification arrives, open [Docker](view:docker) → **Updates**, read **What changed**, and update with a snapshot (see [Update containers without fear](kb:howto/container-updates)).

#### Check

- [ ] The first run notifies once per image it starts watching.

## Limits and gotchas

- Watchtower-style auto-updating is the opposite approach: convenient, but no review and no snapshot. Prefer notify-then-update for anything with data.

## Related

- [Update containers without fear](kb:howto/container-updates)
- [ntfy push notifications](kb:apps/ntfy)
