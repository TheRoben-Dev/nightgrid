# Minecraft server

A Java Edition server in Docker with Paper, automatic backups, RCON kept private and only the game port open.

![Two ways for friends to reach your game server.](fig:app-game-server)

## What you end up with

- A Paper server with world data in a folder you control
- Backups of the world every few hours, pruned automatically
- Console commands through RCON from Nightgrid's terminal

## Before you start

- 2–4 GB RAM for a small group, more for mods; a fast single core matters most.
- For a whole game-server box with Proxmox, AMP and power schedules, see [Game server host](kb:guides/game-server-host).

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `minecraft`:

```yaml
services:
  mc:
    image: itzg/minecraft-server:java21
    restart: unless-stopped
    ports:
      - "25565:25565"
    environment:
      EULA: "TRUE"
      TYPE: PAPER
      VERSION: LATEST
      MEMORY: 4G
      MOTD: Our server
      DIFFICULTY: normal
      ENABLE_WHITELIST: "true"
      WHITELIST: player_one,player_two
      RCON_PASSWORD: ${RCON_PASSWORD}
    volumes:
      - ./data:/data
  backups:
    image: itzg/mc-backup
    restart: unless-stopped
    depends_on: [mc]
    environment:
      BACKUP_INTERVAL: 6h
      PRUNE_BACKUPS_DAYS: 7
      RCON_HOST: mc
      RCON_PASSWORD: ${RCON_PASSWORD}
    volumes:
      - ./data:/data:ro
      - ./backups:/backups
```

- [ ] `RCON_PASSWORD` in the **Vault**. RCON isn't published, so it's only reachable inside the stack.
- [ ] Pin `VERSION` to a release (for example `1.21.4`) once your plugins are chosen.

## Part 2 · Console

- [ ] In a [Terminal](view:terminal) on the host:

```sh
docker exec -i minecraft-mc-1 rcon-cli
```

- [ ] Save it as a snippet in [Automation](view:automation) with a `{{command}}`: `docker exec minecraft-mc-1 rcon-cli {{command}}` (for example `whitelist add name`, `say Restarting in 5 minutes`).

## Part 3 · Network

- [ ] Forward TCP 25565 on the router to the host. Nothing else.
- [ ] [System](view:system) → **security** shows 25565 as the only port on all interfaces from this stack.

## Part 4 · In Nightgrid

- [ ] Add the backups folder as a path job in [Fleet](view:fleet) → **Backups** so a stalled backup container alerts.
- [ ] Turn on log alerts for the container; "Can't keep up!" bursts mean the server is overloaded.

#### Check

- [ ] Friends can join from outside, and `./backups` gets a new archive every 6 hours.

## Limits and gotchas

- Bedrock players need GeyserMC on the server (and UDP 19132 forwarded).
- Plugins and Paper updates can break worlds; take a backup or snapshot before changing either.

## Related

- [Game server host](kb:guides/game-server-host)
- [Game servers with LinuxGSM](kb:apps/linuxgsm)
- [Update containers without fear](kb:howto/container-updates)
