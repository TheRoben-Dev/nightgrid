# NetBird mesh VPN

A WireGuard mesh like Tailscale, but with a management server you can host yourself: peers connect directly where they can, fall back to a relay where they can't, and access policies decide who reaches what. A good choice if you want the Tailscale experience with no outside account.

![Peers, the management server and a route into the LAN.](fig:app-overlay-vpn)

## What you end up with

- A self-hosted NetBird management server with its web dashboard
- Your servers and laptops as peers with stable `100.x` addresses
- A route to your LAN through one server, and policies per group

## Before you start

- A small VPS or a server with a **public IP** and a DNS name (for example `netbird.example.com`). Peers must reach it from anywhere.
- Docker, `jq` and `curl` on that machine.
- Open TCP 80 and 443 and UDP 3478 on its firewall, plus the other ports the setup script prints (relay and signal ports change between releases).

## Part 1 · The management server

- [ ] On the public machine, run the official quick-start script. It writes a compose stack with the management service, signal, relay, dashboard, Caddy for HTTPS and Zitadel as the identity provider:

```bash
export NETBIRD_DOMAIN=netbird.example.com
curl -fsSL https://github.com/netbirdio/netbird/releases/latest/download/getting-started-with-zitadel.sh | bash
```

- [ ] Save the admin credentials it prints in your password manager, then open `https://netbird.example.com`.
- [ ] Read the script before you run it, as with any pipe-to-shell installer. It works in the current folder; run it from a folder you keep, such as `/opt/stacks/netbird`.

## Part 2 · Peers

- [ ] Dashboard → **Setup keys** → create a reusable key for servers (auto-assigned group `servers`).
- [ ] On each Linux server:

```bash
curl -fsSL https://pkgs.netbird.io/install.sh | sh
sudo netbird up --management-url https://netbird.example.com --setup-key <KEY>
```

- [ ] On your own computers, install the desktop client and sign in interactively; they land in the `All` group until you move them.
- [ ] `netbird status -d` shows each peer and whether the connection is **P2P** or **Relayed**.

## Part 3 · Policies and LAN routes

- [ ] **Access control**: delete the default allow-all policy once you've added explicit ones, for example `admins → servers` on TCP 22 and 443.
- [ ] To reach a whole LAN: **Network routes** (called **Networks** in newer versions) → add `192.168.1.0/24` with a server on that LAN as the routing peer, distributed to the `admins` group.

#### Check

- [ ] From a laptop on mobile data, `ping` a server's NetBird address and SSH to a LAN-only machine through the route.

## In Nightgrid

- Nightgrid's built-in Tailscale only speaks Tailscale. For NetBird, run the NetBird client on the computer Nightgrid runs on, then add hosts by their NetBird address or name. Nightgrid uses the system's network like any SSH client.
- Add the dashboard address to [Services](view:services), and the management server as a host so its updates and disks are watched too.

## Limits and gotchas

- If the management server is down, existing connections keep working but new peers and changes wait until it's back. Back up its folder (it holds the database and keys).
- Peers behind strict NAT on both sides go through the relay; that works but is slower.
- Don't run NetBird and Tailscale with overlapping routes on the same machine; both use `100.64.0.0/10` addresses and the routing gets confusing.

## Related

- [Run Headscale](kb:apps/headscale)
- [ZeroTier virtual network](kb:apps/zerotier)
- [WireGuard VPN server](kb:apps/wireguard)
