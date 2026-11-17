# Netdata per-second monitoring

Hundreds of metrics per second on a host, auto-detected: every disk, NIC, container, systemd service and many apps, with anomaly detection and alerts built in. Useful when you need to see what happened in the last few seconds, not the last minute.

## What you end up with

- A dashboard per host on port 19999 with per-second charts
- Hundreds of preconfigured health alerts
- Optionally several hosts streaming to one parent

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `netdata` on the host to watch:

```yaml
services:
  netdata:
    image: netdata/netdata:stable
    restart: unless-stopped
    pid: host
    network_mode: host
    cap_add: [SYS_PTRACE, SYS_ADMIN]
    security_opt: [apparmor:unconfined]
    volumes:
      - ./config:/etc/netdata
      - ./lib:/var/lib/netdata
      - ./cache:/var/cache/netdata
      - /:/host/root:ro,rslave
      - /etc/passwd:/host/etc/passwd:ro
      - /etc/group:/host/etc/group:ro
      - /etc/os-release:/host/etc/os-release:ro
      - /proc:/host/proc:ro
      - /sys:/host/sys:ro
      - /var/log:/host/var/log:ro
      - /var/run/docker.sock:/var/run/docker.sock:ro
```

- [ ] Open `http://host:19999`. You can skip the cloud sign-in and use the local dashboard.

## Part 2 · Tune it

- [ ] Retention: edit `./config/netdata.conf` (`docker compose exec netdata edit-config netdata.conf`); the `[db]` section sets tiers and disk space.
- [ ] Silence alerts you don't care about: `edit-config health.d/<name>.conf`, set `to: silent`.
- [ ] Keep the dashboard on the LAN or tailnet: it shows a lot about the host and has no login by default.

#### Check

- [ ] Run `stress-ng --cpu 2 --timeout 30s` on the host: the CPU chart reacts within a second.

## In Nightgrid

- Nightgrid's own monitor samples less often but keeps 400 days of hourly history and alerts across all hosts; Netdata is the microscope for one host when something is odd right now.
- Open the dashboard through [Tunnels](view:tunnels) when away, instead of exposing port 19999.
- It costs CPU and RAM itself: check its own usage in [Docker](view:docker) before putting it on a Raspberry Pi.

## Limits and gotchas

- `network_mode: host` and `pid: host` give it a lot of visibility by design; only run images you trust this way.
- The newer dashboard nudges you towards Netdata Cloud; the local agent works without it.

## Related

- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [Beszel lightweight monitoring](kb:apps/beszel)
- [Why is this server slow?](kb:linux/slow-server)
