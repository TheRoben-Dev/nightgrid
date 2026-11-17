# Dynamic DNS with ddclient

Keep a DNS name pointing at your home's changing public IP, so a VPN endpoint or a forwarded game port stays reachable.

## What you end up with

- `home.example.com` updated within minutes of an IP change
- ddclient running as a service on a home server

## Before you start

- A DNS provider with an update API (Cloudflare, Porkbun, Namecheap, DuckDNS, many others) and an API token limited to that zone.
- Check whether your router already does dynamic DNS; if it does, use that and skip this.
- CGNAT check: if the router's WAN address differs from what a "what is my IP" site shows, port forwards can't work; use a tailnet or a tunnel instead.

## Part 1 · Install

- [ ] `sudo apt install -y ddclient` (skip the setup questions; you'll write the config).
- [ ] `/etc/ddclient.conf` for Cloudflare (mode 600):

```
daemon=300
ssl=yes
usev4=webv4, webv4=ipify-ipv4
protocol=cloudflare
zone=example.com
login=token
password=<api-token>
home.example.com
```

- [ ] `sudo systemctl restart ddclient` and check: `sudo ddclient -query` then `sudo journalctl -u ddclient -n 20`.

## Part 2 · In Nightgrid

- [ ] [System](view:system) → **services** shows `ddclient` with restart buttons and its journal.
- [ ] A command widget that warns when DNS and reality disagree (`dig` from `dnsutils`):

```sh
test "$(dig +short home.example.com @1.1.1.1)" = "$(dig +short myip.opendns.com @resolver1.opendns.com)" && echo 0 || echo 1
```

  Read it as first number, warn at 1.

#### Check

- [ ] `dig +short home.example.com` returns your current public IP.

## Limits and gotchas

- The token is a credential; keep the config mode 600. Nightgrid's config drift masks it in diffs.
- Proxied (orange-cloud) Cloudflare records hide your IP but only carry HTTP(S); VPN and game ports need DNS-only records.

## Related

- [WireGuard VPN server](kb:apps/wireguard)
- [Cloudflare Tunnel](kb:apps/cloudflared)
- [Build dashboards with command widgets](kb:howto/command-widgets)
