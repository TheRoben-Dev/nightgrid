# Palworld dedicated server

A Palworld world that stays up for your group, with settings you control and saves you back up. Palworld servers are memory hungry, so size the host first.

![Two ways for friends to reach your game server.](fig:app-game-server)

## What you end up with

- A server on UDP 8211 your friends join by IP
- Server settings from environment variables in compose
- Scheduled restarts and backups

## Before you start

- 16 GB of RAM for a small group, more for a long-running world (memory use grows over time)
- A fast SSD for saves

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `palworld`, with the passwords in `.env`:

```yaml
services:
  palworld:
    image: thijsvanloef/palworld-server-docker:latest
    restart: unless-stopped
    stop_grace_period: 30s
    ports:
      - "8211:8211/udp"
      - "27015:27015/udp"
    environment:
      PLAYERS: "16"
      SERVER_NAME: "Our world"
      SERVER_PASSWORD: ${SERVER_PASSWORD}
      ADMIN_PASSWORD: ${ADMIN_PASSWORD}
      COMMUNITY: "false"
      BACKUP_ENABLED: "true"
      BACKUP_CRON_EXPRESSION: "0 */4 * * *"
      AUTO_REBOOT_ENABLED: "true"
      AUTO_REBOOT_CRON_EXPRESSION: "0 5 * * *"
      TZ: Etc/UTC
    volumes:
      - ./palworld:/palworld
```

- [ ] The first start downloads the server with SteamCMD; follow it in the container log.
- [ ] Game rules (EXP rate, death penalty and so on) are more environment variables; see the image's documentation for the names.

## Part 2 · Join

- [ ] In the game: Join Multiplayer Game → enter `host-ip:8211` at the bottom, then the server password.
- [ ] From outside: forward UDP 8211, or use your tailnet.

#### Check

- [ ] You can join, build something, restart the stack, and it's still there.

## In Nightgrid

- Watch memory on [Overview](view:overview): the nightly auto reboot keeps the leak in check.
- Backups land in `./palworld/backups`; copy them off the host with [restic](kb:apps/restic) and watch the job in [Fleet](view:fleet) → **Backups**.

## Limits and gotchas

- Update the game on clients and server together; a version mismatch refuses joins.
- Moving a co-op world to a dedicated server needs a save conversion with the host's player ID; search the image docs for the migration steps.

## Related

- [Game server host](kb:guides/game-server-host)
- [Factorio dedicated server](kb:apps/factorio)
- [Mumble voice chat](kb:apps/mumble)
