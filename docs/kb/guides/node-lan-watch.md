# Watch the home network

An always-on server sees the whole LAN. With [Nightgrid Node](kb:basics/nightgrid-node) on it, you get a running inventory of devices and a heads-up when something new joins, an address is fought over, the router suddenly answers from another MAC, or a second DHCP server starts handing out addresses.

## What you end up with

- An inventory of LAN devices, built from the kernel's neighbour table (no scanning, nothing sent)
- Timeline events for new devices, moved addresses, IP conflicts and a gateway that changes its MAC (replaced router or ARP spoofing)
- With the listen-only helper: the DHCP servers on the LAN, a rogue one next to the real one, changed router or DNS settings, and device names

## Before you start

- Nightgrid Node on one always-on host per LAN segment ([Install Nightgrid Node](kb:howto/node-install))
- That host wired to the LAN you want to watch (Wi-Fi works, but sees less)

## Part 1 · The inventory

- [ ] Nothing to grant: `[network] enabled = true` is the default, reading `/proc/net/arp` every 30 s.
- [ ] `nightgrid-node query net.devices | jq` lists what the host has talked to or heard from. The list grows as devices talk; devices that never touch this host's subnet won't appear.
- [ ] Container bridges (`docker0`, `br-*`, `veth*`) are always ignored. Ignore more with `[network] ignore_ifaces`, for example a VPN interface.
- [ ] For a full picture right now, Nightgrid's [Network](view:network) map can scan from this host; the node keeps the history between scans.

## Part 2 · DHCP watch (optional)

- [ ] `sudo nightgrid-node allow netwatch` starts a second, listen-only process with `CAP_NET_RAW`: a kernel filter passes it ARP and DHCP only, and it never sends a packet.
- [ ] List the DHCP servers you expect in `[network] dhcp_servers`, for example `["192.168.1.1"]`, and restart the node. A server not listed that shows up next to another one is reported as possibly rogue.
- [ ] `nightgrid-node query net.dhcp | jq` shows servers seen with the router and DNS they hand out. Device names from DHCP requests fill in `net.devices`.

## Part 3 · What the events mean

- [ ] **new device 192.168.1.23** (`aa:bb:cc:… on eth0`): a guest's phone, a new IoT plug, or something you didn't plug in. More than 20 at once arrive as "N new devices" (usually the first run).
- [ ] **device moved 192.168.1.20 → 192.168.1.21**: a new DHCP lease. Normal; give servers static leases.
- [ ] **address 192.168.1.50 answers from a new MAC**: a device was replaced, or two devices share an address.
- [ ] **IP conflict on 192.168.1.50** (warning; critical when it's the gateway): the address keeps switching between MACs. Two devices with the same static IP, or spoofing.
- [ ] **gateway 192.168.1.1 answers from a new MAC**: you replaced the router, or someone is ARP-spoofing the gateway to sit in the middle. If you didn't change the router, find the MAC's device right away.
- [ ] **another DHCP server on the LAN: 192.168.1.66**: usually a second router or a mesh node left in router mode. Clients that pick it get the wrong gateway or DNS. Switch that device to access-point/bridge mode.
- [ ] **DHCP server 192.168.1.1 hands out a different router / DNS**: someone changed the router's settings, or Pi-hole/AdGuard was added or removed as DNS.

## Part 4 · Make it useful

- [ ] Follow along: `nightgrid-node watch events` while you add a device, and see it appear.
- [ ] The daily summary (`summary.daily`) gets a `Network` line when anything changed.
- [ ] Name your devices in Nightgrid's [Network](view:network) map so unknown ones stand out.

## Notes

- Privacy: the node stores MAC and IPv4 addresses of LAN devices in its own database; `uninstall --purge` removes them.
- One watching host per broadcast domain is enough. On VLANs, put a node in each VLAN you care about, or a host with an interface in each.
