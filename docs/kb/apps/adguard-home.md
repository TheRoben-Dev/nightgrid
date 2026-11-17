# AdGuard Home

A DNS server with ad blocking, encrypted upstreams (DoH, DoT, DoQ) and per-client rules in one binary. An alternative to Pi-hole with built-in DNS-over-HTTPS.

![Where a filtering DNS server sits in the chain (Pi-hole shown, AdGuard Home works the same).](fig:dns-chain)

## What you end up with

- Network-wide blocking with encrypted upstream DNS
- Per-device settings (stricter filtering for kids' devices)
- Stats and controls on a Nightgrid card

## Before you start

- A host with a static LAN IP and port 53 free (see the `systemd-resolved` note in [Pi-hole](kb:apps/pihole)).

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `adguard`:

```yaml
services:
  adguard:
    image: adguard/adguardhome:latest
    restart: unless-stopped
    ports:
      - "53:53/tcp"
      - "53:53/udp"
      - "3000:3000/tcp"
      - "8082:80/tcp"
    volumes:
      - ./work:/opt/adguardhome/work
      - ./conf:/opt/adguardhome/conf
```

- [ ] Open port 3000 on the host for the first-run wizard. Set the web interface to port 80 inside the container (published as 8082), DNS on 53, and create the admin login.
- [ ] Remove the `3000` port mapping afterwards and recreate the stack.

## Part 2 · Configure

- [ ] Settings → DNS settings → upstreams, for example `https://dns.quad9.net/dns-query`. Enable parallel requests and DNSSEC.
- [ ] Filters → DNS blocklists: the default AdGuard list plus one or two more. More lists means more false positives.
- [ ] Filters → DNS rewrites for local names (`nas.home.lan` → `192.168.1.20`).
- [ ] Point the router's DHCP DNS at AdGuard's IP.

## Part 3 · In Nightgrid

- [ ] [Services](view:services) → **Add integration** → **AdGuard Home** with the admin login. **Manage…** covers pausing, rewrites and lists.

#### Check

- [ ] The card shows queries; a known ad domain resolves to `0.0.0.0`.

## Limits and gotchas

- Run a second instance (and AdGuardHome-Sync) if your network depends on it.
- Using `network_mode: host` instead of published ports lets AdGuard see real client IPs and act as a DHCP server; with bridge networking every query may look like it comes from Docker's gateway.

## Related

- [Pi-hole](kb:apps/pihole)
- [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard)
- [Unbound recursive DNS](kb:apps/unbound)
