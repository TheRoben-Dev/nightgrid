# Portainer alongside Nightgrid

Portainer is a web UI for Docker. You don't need it for Nightgrid, but if you already use it (or want a browser UI for others), here's how to run it safely and link it from Nightgrid.

![Portainer with the local socket and a remote agent.](fig:app-portainer)

## What you end up with

- Portainer CE on HTTPS, reachable only on your LAN or tailnet
- "Open in Portainer" links on Nightgrid's Services board

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `portainer`:

```yaml
services:
  portainer:
    image: portainer/portainer-ce:lts
    restart: unless-stopped
    ports:
      - "127.0.0.1:9443:9443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./data:/data
```

- [ ] Open it through a saved tunnel in [Tunnels](view:tunnels) (local 9443 → `127.0.0.1:9443`) within 5 minutes of starting, and create the admin user (it locks itself otherwise; restart to retry).

## Part 2 · Other hosts

- [ ] Add more Docker hosts with the Portainer Agent on each (port 9001, LAN-only).

## Part 3 · In Nightgrid

- [ ] [Services](view:services) → **Add integration** → **Portainer** for links to containers.
- [ ] Nightgrid's own Docker view covers logs, shells, stacks, updates with snapshots and rollback over SSH, with no socket exposed.

#### Check

- [ ] The Portainer card links open the right container pages.

## Limits and gotchas

- Portainer with the Docker socket is root on the host. Never expose it to the internet.
- Stacks created in Portainer live in its own data folder, not as compose files on disk. Nightgrid lists them as compose projects but you edit them in Portainer.

## Related

- [Deploy a new compose stack](kb:howto/deploy-stack)
- [The docker group](kb:basics/docker-group)
