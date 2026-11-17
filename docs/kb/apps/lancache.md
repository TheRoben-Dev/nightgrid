# LanCache for game downloads

Cache Steam, Epic, Battle.net, Xbox and other game downloads on a server, so the second PC (or a reinstall) downloads at LAN speed instead of pulling 100 GB from the internet again. Worth it with several gaming PCs or handhelds, or a slow or capped connection.

![Game downloads redirected to the cache.](fig:app-lancache)

## What you end up with

- The LanCache monolithic cache and its DNS service in Docker
- Clients pointed at its DNS, so supported game CDNs go through the cache
- A cache sized to fit your disk, with old games evicted automatically

## Before you start

- A disk with room: 500 GB is a sensible minimum, more if you keep many large games. An SSD or a fast HDD both work; the network is usually the limit.
- The server needs ports 80 and 443 free (443 is passed through untouched, not cached). Don't run it on the box with your reverse proxy, or give it a second IP address.

## Part 1 · The stack

- [ ] In a [Terminal](view:terminal) tab on the host, clone the official compose setup: `git clone https://github.com/lancachenet/docker-compose ~/stacks/lancache`.
- [ ] Edit `.env` in [Files](view:files):
  - `LANCACHE_IP` and `DNS_BIND_IP`: the server's LAN address
  - `UPSTREAM_DNS`: where other lookups go (your router, Pi-hole, or `1.1.1.1`)
  - `CACHE_ROOT`: the folder on the big disk, such as `/srv/lancache`
  - `CACHE_DISK_SIZE`: a bit under the free space, such as `900g`
- [ ] `docker compose up -d` in that folder; both containers appear in [Docker](view:docker).

## Part 2 · Point clients at it

- [ ] Simplest: in the router's DHCP settings, hand out the LanCache server as the DNS server. LanCache answers game CDN names with its own address and forwards everything else upstream.
- [ ] Already run Pi-hole or AdGuard Home? Keep them as the clients' DNS and instead set them to forward to the LanCache DNS, or add LanCache's domain lists as local DNS overrides (the `cache-domains` project publishes lists for both).
- [ ] Steam: no client setting needed. Clear the download cache in Steam's settings once so it looks up the CDN again.

#### Check

- [ ] Install a small game on one PC, uninstall it, install it again (or on a second PC): the second download runs at LAN speed, and `tail -f /srv/lancache/logs/access.log` shows `HIT` lines.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** shows the cache disk's fill level and forecast; the cache stays under `CACHE_DISK_SIZE` by evicting the oldest data.
- [Network](view:network) → **Tools** → **DNS** with the LanCache server in the **Server** field and a name like `lancache.steamcontent.com` should answer with the cache's IP.

## Limits and gotchas

- Only HTTP downloads are cached. Services that use HTTPS for game files (some launchers, many updates) pass through uncached; check LanCache's list of supported services.
- If the cache box is down and clients use it for DNS, the whole internet looks broken. Use Pi-hole or AdGuard as the primary DNS with LanCache behind it, or give clients a second DNS server.
- First downloads through the cache are no faster; only repeats are.

## Related

- [Game server host](kb:guides/game-server-host)
- [Pi-hole](kb:apps/pihole)
- [Debug DNS on a Linux server](kb:linux/dns-debug)
