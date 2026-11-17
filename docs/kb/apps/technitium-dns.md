# Technitium DNS Server

A full DNS server with a web UI: ad and tracker blocking like Pi-hole, a recursive resolver like Unbound, authoritative zones for your home domain, DNS-over-HTTPS/TLS/QUIC upstreams, conditional forwarding and an optional DHCP server, all in one container.

## What you end up with

- Technitium answering DNS for the LAN on port 53
- Block lists, encrypted upstreams or full recursion
- A `home.lan` zone with records for your servers

## Before you start

- Port 53 must be free on the host. On Ubuntu, `systemd-resolved` holds it: set `DNSStubListener=no` in `/etc/systemd/resolved.conf` and restart it (see [Debug DNS on a Linux server](kb:linux/dns-debug)).
- Give the host a fixed address; clients will point at it.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `technitium`, with `ADMIN_PASSWORD` in `.env`:

```yaml
services:
  dns:
    image: technitium/dns-server:latest
    restart: unless-stopped
    hostname: dns1
    network_mode: host
    environment:
      DNS_SERVER_DOMAIN: dns1.home.lan
      DNS_SERVER_ADMIN_PASSWORD: ${ADMIN_PASSWORD}
    volumes:
      - ./config:/etc/dns
```

- [ ] Host networking lets the logs show each client's real address instead of Docker's gateway. The web console is on port 5380.

## Part 2 · Resolving

- [ ] **Settings → Proxy & Forwarders**: pick a forwarder preset with DNS-over-HTTPS (Cloudflare, Quad9) and protocol **HTTPS**, or leave forwarders empty to resolve recursively from the root servers yourself.
- [ ] **Settings → Blocking**: enable blocking and **Quick Add** a list (for example a Steven Black or OISD list). Lists refresh on their own.
- [ ] **Settings → General**: enable **DNSSEC validation**.

## Part 3 · Your own zone

- [ ] **Zones → Add Zone** → `home.lan`, type **Primary**.
- [ ] Add **A** records for servers (`nas` → `192.168.1.20`). Tick **Add reverse (PTR) record** so IPs resolve back to names.
- [ ] Point the router's DHCP DNS setting at this host, or use Technitium's own **DHCP** (turn the router's off first; two DHCP servers fight).

#### Check

- [ ] From a client: `nslookup nas.home.lan <dns-host>` returns the address, `nslookup doubleclick.net <dns-host>` returns `0.0.0.0`, and the **Dashboard** counts the queries.

## In Nightgrid

- Nightgrid's DNS integration covers Pi-hole and AdGuard Home; Technitium is managed in its own console. Add `http://<host>:5380` to [Services](view:services) to open it from there.
- [Network](view:network) → **Tools** → **DNS** with **Server** set to this host, run from another machine, confirms it answers on the LAN.
- Most other services depend on DNS: link them to this host in **Settings → Alerts → Dependencies**.

## Limits and gotchas

- One DNS server is a single point of failure. Run a second Technitium (it supports zone transfers to a secondary) and give clients both.
- With `network_mode: host`, **⧉** isn't offered (no published ports); use the Services link.
- Blocking breaks some apps and sign-in flows; the **Logs** page shows what was blocked so you can allow it.

## Related

- [Pi-hole](kb:apps/pihole)
- [AdGuard Home](kb:apps/adguard-home)
- [Unbound recursive DNS](kb:apps/unbound)
