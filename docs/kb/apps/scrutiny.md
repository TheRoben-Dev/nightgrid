# Scrutiny SMART dashboard

A web dashboard for disk health that tracks SMART attributes over time and compares them to real-world failure rates. Useful next to Nightgrid's SMART checks when you want long attribute history per drive.

![Collectors on each host, history in one place.](fig:app-scrutiny)

## What you end up with

- Per-drive history of temperatures and failure-predicting attributes
- A collector on each host sending data to one web UI

## Part 1 · All-in-one on one host

- [ ] [Docker](view:docker) → **+ New stack** named `scrutiny`:

```yaml
services:
  scrutiny:
    image: ghcr.io/analogj/scrutiny:master-omnibus
    restart: unless-stopped
    cap_add: [SYS_RAWIO]
    ports:
      - "127.0.0.1:8080:8080"
    volumes:
      - /run/udev:/run/udev:ro
      - ./config:/opt/scrutiny/config
      - ./influxdb:/opt/scrutiny/influxdb
    devices:
      - /dev/sda
      - /dev/sdb
```

- [ ] List every disk under `devices` (NVMe drives as `/dev/nvme0`, plus `cap_add: [SYS_ADMIN]`).
- [ ] Open it through a saved tunnel in [Tunnels](view:tunnels).

## Part 2 · More hosts

- [ ] Run the `collector` image on other hosts with `COLLECTOR_API_ENDPOINT` pointing at the main instance (port 8080 reachable from the LAN for that).

## Part 3 · With Nightgrid

- [ ] Nightgrid already alerts on SMART warnings and degraded pools ([Keep an eye on disks and pools](kb:howto/disks)); Scrutiny adds the long attribute history.

#### Check

- [ ] Every disk shows "Passed" with attribute history after a day.

## Limits and gotchas

- USB enclosures often hide SMART from both tools.

## Related

- [Keep an eye on disks and pools](kb:howto/disks)
- [Prometheus and Grafana](kb:apps/prometheus-grafana)
