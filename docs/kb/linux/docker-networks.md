# Docker networks: bridge, shared proxy network and macvlan

Understand how containers talk to each other and to your LAN, set up one shared network for your reverse proxy, and give a container its own LAN address when it really needs one.

![Three network setups side by side.](fig:docker-networks)

## What you end up with

- Apps reachable by name from the proxy, with no published ports
- A macvlan network for the rare container that needs a LAN IP (Pi-hole, Home Assistant without host mode)

## Part 1 · The defaults

- [ ] Each compose stack gets its own bridge network; services reach each other by service name.
- [ ] Published ports (`8080:80`) open on every host interface unless you bind them (`127.0.0.1:8080:80`).

## Part 2 · A shared proxy network

```sh
docker network create proxy
```

- [ ] In each app's compose file:

```yaml
services:
  app:
    networks: [default, proxy]
networks:
  proxy:
    external: true
```

- [ ] The proxy (Caddy, Traefik) joins `proxy` too and targets `app:<port>`. The app needs no `ports:`.

## Part 3 · macvlan

```sh
docker network create -d macvlan --subnet=192.168.1.0/24 --gateway=192.168.1.1 --ip-range=192.168.1.240/28 -o parent=eth0 lan
```

- [ ] In compose: `networks: { lan: { ipv4_address: 192.168.1.241 } }` with `lan` declared `external: true`.
- [ ] Keep `--ip-range` outside the router's DHCP pool.

## Part 4 · In Nightgrid

- [ ] [System](view:system) → **security** shows which container owns each port and who can reach it.
- [ ] The [Network](view:network) map lists Docker networks from scans; containers stay in [Docker](view:docker).

#### Check

- [ ] `docker network inspect proxy` lists the proxy and your apps.

## Limits and gotchas

- With macvlan, the host itself can't reach the container's IP (a kernel limitation) without an extra macvlan interface on the host.
- Avoid overlapping subnets: Docker picks `172.17+` ranges; if your LAN or VPN uses them, set `default-address-pools` in `/etc/docker/daemon.json`.

## Related

- [Docker and compose basics](kb:basics/docker-compose)
- [Caddy reverse proxy](kb:apps/caddy)
- [Prepare a Linux Docker host](kb:apps/docker-host)
