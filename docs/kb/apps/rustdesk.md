# RustDesk relay for remote support

Help family and friends with their computers through your own RustDesk ID and relay server, instead of a commercial remote-support service.

## What you end up with

- `hbbs` (ID server) and `hbbr` (relay) on a server you control
- RustDesk clients pointed at your server with your public key
- Remote sessions that don't depend on a third party

## Before you start

- A server reachable from the internet (a small VPS works well), with ports 21115–21117 TCP and 21116 UDP open.
- The bigger picture for looking after family machines is in [Remote family IT](kb:guides/remote-family-it).

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `rustdesk`:

```yaml
services:
  hbbs:
    image: rustdesk/rustdesk-server:latest
    command: hbbs
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./data:/root
    depends_on: [hbbr]
  hbbr:
    image: rustdesk/rustdesk-server:latest
    command: hbbr
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./data:/root
```

- [ ] Allow the ports in [System](view:system) → **FIREWALL**: 21115/tcp, 21116/tcp, 21116/udp, 21117/tcp.
- [ ] The public key appears in `./data/id_ed25519.pub` after the first start. Copy it with [Files](view:files).

## Part 2 · Clients

- [ ] In each RustDesk client: Settings → Network → ID/Relay server: ID server `rd.example.com`, key = the public key. Leave the relay blank (it uses the same host).
- [ ] Set a permanent password on unattended machines, or have the person read you the one-time code.

#### Check

- [ ] Two clients on different networks connect, and the connection info says it's using your relay or direct.

## Limits and gotchas

- Without the key set, clients refuse your server. Without the key requirement, anyone can use your relay.
- The relay carries screen traffic; a VPS with limited bandwidth may throttle busy sessions.

## Related

- [Remote family IT](kb:guides/remote-family-it)
- [Apache Guacamole remote desktop gateway](kb:apps/guacamole)
