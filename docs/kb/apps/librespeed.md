# LibreSpeed LAN speed test

A speed test page hosted on your own server, so you measure your LAN and Wi-Fi instead of your internet line. Open it on a phone in each room to find the dead spots.

## What you end up with

- A browser speed test to your server from any device
- Optional results history

## Part 1 · The stack

- [ ] Run it on a host with a wired connection at the speed you want to test (a 2.5 GbE server can show 2.5 GbE Wi-Fi 7 results; a 1 GbE one can't).
- [ ] [Docker](view:docker) → **+ New stack** named `librespeed`:

```yaml
services:
  librespeed:
    image: lscr.io/linuxserver/librespeed:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
      PASSWORD: ${STATS_PASSWORD}
    volumes:
      - ./config:/config
    ports:
      - "8089:80"
```

- [ ] `PASSWORD` protects the results page at `/results/stats.php`.

## Part 2 · Measure

- [ ] Open `http://host:8089` on a phone next to the access point, then in each room. Note download, upload and jitter.
- [ ] Repeat from a wired laptop to get the baseline for the server's own link.

#### Check

- [ ] Wired results come close to the link speed (around 940 Mbit/s on gigabit). If not, the problem is the server, cable or switch, not Wi-Fi.

## In Nightgrid

- [Fleet](view:fleet) → **Speed** measures between two of your hosts with iperf3; LibreSpeed covers phones, tablets and TVs that you can't SSH into.
- Use [Network](view:network) → **Tools** → **Ping** from the same host to check latency to a device that tests slow.

## Limits and gotchas

- Browser tests top out around a few Gbit/s; for 10 GbE use iperf3.
- Run it over plain HTTP on the LAN: going through a reverse proxy or tunnel measures the proxy too.

## Related

- [Find slow links with iperf3](kb:howto/speed-test)
- [Speedtest Tracker](kb:apps/speedtest-tracker)
- [SmokePing latency history](kb:apps/smokeping)
