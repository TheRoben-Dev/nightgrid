# Game server host

Run Valheim, Minecraft, ARK and similar dedicated servers on one Proxmox node, and handle all of the day-to-day work from Nightgrid: starting and stopping, updates, mods, backups and power. The servers keep running with Nightgrid closed. Nightgrid is where you control them.

## What you end up with

- One Proxmox VE node with a container or VM per game, or one AMP instance running several games
- AMP game servers you can start, stop and restart from a card in [Services](view:services)
- Docker-based game servers managed as compose stacks in [Docker](view:docker)
- A snapshot before every game update or mod change, plus nightly vzdump backups
- Only the game ports reachable from the internet, checked with the port map
- Optional power savings: the node shuts down overnight and wakes on schedule or with Wake-on-LAN

## Before you start

- Hardware: a fast CPU matters more than core count (most game servers are single-thread bound). 32 GB RAM covers ARK plus a couple of smaller servers. Use NVMe for worlds and saves.
- Proxmox VE 8.x installed, with root SSH key login working from Nightgrid.
- Settings → **FEATURES**: the **Standard** preset leaves out Proxmox, VM screens and UPS. Pick **Everything**, or turn on **Proxmox** (and **UPS** if you have one) by hand.
- Decide per game: **AMP** (one licensed panel, lots of games, its own updater) or **plain Docker** (one compose stack per game, free images). Both work with Nightgrid. They're shown separately below.

## Part 1 · Guests for the servers

