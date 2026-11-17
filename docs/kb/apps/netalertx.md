# NetAlertX device presence

Scans your LAN on a schedule and tells you when a new device joins, a known one disappears, or a device's address changes. Useful as a 24/7 watch alongside Nightgrid's on-demand network map.

## What you end up with

- A device list with first seen, last seen and online history
- Notifications for new and missing devices
- Presence of phones as a "who's home" signal (optional)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `netalertx`, on a host wired into the LAN you want to watch:

```yaml
services:
  netalertx:
    image: ghcr.io/jokob-sk/netalertx:latest
    restart: unless-stopped
    network_mode: host
    environment:
      TZ: Etc/UTC
      PORT: "20211"
    volumes:
      - ./config:/app/config
      - ./db:/app/db
```

- [ ] Host networking lets it send ARP scans. Open `http://host:20211`.

## Part 2 · Set up

- [ ] Settings → **ARPSCAN**: set the subnet and interface (for example `192.168.1.0/24 --interface=eth0`) and a schedule.
- [ ] Mark known devices with names and owners; set **Alert down** on the ones that should always be on (NAS, printer, access points).
- [ ] Notifications: pick a publisher (ntfy, email, webhook and more) in Settings.

#### Check

- [ ] Connect a phone that's never been on the network: it shows as new within one scan.

## In Nightgrid

- [Network](view:network) → **Watch the network** does scheduled scans with NEW DEVICE and DEVICE MISSING alerts from one of your hosts, with no extra service. NetAlertX adds longer presence history and its own notification channels.
- Don't run both scanning the same subnet every minute; pick reasonable intervals.

## Limits and gotchas

- Phones with private (random) MAC addresses show up as new devices on each network and sometimes each day. Turn private addresses off for your home network on devices you want to recognise.
- ARP only sees the local subnet; other VLANs need a scanner in each.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [ntfy push notifications](kb:apps/ntfy)
- [Pi-hole](kb:apps/pihole)
