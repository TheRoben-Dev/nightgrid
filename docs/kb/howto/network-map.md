# Map your network and find exposed ports

Scan your LAN from one of your hosts, see every device with its maker and services, label the ones that aren't servers, and spot anything reachable from the internet.

![What the map shows: internet, router, your hosts and other devices.](fig:ng-network-map)

## What you end up with

- A map: internet, router, tailnet, your hosts and other devices
- Red lines for every way in from the internet: port forwards, public addresses, Funnels and tunnels
- Labels for TVs, phones and printers that DRIFT can answer questions about

## Part 1 · First scan

- [ ] [Network](view:network) (`Mod+0`). Select the machine to scan from: a host marked **◉** (it answers) or **This computer**.
- [ ] **Scan from …** reads that machine's neighbour table, mDNS (`avahi-browse`), Tailscale peers, router port forwards (`upnpc`), Tailscale Funnel and cloudflared tunnels. Tools that aren't installed are listed as notes.
- [ ] **Ping sweep** also pings every address in the subnet (private /24 or smaller) and tries a few TCP ports, to find quiet devices.
- [ ] For a fuller picture install the helpers on the scanning host: `sudo apt install avahi-utils miniupnpc`.

## Part 2 · What's exposed

- [ ] **Red** lines and the ▲ count are ways in from the internet. Nightgrid works these out from your own machines and never asks an outside service.
- [ ] Select a host to see how the internet reaches it and its listening ports. **Audit ports** (or on all hosts) marks exposed ones INTERNET.
- [ ] A UPnP forward you didn't make? An app opened it. Turn UPnP off on the router if you don't need it.

## Part 3 · New and missing devices

- [ ] Every device a scan sees goes into an **inventory**. First-time devices are marked **NEW**; with nothing selected, **New this scan** and **Missing** list what appeared and what didn't answer.
- [ ] **Trust** devices you expect and **Forget** ones that are gone. An unknown NEW device on your LAN is worth identifying.
- [ ] Networks behind a router, VPN or subnet route: type the range (for example `10.20.0.0/24`) and scan. Devices there show less detail, because MACs and local names don't cross a router.

## Part 4 · More than one network

- [ ] Scan from a machine on each network you have: home Wi-Fi from **This computer**, the server LAN from one of its hosts. Each scan lands in its own saved network (subnet plus the router's MAC), listed in the **NETWORKS** bar.
- [ ] **✎** gives each a name ("Home", "Servers"). Devices of one network never show as missing from another.
- [ ] Tick **on map** on the other network. It appears behind its own router; the **Internet** node above both routers, and the **tailnet** beside them, show how the two reach each other.
- [ ] Replaced a router? The network comes back as a new one: **✎ → Merge into** the old one.

## Part 5 · Arrange the map

- [ ] Drag nodes to place them; drop a device onto an access point, switch or host to say it connects through it (or use **Connected through** in its details). **Auto layout** forgets positions; **LIST** gives a compact view.
- [ ] **Save as expected** remembers the map. Later scans show missing devices as dashed **MISSING** boxes and unexpected ones with **+**.
- [ ] Want switches on the map? Install `lldpd` on the scanning host (`sudo apt install lldpd`). The next scan puts the switch and port it's plugged into between it and the router.

## Part 6 · Security checks and a watch

- [ ] The **SECURITY** bar above the map counts what the scan found: risky router port forwards, UPnP on, a second DHCP server, a public address on an interface.
- [ ] **Check web security** reads each device's web pages and certificates: expired or self-signed certificates, admin pages of gear that ships with default passwords, and logins over plain HTTP. It never tries a password.
- [ ] **Watch the network** repeats the scan from the selected scanner on a schedule and raises **NEW DEVICE**, **DEVICE MISSING** and **NETWORK RISK** alerts. Trust a device to silence its NEW alert.

## Part 7 · Label devices

- [ ] Select a device for its address, MAC, maker (looked up offline), names and services.
- [ ] **Label** it: a name, a type, `#tags` and a note. Labels follow the MAC and travel in profile exports.
- [ ] Ask DRIFT "what is 192.168.1.44?".
- [ ] A device you want to manage: **Add as host**.

#### Check

- [ ] Every device on the map has a label or is a host, and the ▲ count matches the ports you meant to open.

## Limits and gotchas

- Phones and VMs with private random MACs show "Random MAC" and may get a new MAC on reconnect, which loses their label.
- Docker containers aren't listed as devices; they're in [Docker](view:docker).
- Nothing is scanned until you click. Scanning from a host uses that host's tools and network position.

## Related

- [Raise a host's security score](kb:howto/security-score)
- [Cloudflare Tunnel](kb:apps/cloudflared)
- [Reach your servers away from home](kb:howto/remote-access)
