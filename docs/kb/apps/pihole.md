# Pi-hole

Run Pi-hole v6 as your network's DNS server to block ads and trackers for every device, give your servers local names, and control it from Nightgrid.

![Where Pi-hole sits in the DNS chain.](fig:dns-chain)

## What you end up with

- Network-wide ad blocking, handed out by your router's DHCP
- Local names like `nas.home.lan`
- Stats and controls on a Nightgrid card

## Before you start

- A host with a static LAN IP. DNS must not move.
- Port 53 free: on Ubuntu, `systemd-resolved` holds it. Free it:

```sh
sudo sed -i 's/^#\?DNSStubListener=.*/DNSStubListener=no/' /etc/systemd/resolved.conf
sudo systemctl restart systemd-resolved
```

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `pihole`: {check: container:pihole}

```yaml
services:
  pihole:
    image: pihole/pihole:latest
    restart: unless-stopped
    ports:
      - "53:53/tcp"
      - "53:53/udp"
      - "8081:80/tcp"
    environment:
      TZ: Etc/UTC
      FTLCONF_webserver_api_password: ${WEB_PASSWORD}
      FTLCONF_dns_listeningMode: all
      FTLCONF_dns_upstreams: 9.9.9.9;149.112.112.112
    volumes:
      - ./etc-pihole:/etc/pihole
    cap_add:
      - SYS_NICE
```

- [ ] Put `WEB_PASSWORD` in the **Vault**. Set `TZ` to yours.
- [ ] Web UI on port 8081 of the host, path `/admin`.

## Part 2 · Point the network at it

- [ ] In your router's DHCP settings, set the DNS server to the Pi-hole's IP. Remove the router itself as a second DNS, or ads leak through it.
- [ ] Better redundancy: a second Pi-hole on another host, both handed out by DHCP.
- [ ] Local names: Pi-hole → Settings → Local DNS Records, or from Nightgrid (Part 3).

## Part 3 · In Nightgrid

- [ ] [Services](view:services) → **Add integration** → **Pi-hole** with an app password (Settings → Web interface / API → Configure app password).
- [ ] **Manage…** pauses blocking, edits local records and lists (see [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard)).
- [ ] Add a dependency: "everything depends on DNS" isn't quite true, but services checked by name do. Add the ones that matter in **Settings → Alerts → Dependencies**.

#### Check

- [ ] `dig @<pihole-ip> doubleclick.net` returns `0.0.0.0`, and the card shows queries climbing.

## Limits and gotchas

- If the Pi-hole host goes down with no second DNS, the whole network "loses internet".
- Some smart TVs and Android devices hardcode 8.8.8.8. Block outbound DNS on the router to force them through Pi-hole.
- Recursive DNS without a public upstream: [Unbound recursive DNS](kb:apps/unbound).

## Related

- [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard)
- [Unbound recursive DNS](kb:apps/unbound)
- [AdGuard Home](kb:apps/adguard-home)
