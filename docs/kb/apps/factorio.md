# Factorio dedicated server

A headless Factorio server for a factory that keeps running (or pauses) when everyone logs off, with saves you can back up and mods kept in step with clients.

## What you end up with

- A server on UDP 34197 that friends join by address
- Saves and mods in one folder
- Auto-pause when nobody is online

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `factorio`:

```yaml
services:
  factorio:
    image: factoriotools/factorio:stable
    restart: unless-stopped
    ports:
      - "34197:34197/udp"
      - "27015:27015/tcp"
    volumes:
      - ./data:/factorio
```

- [ ] The first start creates a new map, `./data/config/server-settings.json` and an RCON password in `./data/config/rconpw`.
- [ ] Space Age and other paid DLC need your factorio.com username and token in `server-settings.json` for mod downloads; the base game doesn't.

## Part 2 · Settings

- [ ] Stop the stack, edit `./data/config/server-settings.json`: `name`, `description`, `game_password`, `visibility.public` (false for a private server), `auto_pause: true`. Start again.
- [ ] Use your own save: put it in `./data/saves/` and set `SAVE_NAME` to its name (without `.zip`) in the environment, with `LOAD_LATEST_SAVE: "false"`.
- [ ] Mods: copy the `mods` folder from a client into `./data/mods`. Server and clients must match versions.

## Part 3 · Open it to friends

- [ ] Forward UDP 34197 on the router to the host, or have friends join over your tailnet (no ports opened).

#### Check

- [ ] Multiplayer → Connect to address → `host:34197` loads the map.

## In Nightgrid

- Start, stop and read the server log in [Docker](view:docker); the log shows players joining and save times.
- Back up `./data/saves` with a timer from [System](view:system) → **Schedules**, and watch it in [Fleet](view:fleet) → **Backups**.
- After forwarding the port, the [Network](view:network) map shows it as internet-facing, which is what you expect here and nothing else.

## Limits and gotchas

- `:stable` follows the stable game release; pin a version tag if your group wants to stay on one.
- Big late-game bases are single-thread bound: CPU clock speed matters more than core count.

## Related

- [Game server host](kb:guides/game-server-host)
- [Game servers with LinuxGSM](kb:apps/linuxgsm)
- [Reach your servers away from home](kb:howto/remote-access)
