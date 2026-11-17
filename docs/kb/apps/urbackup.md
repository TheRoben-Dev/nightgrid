# UrBackup for Windows and Linux clients

A backup server for the PCs and laptops in the house: file backups and full image backups of Windows machines, incremental and deduplicated, with a web UI and a client that runs in the tray.

## What you end up with

- Nightly file backups of chosen folders from every PC
- Image backups of Windows system drives, restorable with a boot USB
- One web page showing which clients are behind

## Part 1 · The server

- [ ] On a host with plenty of storage (a ZFS dataset or btrfs volume is ideal):

```yaml
services:
  urbackup:
    image: uroni/urbackup-server:latest
    restart: unless-stopped
    network_mode: host
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
    volumes:
      - ./database:/var/urbackup
      - /tank/urbackup:/backups
```

- [ ] Deploy it with [Docker](view:docker) → **+ New stack**. `network_mode: host` lets clients find the server by broadcast on the LAN. The web UI is on port 55414.
- [ ] Settings → Users: create an admin account (it starts without one).

## Part 2 · Clients

- [ ] Install the UrBackup client on each PC (Windows installer, or the Linux client script from the downloads page). On the LAN it finds the server by itself.
- [ ] In the web UI, per client: which folders (default "all user folders" on Windows), file backup interval and image backup interval.
- [ ] Laptops away from home: Settings → Internet: give the server a public name and port 55415, or connect them over your tailnet instead.

## Part 3 · Restore

- [ ] Files: Backups → client → browse and download, or "Restore" from the client's tray icon.
- [ ] Images: write the UrBackup restore ISO to a USB stick, boot the PC from it, pick the image.

#### Check

- [ ] Restore one file to a different folder, and test-boot the restore USB once so you know it sees the disk and network.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** forecasts when the backup pool fills up; image backups grow fast.
- Add the web UI to [Services](view:services), and back up `./database` too: without it, the backup folders are hard to use.

## Limits and gotchas

- Image backups are Windows only. For Linux machines use file backups (or restic).
- On ZFS or btrfs, turn on the matching snapshot mode in UrBackup for much faster incrementals.

## Related

- [Backups](kb:basics/backups)
- [restic backups with systemd timers](kb:apps/restic)
- [Build a DIY NAS](kb:guides/diy-nas)
