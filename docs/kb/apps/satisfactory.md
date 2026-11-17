# Satisfactory dedicated server

Keep your factory running for the whole group: a dedicated server in Docker that updates itself, keeps rolling save backups and lets anyone in the group play without the host being online.

![Two ways for friends to reach your game server.](fig:app-game-server)

## What you end up with

- A Satisfactory 1.x dedicated server claimed and managed from the game
- Saves and backups in a folder you can copy
- Ports forwarded so friends can join from anywhere

## Before you start

- RAM is the limit: 8 GB minimum, 12–16 GB for late-game factories. Give it a modern CPU with good single-core speed.
- The game version on the server and on every client must match. Experimental clients need the experimental server.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `satisfactory`:

```yaml
services:
  satisfactory:
    image: wolveix/satisfactory-server:latest
    restart: unless-stopped
    environment:
      MAXPLAYERS: "4"
      PUID: "1000"
      PGID: "1000"
      STEAMBETA: "false"
    volumes:
      - ./config:/config
    ports:
      - "7777:7777/udp"
      - "7777:7777/tcp"
      - "8888:8888/tcp"
    deploy:
      resources:
        limits:
          memory: 12G
```

- [ ] `STEAMBETA: "true"` runs the experimental branch. The first start downloads several GB; watch the logs until the server says it's ready.
- [ ] The `config` folder must belong to uid 1000: `sudo chown -R 1000:1000 config` if the logs show permission errors.

## Part 2 · Claim the server

- [ ] In the game: **Server Manager** → **Add Server** → the host's address and port 7777.
- [ ] The first person to connect claims it: set a server name and an **admin password**, then a **client password** for friends.
- [ ] **Create Game** to start a new world, or upload an existing save under **Manage Saves**.

## Part 3 · Outside access

- [ ] Forward TCP and UDP 7777 and TCP 8888 to the Docker host. Recent versions use TCP for the server API and 8888 for reliable messaging; missing either makes the server show as offline in the Server Manager.

#### Check

- [ ] A friend adds the server by your public IP, sees it online and joins with the client password.

## In Nightgrid

- [Docker](view:docker) → the container's logs show joins, saves and crashes; **Snapshots** are taken before Nightgrid updates the image, so a broken update can be rolled back.
- Autosaves land in `config/saved/server/`, and the image keeps backups in `config/backups/`. Add that folder in [Fleet](view:fleet) → **Backups** → **+ Add job** as a path.
- Satisfactory is memory-hungry: watch the host's memory in [System](view:system) and add swap or zram if it gets close to the limit.

## Limits and gotchas

- Out-of-memory kills look like random crashes; the host's [System](view:system) → **timeline** shows them as OOM kills.
- Mods need the Satisfactory Mod Manager on every client and matching server-side installs; check each mod supports dedicated servers.
- Updating the server before players update their game makes them unable to join until they do.

## Related

- [Game server host](kb:guides/game-server-host)
- [Valheim server in Docker](kb:apps/valheim)
- [Swap and zram](kb:linux/swap-zram)
