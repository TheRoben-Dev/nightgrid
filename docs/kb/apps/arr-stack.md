# The *arr stack

Sonarr (TV), Radarr (films), Prowlarr (indexers), Bazarr (subtitles) and a download client, set up so imports are instant hardlinks instead of slow copies.

## What you end up with

- Requests that search, download, rename and file themselves into your media library
- One `/data` folder shared by every container, so moves are hardlinks
- All admin UIs behind localhost or a login

## Before you start

- One filesystem for downloads and media. Hardlinks don't cross filesystems or datasets:

```
/srv/data/
  torrents/{movies,tv}
  media/{movies,tv}
```

- `sudo mkdir -p /srv/data/{torrents,media}/{movies,tv} && sudo chown -R 1000:1000 /srv/data`.
- Only download what you have the right to.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `arr`:

```yaml
x-common: &common
  restart: unless-stopped
  environment:
    PUID: 1000
    PGID: 1000
    TZ: Etc/UTC
    UMASK: "002"

services:
  qbittorrent:
    <<: *common
    image: lscr.io/linuxserver/qbittorrent:latest
    ports: ["127.0.0.1:8080:8080", "6881:6881", "6881:6881/udp"]
    volumes: ["./qbittorrent:/config", "/srv/data/torrents:/data/torrents"]
  prowlarr:
    <<: *common
    image: lscr.io/linuxserver/prowlarr:latest
    ports: ["127.0.0.1:9696:9696"]
    volumes: ["./prowlarr:/config"]
  sonarr:
    <<: *common
    image: lscr.io/linuxserver/sonarr:latest
    ports: ["127.0.0.1:8989:8989"]
    volumes: ["./sonarr:/config", "/srv/data:/data"]
  radarr:
    <<: *common
    image: lscr.io/linuxserver/radarr:latest
    ports: ["127.0.0.1:7878:7878"]
    volumes: ["./radarr:/config", "/srv/data:/data"]
  bazarr:
    <<: *common
    image: lscr.io/linuxserver/bazarr:latest
    ports: ["127.0.0.1:6767:6767"]
    volumes: ["./bazarr:/config", "/srv/data/media:/data/media"]
```

- [ ] Reach the UIs with saved tunnels in [Tunnels](view:tunnels), or behind your proxy with a login (see [Authelia single sign-on](kb:apps/authelia)).
- [ ] qBittorrent prints a temporary admin password in its log on first start. Find it in the container's logs in Docker, then change it.

## Part 2 · Wire them together

- [ ] qBittorrent: default save path `/data/torrents`, categories `tv` and `movies` saving to `/data/torrents/tv` and `/data/torrents/movies`.
- [ ] Prowlarr: add indexers, then Settings → Apps → add Sonarr and Radarr with their API keys (Settings → General in each). Use container names as hosts (`sonarr:8989`).
- [ ] Sonarr and Radarr: root folders `/data/media/tv` and `/data/media/movies`; download client qBittorrent at `qbittorrent:8080` with the matching category.
- [ ] Settings → Media Management: **Use Hardlinks instead of Copy** on.
- [ ] Point Jellyfin or Plex at `/srv/data/media`.

#### Check

- [ ] After an import, `ls -li` shows the same inode number for the file in `torrents` and in `media`.

## Part 3 · In Nightgrid

- [ ] Add each UI as a service check in [Services](view:services) on its local port (through the host's address) so a crashed app alerts.
- [ ] Turn on log alerts for the stack's containers; failed imports show as repeating ERROR lines.
- [ ] Disk forecasts in [Fleet](view:fleet) → **Disks** tell you when `/srv/data` will be full.

## Limits and gotchas

- Routing the download client through a VPN: put it behind a Gluetun container (`network_mode: service:gluetun`) and publish its ports on Gluetun instead.
- Different mount paths in different containers break hardlinks and remote path mapping. Same `/data` everywhere.

## Related

- [Jellyfin media server](kb:apps/jellyfin)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
- [mergerfs and SnapRAID](kb:apps/mergerfs-snapraid)
