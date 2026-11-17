# Reach your servers away from home

Use Nightgrid's built-in Tailscale client so your hosts work from a café or a phone hotspot, without installing the Tailscale app on this computer and without opening ports at home.

## What you end up with

- Nightgrid joined to your tailnet as its own device
- Hosts saved with their LAN addresses that still work away from home, through a subnet router
- Optional per-host tailnet fallback addresses, tried automatically when the LAN address doesn't answer

## Before you start

- A Tailscale account (or a self-hosted Headscale, see [Run Headscale](kb:apps/headscale)).
- One always-on Linux machine at home to act as a subnet router (see [Tailscale subnet router](kb:apps/tailscale-subnet-router)).

## Part 1 · Join the tailnet

- [ ] **Settings → Connections → Built-in Tailscale**: turn it on.
- [ ] **LOG IN** opens the login page in your browser. When it finishes, the state shows **Connected** with Nightgrid's tailnet name and address.
- [ ] If your tailnet requires device approval, approve Nightgrid in the admin console.
- [ ] **DEVICES** lists the tailnet; click an address to copy it.

## Part 2 · Reach the LAN through a subnet router

- [ ] On the home server, advertise the LAN and approve the route in the admin console.
- [ ] Leave **Use subnet routes** on. Nightgrid tries the LAN first and the tailnet when the LAN doesn't answer, so one saved address works everywhere.

#### Check

- [ ] On a phone hotspot, open a terminal on a host saved with its `192.168.x.x` address. It connects.

## Part 3 · Per-host fallback

- [ ] For hosts that run Tailscale themselves, put their 100.x address or MagicDNS name in **Tailnet address** in the host editor.
- [ ] Nightgrid remembers which address worked. The host key is always checked against the main address, so switching never raises a host-key warning.

## Part 4 · Tailscale SSH

- [ ] Hosts with Tailscale SSH enabled work with **SSH agent** as they are; the tailnet policy decides who gets in. See [Use Tailscale SSH hosts](kb:howto/tailscale-ssh).

## When Nightgrid is closed

- The subnet router and the tailnet keep working for your other devices. Nightgrid's own tailnet device goes offline until the app starts.

## Limits and gotchas

- The route must be approved in the admin console, or nothing behind the router is reachable.
- Two sites using the same subnet (both `192.168.1.0/24`) can't both be routed. Renumber one of them.
- Built-in Tailscale off: tailnet addresses are reached through the system Tailscale app if it's connected.
- Privacy: only Tailscale's control server and relays (or your Headscale) are contacted, and only while the feature is on.

## Related

- [Tailscale subnet router](kb:apps/tailscale-subnet-router)
- [Run Headscale](kb:apps/headscale)
- [Use Tailscale SSH hosts](kb:howto/tailscale-ssh)
- [WireGuard VPN server](kb:apps/wireguard)
