# SABnzbd Usenet downloader

Downloads from Usenet: fast, reliable, and verified and repaired with par2 automatically. Pairs with Sonarr, Radarr and Lidarr as their download client.

## What you end up with

- SABnzbd on port 8080 connected to your Usenet provider
- Categories that sort finished downloads for the *arr apps
- Downloads on a fast disk, finished files moved to the library

## Before you start

- An account with a Usenet provider and an indexer (both paid services, usually)
- A download folder on a fast disk, ideally the same filesystem as the media library so moves are instant

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `sabnzbd`, or add it to your *arr stack:

```yaml
services:
  sabnzbd:
    image: lscr.io/linuxserver/sabnzbd:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
    volumes:
      - ./config:/config
      - /srv/data:/data
    ports:
      - "8080:8080"
```

- [ ] Mount the same `/data` path as Sonarr and Radarr so paths match in every container.

## Part 2 · Setup wizard

- [ ] Open port 8080; the wizard asks for your provider's server, port 563 (SSL), user, password and connection count.
- [ ] Config → Folders: temporary `/data/usenet/incomplete`, completed `/data/usenet/complete`.
- [ ] Config → Categories: `tv`, `movies`, `music` with their own subfolders.
- [ ] Config → General: note the **API key** for the *arr apps.

## Part 3 · Connect the *arr apps

- [ ] In Sonarr/Radarr: Settings → Download Clients → **+** → SABnzbd, host `sabnzbd`, port 8080, the API key, and the matching category.

#### Check

- [ ] Grab something from Sonarr: it appears in SABnzbd's queue, downloads, and Sonarr imports it.

## In Nightgrid

- Watch the download disk in [Fleet](view:fleet) → **Disks**; a full disk pauses SABnzbd.
- Keep the web UI on the LAN; open it from away through [Tunnels](view:tunnels) or **⧉** in [Docker](view:docker).

## Limits and gotchas

- Port 8080 is a popular default; change the host side of the mapping if another app has it.
- Downloads and par2 repair are CPU and disk heavy; a slow USB disk as temp folder makes everything crawl.

## Related

- [The *arr stack](kb:apps/arr-stack)
- [Jellyfin media server](kb:apps/jellyfin)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
