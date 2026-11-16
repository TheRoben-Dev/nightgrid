# Crafty Controller for Minecraft

A web panel for running several Minecraft servers (Java and Bedrock): create servers, schedule restarts and backups, and give friends limited access.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `crafty`:

```yaml
services:
  crafty:
    image: registry.gitlab.com/crafty-controller/crafty-4:latest
    restart: unless-stopped
    environment:
      TZ: Etc/UTC
    ports:
      - "127.0.0.1:8443:8443"
      - "25565-25575:25565-25575"
      - "19132:19132/udp"
    volumes:
      - ./backups:/crafty/backups
      - ./logs:/crafty/logs
      - ./servers:/crafty/servers
      - ./config:/crafty/app/config
      - ./import:/crafty/import
```

- [ ] Open the panel through a saved tunnel in [Tunnels](view:tunnels) to port 8443. The first admin password is in `./config/default-creds.txt`.

## Part 2 · Servers

- [ ] Create a server (Paper, Fabric, Forge, vanilla or Bedrock), give each a port in 25565–25575, and set memory.
- [ ] Schedules: nightly backup and a daily restart with a warning message.
- [ ] Forward only the ports in use on the router.

#### Check

- [ ] Each server starts from the panel and accepts players.

## Limits and gotchas

- One container runs every server; a host restart restarts them all. For isolation, use one stack per server (see [Minecraft server](kb:apps/minecraft)).
- Add `./backups` as a path job in [Fleet](view:fleet) → **Backups**.

## Related

- [Minecraft server](kb:apps/minecraft)
- [Game server host](kb:guides/game-server-host)
