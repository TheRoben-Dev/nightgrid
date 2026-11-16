# Beszel lightweight monitoring

A small hub-and-agent monitor with history for CPU, memory, disk, network, temperatures and Docker containers, plus alerts. An option when you want server-side history that runs 24/7 without Prometheus.

## What you end up with

- A hub web UI with history for every host
- A tiny agent per host
- Alerts by email or webhook while your computer is off

## Part 1 · Hub

- [ ] [Docker](view:docker) → **+ New stack** named `beszel`:

```yaml
services:
  beszel:
    image: henrygd/beszel:latest
    restart: unless-stopped
    volumes:
      - ./data:/beszel_data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route it to `beszel:8090` and create the admin account.

## Part 2 · Agents

- [ ] In the hub, **Add system**: it shows a compose snippet with the hub's public key for the agent. Deploy that on each host with **+ New stack** (the agent uses host networking on port 45876).
- [ ] Allow 45876 only from the hub's host in [System](view:system) → **FIREWALL**.

## Part 3 · With Nightgrid

- [ ] Nightgrid already keeps a year of host history while it runs. Beszel's value is history and alerts while Nightgrid is closed.

#### Check

- [ ] Every host shows up with charts.

## Limits and gotchas

- Agents mount the Docker socket read-only for container stats.

## Related

- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [Uptime history and monthly reports](kb:howto/uptime-reports)