- [ ] Open [Proxmox](view:proxmox), pick the node and go to **GUESTS**.
- [ ] **+ Container** for Docker-based servers: Debian 12 template, unprivileged, enough disk for worlds, and your SSH public key. Give each heavy game (ARK) its own guest so it can be restarted and snapshotted alone.
- [ ] For Docker inside an LXC, turn on nesting and keyctl for that container (in the node's terminal):

```sh
pct set <ctid> --features nesting=1,keyctl=1
```

- [ ] For AMP, a small VM or container with Debian 12 is enough. Install AMP following CubeCoders' instructions in a [Terminal](view:terminal) on that guest.
- [ ] In the guest's window, open **HARDWARE** and set cores and memory. Give the game guest the cores it needs, not all of them, so one server can't starve the rest.
- [ ] Optional: cap a container's CPU time, or pin a VM to fixed cores so a busy server stays off the others' cores. Neither has a field in Nightgrid yet. The first command is for a container, the second for a VM:

```sh
pct set <ctid> --cores 4 --cpulimit 4
```

```sh
qm set <vmid> --affinity 0-3
```

- [ ] Add each guest as a host in Nightgrid (**+** in the sidebar) with its LAN address, so Docker, System and monitoring work on it directly.

#### Check

- [ ] Each guest shows in [Overview](view:overview) with CPU and memory.
- [ ] `docker info` works in a Terminal on the Docker guest.

## Part 2 · Servers as compose stacks (plain Docker)

- [ ] In [Docker](view:docker), pick the game guest and choose **+ New stack**. Start from the image (for example a Valheim or Minecraft server image you trust) and let Nightgrid fill in the ports and volumes it declares.
- [ ] Put the world and config in a named folder next to the compose file, not in the container.
- [ ] Server passwords and RCON passwords go in the stack's **Vault**, never in `compose.yaml`. **Deploy .env…** writes them to the host with mode 600.
- [ ] Set a restart policy so the server comes back after a crash or a reboot.

Example for a Minecraft server. Paste this into the editor (Docker → Stacks); don't send it to a terminal.

```yaml
services:
  minecraft:
    image: itzg/minecraft-server:latest
    restart: unless-stopped
    ports:
      - "25565:25565"
    environment:
      EULA: "TRUE"
      TYPE: "PAPER"
      MEMORY: "6G"
      RCON_PASSWORD: ${RCON_PASSWORD}
    volumes:
      - ./data:/data
```

- [ ] Pin the image to a version tag instead of `latest` once things work. DRIFT's compose review flags `:latest`, and a pinned tag means a game update only happens when you choose.

#### Check

- [ ] The stack is **up** and the container's logs show the server listening.
- [ ] You can join from a client on the LAN.

## Part 3 · AMP servers

- [ ] In [Services](view:services), **Add integration** → **AMP** with the panel address and a login. The password goes to the OS keychain.
- [ ] The AMP card lists instances with **start**, **stop** and **restart**. Stop and restart confirm with the instance named.
- [ ] Let AMP handle game updates and its own schedules. Those keep running with Nightgrid closed.
- [ ] Add a plain service check for the AMP panel too, so you get an alert when the panel itself is down.

#### Check

- [ ] The AMP card shows every instance with its state.
- [ ] Restarting one from Nightgrid shows in AMP's own console.

## Part 4 · Updates and mods without losing worlds

- [ ] Before a game update or a mod change: open the guest in [Proxmox](view:proxmox) → **SNAPSHOTS** and take one. Tick **with RAM** only for a VM you want to resume exactly as it was.
- [ ] For Docker stacks, use **Updates** in Docker. Nightgrid takes a stack snapshot (images, compose and `.env`) before updating, and **Roll back…** undoes it.
- [ ] Upload mods with [Files](view:files) by dragging them into the server's mods folder. Restart the container afterwards.
- [ ] If the world breaks, use **Roll back…** on the guest snapshot. Everything since that snapshot is lost, including player progress.
- [ ] Delete old snapshots once the update has proven itself. Long snapshot chains slow down disks on LVM-thin.

#### Check

- [ ] **SNAPSHOTS** shows the pre-update snapshot with a name you'll recognise later.

## Part 5 · Backups

- [ ] In [Proxmox](view:proxmox) → **BACKUPS**, see which guests no job covers. Nightgrid lists jobs but doesn't create them yet. Add a nightly job from the node's terminal:

```sh
pvesh create /cluster/backup --schedule "04:30" --storage <backup-storage> --vmid <id1>,<id2> --mode snapshot --prune-backups keep-daily=7,keep-weekly=4 --enabled 1
```

- [ ] Snapshot mode backs up a running server, but an in-memory world can be a few minutes behind. For games that save on a timer, schedule the job just after their autosave, or stop the server first with a hook script.
- [ ] **Back up now** on a guest before major game updates.
- [ ] Once a month, **restore as new** a backup to a spare id and start it, to prove it works. Then destroy the copy.

#### Check

- [ ] Every game guest's newest backup is under two days old in **BACKUPS** (orange after two days, red after a week).

## Part 6 · Exposing only the game ports

- [ ] Forward only the game ports on your router (for example 25565/tcp, 2456-2457/udp for Valheim, 7777-7778/udp and 27015/udp for ARK). Never forward 22 or 8006.
- [ ] Open [System](view:system) → **security** on each game guest. Every listening port shows who can reach it. **Port map · all hosts** puts them side by side.
- [ ] Bind RCON and admin web panels to 127.0.0.1 in compose (`127.0.0.1:25575:25575`) and reach them through a saved tunnel in [Tunnels](view:tunnels).
- [ ] On the guest, use **FIREWALL** in its Proxmox window to allow the game ports from anywhere and everything else only from the LAN. Node-level changes ask you to **Keep** them within 90 seconds.

#### Check

- [ ] Nothing other than game ports shows as reachable on all interfaces in the port map.

## Part 7 · Power and UPS

- [ ] Shutting down overnight: [System](view:system) → Schedules → **POWER SCHEDULE**, Shut down at 02:00 on the node. It's a systemd timer on the node, so it works with Nightgrid closed.
- [ ] Waking: put the node's MAC in the host editor and enable Wake-on-LAN in its BIOS. **WAKE** in the System toolbar sends the packet. Use **Wake from** another always-on host so you can wake it from away. A scheduled Wake up in POWER SCHEDULE also needs that **Wake from** host.
- [ ] Set guests to start with the node: in each guest's **HARDWARE** → boot options, turn on start at boot.
- [ ] UPS: [Fleet](view:fleet) → **UPS**, then **+ UPS**. Nightgrid's group shutdown needs the app running (the tray is enough). For shutdown with Nightgrid closed, configure NUT's own `upsmon` on the node.

#### Check

- [ ] After a scheduled shutdown and a wake, every game server comes back by itself without opening Nightgrid.

## When Nightgrid is closed

- Keeps running: the game servers, Docker restart policies, AMP's schedules and updates, vzdump jobs, the POWER SCHEDULE timer, start at boot, NUT `upsmon` if configured on the node.
- Needs Nightgrid open (or in the tray): alerts, uptime history, UPS group shutdown from Fleet, AMP start and stop from the card, Wake-on-LAN from **WAKE**.

## Limits and gotchas

- Nightgrid can't create vzdump jobs or set CPU affinity yet. Use the `pvesh` and `qm`/`pct` commands above.
- Snapshot-mode backups of a running server can catch a world mid-save. Test-restore at least once.
- Some game servers ignore SIGTERM and lose unsaved progress on Shut down. Use the game's save command over RCON first.
- Docker inside an unprivileged LXC is fine for game servers, but on ZFS it needs ZFS 2.2+ (PVE 8.1+) for overlay2.
- Wake-on-LAN usually doesn't work over Wi-Fi, and needs the NIC set to wake in both BIOS and OS.
- Exposing a game port exposes that server's bugs too. Keep game images updated and never run them privileged.

## Related

- Proxmox media server
- Family cloud and photos
- Repair bench server
- Local AI GPU server
- Smart home hub
- Remote family IT
- Dev and git deploy lab
