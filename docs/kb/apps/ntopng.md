# ntopng traffic analysis

See who is using the bandwidth: ntopng watches a network interface and breaks traffic down by host, application (Netflix, Steam, Zoom, DNS…), country and flow, live and with history. Run it where the traffic passes: the router, a mirror port, or the server you want to understand.

## What you end up with

- ntopng Community in Docker watching one or more interfaces
- Top talkers, applications and flows per host
- Alerts for new devices, odd ports and blacklisted addresses

## Before you start

- Placement decides what you see. On an ordinary server ntopng sees only that server's traffic. For the whole LAN, run it on the router (OPNsense has an ntopng plugin), or on a box connected to a switch **mirror (SPAN) port**.
- A second NIC for the mirror port keeps the capture separate from the host's own traffic.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `ntopng`. It needs the host's network to see real interfaces:

```yaml
services:
  ntopng:
    image: ntop/ntopng:stable
    restart: unless-stopped
    network_mode: host
    command: ["-i", "eth0", "-w", "3000", "--community"]
    volumes:
      - ./data:/var/lib/ntopng
```

- [ ] Replace `eth0` with the interface name from `ip -br link`. Repeat `-i` for several interfaces.
- [ ] Open `http://<host>:3000`. Default login is `admin` / `admin`; it asks for a new password at once.

## Part 2 · Read it

- [ ] **Dashboard**: top hosts and applications right now.
- [ ] **Hosts**: click one for its traffic history, peers, applications and open ports it was seen using.
- [ ] **Flows**: live connections with application detection (nDPI); filter by host or protocol.
- [ ] **Settings → Preferences**: enable host and interface timeseries to keep history; it's stored in the `data` folder.

## Part 3 · Mirror port (optional)

- [ ] On a managed switch, mirror the router's uplink port to the port your ntopng box uses. Every packet to and from the internet is copied there.
- [ ] Bring that NIC up with no IP address (`ip link set eth1 up promisc on`) and point `-i` at it.

#### Check

- [ ] Start a big download on another machine and watch it top the **Hosts** list with the right application.

## In Nightgrid

- Nightgrid's [Network](view:network) map shows devices and ports; ntopng shows what they're actually sending. The **⧉** window in [Docker](view:docker) doesn't apply with host networking, so add `http://<host>:3000` to [Services](view:services) and open it from there.
- [Fleet](view:fleet) → **Speed** measures link capacity between hosts; ntopng shows what fills it.

## Limits and gotchas

- Community edition has limited history and no reports; it's still plenty for a home network.
- On a busy gigabit link, ntopng plus timeseries can use a full CPU core and several GB of disk a month. Keep retention short.
- HTTPS traffic is classified by name and fingerprint, not content; ntopng can't see what's inside.

## Related

- [NetAlertX device presence](kb:apps/netalertx)
- [SmokePing latency history](kb:apps/smokeping)
- [OPNsense firewall in a VM](kb:apps/opnsense)
