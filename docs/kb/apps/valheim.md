# Valheim server in Docker

A dedicated Valheim world that runs whether or not anyone is online, with automatic updates, scheduled world backups and optional BepInEx mods, all from one container.

## What you end up with

- A password-protected Valheim server reachable from the internet
- World backups every few hours, pruned automatically
- Mods (BepInEx plugins) that survive container updates

## Before you start

- 4 GB RAM free (more with mods or big worlds) and 2 fast CPU cores. Valheim's server is mostly single-threaded.
- Router access to forward UDP ports. Behind CGNAT, see the Limits section.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `valheim`, with `SERVER_PASS` in `.env` (at least 5 characters and not part of the server name):

```yaml
services:
  valheim:
    image: ghcr.io/lloesche/valheim-server
    restart: unless-stopped
    cap_add:
      - sys_nice
    stop_grace_period: 2m
    environment:
      SERVER_NAME: "Our Viking Server"
      WORLD_NAME: Midgard
      SERVER_PASS: ${SERVER_PASS}
      SERVER_PUBLIC: "false"
      TZ: Etc/UTC
      BACKUPS: "true"
      BACKUPS_CRON: "0 */4 * * *"
      BACKUPS_MAX_AGE: "7"
      UPDATE_IF_IDLE: "true"
    volumes:
      - ./config:/config
      - ./data:/opt/valheim
    ports:
      - "2456-2457:2456-2457/udp"
```

- [ ] The first start downloads the server (about 1 GB); follow it in the container's logs until "Game server connected".
- [ ] `stop_grace_period` gives the server time to save the world on stop. Don't shorten it.
- [ ] To move an existing world, copy its `.db` and `.fwl` files into `config/worlds_local/` before the first start and match `WORLD_NAME`.

## Part 2 · Reach it from outside

- [ ] Forward UDP 2456–2457 on the router to the Docker host.
- [ ] Friends join with **Join IP** → `your-public-ip:2456` and the password. `SERVER_PUBLIC: "true"` lists it in the community browser instead.
- [ ] Crossplay with Xbox/Game Pass players: add `SERVER_ARGS: "-crossplay"`; joins then go through PlayFab with a join code shown in the logs.

## Part 3 · Mods

- [ ] Set `BEPINEX: "true"` and redeploy. Plugin `.dll` files go in `config/bepinex/plugins/`, their configs in `config/bepinex/config/`.
- [ ] Every player needs the same mods and versions; most mods say whether they're server-only or both.

#### Check

- [ ] You can join from outside your network, and `config/backups/` gains a zip every 4 hours.

## In Nightgrid

- [Docker](view:docker) shows the container's logs and restart button; a restart is safe once nobody's online.
- Add `config/backups` in [Fleet](view:fleet) → **Backups** → **+ Add job** as a path so you're told if backups stop.
- [Fleet](view:fleet) → **Logs**: a saved search for "Got connection SteamID" with ◉ on shows who joined, if you want to know.

## Limits and gotchas

- Behind CGNAT (no real public IP) port forwarding can't work; use crossplay join codes, or a VPS relay over WireGuard.
- Big updates sometimes break mods; turn `BEPINEX` off until the mods catch up, rather than staying on an old server version clients can't join.
- Worlds are worth more than the server; keep a copy of `config/backups` off the host.

## Related

- [Game server host](kb:guides/game-server-host)
- [Game servers with LinuxGSM](kb:apps/linuxgsm)
- [Satisfactory dedicated server](kb:apps/satisfactory)
