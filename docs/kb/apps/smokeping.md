# SmokePing latency history

Graphs latency and packet loss to a list of targets over weeks, with the "smoke" showing jitter. The tool for proving your ISP drops packets every evening, or that a Wi-Fi bridge is flaky.

## What you end up with

- Latency and loss to your router, ISP gateway, DNS servers and a few internet hosts
- Graphs from the last 3 hours to the last year
- Evidence to send your ISP

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `smokeping`, on a wired, always-on host:

```yaml
services:
  smokeping:
    image: lscr.io/linuxserver/smokeping:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
    volumes:
      - ./config:/config
      - ./data:/data
    ports:
      - "8086:80"
```

## Part 2 · Targets

- [ ] Edit `./config/Targets`. Keep the header the image created and add your own section:

```
+ Home
menu = Home
title = Home network

++ Router
host = 192.168.1.1

++ ISPGateway
host = 10.0.0.1

++ Quad9
host = 9.9.9.9

++ Cloudflare
host = 1.1.1.1
```

- [ ] Find the ISP gateway with a traceroute: the first hop after your router.
- [ ] Restart the stack and open `http://host:8086/smokeping/smokeping.cgi`.

#### Check

- [ ] After an hour the graphs show a line per target; the router should sit near 1 ms with no loss.

## In Nightgrid

- [Network](view:network) → **Tools** → **Ping** and **Path** answer "is it bad right now"; SmokePing answers "has it been bad every evening for a week".
- [Fleet](view:fleet) → **Speed** measures throughput between your hosts with iperf3, the other half of the story.

## Limits and gotchas

- Loss to the router while internet hosts look fine points at the LAN or Wi-Fi, not the ISP.
- Some hosts rate-limit ICMP; loss to them alone means little. Compare several targets.

## Related

- [Speedtest Tracker](kb:apps/speedtest-tracker)
- [Diagnose network problems with Network → Tools](kb:howto/network-tools)
- [Find slow links with iperf3](kb:howto/speed-test)
