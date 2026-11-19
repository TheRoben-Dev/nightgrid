# Syncthing

Keep folders in sync between computers, phones and servers directly, peer to peer, with no cloud in between.

![An always-on server keeps every device in sync.](fig:app-syncthing)

## What you end up with

- A Syncthing instance on a server that's always on, so devices sync even when they're never online together
- Versioned folders so a deleted or overwritten file can come back
- The Nightgrid profile sync folder kept on every machine

## Part 1 · On the server

- [ ] [Docker](view:docker) → **+ New stack** named `syncthing`:

```yaml
services:
  syncthing:
    image: syncthing/syncthing:latest
    restart: unless-stopped
    hostname: server-sync
    environment:
      PUID: 1000
      PGID: 1000
    ports:
      - "127.0.0.1:8384:8384"
      - "22000:22000/tcp"
      - "22000:22000/udp"
      - "21027:21027/udp"
    volumes:
      - ./config:/var/syncthing/config
      - /srv/sync:/var/syncthing/data
```

- [ ] The GUI is on 8384, bound to localhost. Open it through a saved tunnel in [Tunnels](view:tunnels) (local 8384 → `127.0.0.1:8384`).
- [ ] Settings → GUI: set a username and password.

## Part 2 · Connect devices

- [ ] Install Syncthing on your computers (Syncthing app on Android, Möbius Sync on iOS).
- [ ] Add each device by its ID on both sides, share a folder, accept it on the other end.
- [ ] Folder → Edit → File Versioning → **Staggered** on the server, so it keeps old versions.

## Part 3 · Nightgrid profile

- [ ] [Services](view:services) → **Add integration** → **Syncthing** with the API key (Actions → Settings → General): folder states, devices online and errors on a card, with **Rescan** and pause/resume (see [Add app cards](kb:howto/integrations)).

- [ ] Share a folder like `nightgrid` between your computers and the server.
- [ ] In Nightgrid, **Settings → Data → SYNC FOLDER**: pick that folder. **Export to sync folder** puts the encrypted profile there.

#### Check

- [ ] A file saved on one computer appears on the other within seconds while both are online, and via the server when they aren't.

## Limits and gotchas

- Sync isn't backup: a deleted file is deleted everywhere (versioning softens this). Back up the server's copy too.
- Port 22000 open on the LAN helps local speed; it needn't be forwarded on the router (relays and NAT traversal handle remote).
- Don't sync live databases or VM disks.

## Related

- [Back up and move your Nightgrid setup](kb:howto/profiles)
- [Nextcloud](kb:apps/nextcloud)
- [restic backups with systemd timers](kb:apps/restic)
