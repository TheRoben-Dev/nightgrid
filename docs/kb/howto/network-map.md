# Map your network and find exposed ports

Scan your LAN from one of your hosts, see every device with its maker and services, label the ones that aren't servers, and spot anything reachable from the internet.

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

## Part 3 · Label devices

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
