# Glances system monitor

A top-like monitor that also runs as a small web page and API: CPU, memory, disks, sensors, network, containers and processes in one screen. Handy as a quick look on a host and as a data source for dashboards like Homepage.

## What you end up with

- `glances` in any terminal for a one-screen overview
- A web view on port 61208
- An API other dashboards can read

## Part 1 · In a terminal

- [ ] `sudo apt install glances` (or `pipx install 'glances[all]'` for the newest), then run `glances`.
- [ ] Keys: `1` per-CPU, `d` disk I/O, `s` sensors, `D` containers, `q` quit.

## Part 2 · As a web service

- [ ] [Docker](view:docker) → **+ New stack** named `glances`:

```yaml
services:
  glances:
    image: nicolargo/glances:latest-full
    restart: unless-stopped
    pid: host
    network_mode: host
    environment:
      GLANCES_OPT: "-w"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /etc/os-release:/etc/os-release:ro
```

- [ ] Open `http://host:61208`. For a password, add `--password` to `GLANCES_OPT` and set it once interactively (see the image's docs).

#### Check

- [ ] `curl -s http://host:61208/api/4/cpu` returns JSON (older releases use `/api/3/`).

## In Nightgrid

- Nightgrid's [Overview](view:overview) already shows CPU, memory, disk, network and temperatures for every host; Glances is useful as a Homepage widget source or on hosts you don't add to Nightgrid.
- Run `glances` in a [Terminal](view:terminal) tab for a live look at one host; `h` shows its keys.

## Limits and gotchas

- The web view has no authentication unless you set a password; keep it on the LAN.
- `latest-full` includes optional plugins; `latest` is smaller.

## Related

- [Homepage dashboard](kb:apps/homepage)
- [Netdata per-second monitoring](kb:apps/netdata)
- [Why is this server slow?](kb:linux/slow-server)
