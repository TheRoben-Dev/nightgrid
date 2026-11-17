# Route containers through a VPN with Gluetun

Send one container's traffic (a download client, a scraper) through a commercial VPN, with a kill switch so it never leaks over your normal connection.

![Containers sending all traffic through Gluetun's VPN.](fig:app-gluetun)

## What you end up with

- A Gluetun container connected to your VPN provider
- Other containers using Gluetun's network, so all their traffic goes through the tunnel
- Their web UIs still reachable on your LAN

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** (or add to your `arr` stack):

```yaml
services:
  gluetun:
    image: qmcgaw/gluetun:latest
    restart: unless-stopped
    cap_add: [NET_ADMIN]
    devices:
      - /dev/net/tun:/dev/net/tun
    environment:
      VPN_SERVICE_PROVIDER: <provider>
      VPN_TYPE: wireguard
      WIREGUARD_PRIVATE_KEY: ${WG_PRIVATE_KEY}
      SERVER_COUNTRIES: Netherlands
      FIREWALL_OUTBOUND_SUBNETS: 192.168.1.0/24
    ports:
      - "127.0.0.1:8080:8080"
  qbittorrent:
    image: lscr.io/linuxserver/qbittorrent:latest
    restart: unless-stopped
    network_mode: "service:gluetun"
    environment:
      PUID: 1000
      PGID: 1000
    volumes:
      - ./qbittorrent:/config
      - /srv/data/torrents:/data/torrents
```

- [ ] The client's ports move to the `gluetun` service. Provider names and variables are in Gluetun's wiki; keys go in the **Vault**.

## Part 2 · Verify

- [ ] Check the public IP from inside: `docker exec <stack>-gluetun-1 wget -qO- https://ipinfo.io/ip`. It must be the VPN's, not yours.
- [ ] Stop Gluetun: the client loses all network. That's the kill switch.

## Part 3 · In Nightgrid

- [ ] Turn on log alerts for the Gluetun container; reconnect loops show as repeating errors.
- [ ] A command widget for the exit IP's country is easy with the same `wget` and a text value.

#### Check

- [ ] The client's web UI works from the LAN and its traffic exits through the VPN.

## Limits and gotchas

- When Gluetun restarts, containers using its network must restart too (`docker compose up -d` recreates them).
- Port forwarding for incoming peers only works with providers that support it; Gluetun's docs list them.

## Related

- [The *arr stack](kb:apps/arr-stack)
- [Docker networks: bridge, shared proxy network and macvlan](kb:linux/docker-networks)
