# Deploy a new compose stack

Turn a container image into a running Docker Compose stack with its ports, volumes and secrets set up properly, then keep it healthy.

## What you end up with

- A folder on the host with `compose.yaml` and a mode-600 `.env`
- Secrets kept encrypted in the stack's **Vault**, not in `compose.yaml`
- A service check and a backup plan for the app

## Before you start

- A host with Docker and the compose plugin (see [Prepare a Linux Docker host](kb:apps/docker-host)).
- Your login in the `docker` group, or sudo (see [The docker group](kb:basics/docker-group)).
- The image name and tag you want (the app's docs list it).

## Part 1 · Create the stack

- [ ] [Docker](view:docker) → pick the host → **+ New stack**.
- [ ] Start from an image: Nightgrid pre-fills ports, volumes and variables the image declares. Or paste YAML from the app's docs.
- [ ] Keep data in folders next to the compose file (`./data:/data`), so the whole app lives in one folder you can back up.
- [ ] Pin a version tag (`:1.2.3` or `:2`) instead of `:latest`. DRIFT's compose review flags unpinned images.
- [ ] Bind admin ports to localhost (`127.0.0.1:8080:8080`) when a reverse proxy will front the app.

## Part 2 · Secrets

- [ ] Put passwords and API keys in the **Vault**. Paste `NAME=value` into the name field to fill both.
- [ ] Reference them in compose as `${NAME}`.
- [ ] **Deploy .env…** shows which names are added, changed or removed, then **Write .env to <host>** writes the file with mode 600 and **Up now** restarts the stack.

## Part 3 · Bring it up and check it

- [ ] Start the stack and open the container's logs (follow mode). **✦ Explain** sends the newest lines to DRIFT if something fails.
- [ ] Attach the compose file to DRIFT for a **compose review**: restart policies, unpinned images, ports open on all interfaces, inline secrets, privileged containers, `docker.sock`.

#### Check

- [ ] The container is running, and `docker compose ps` in a [Terminal](view:terminal) shows it healthy if it has a healthcheck.

## Part 4 · Make it production-ish

- [ ] Add a route in your reverse proxy (see [Manage reverse proxy routes](kb:howto/proxy-routes)).
- [ ] Add a service check in [Services](view:services) → **Add service** so you get an alert when it's down.
- [ ] Add the stack folder to your backups (see [Monitor backups and test restores](kb:howto/backups)).

## Limits and gotchas

- Docker publishes ports straight through iptables, past ufw. A port mapped as `8080:8080` is reachable from everywhere ufw would otherwise block. Bind to `127.0.0.1` or a LAN IP.
- Changing `.env` needs `docker compose up -d` to take effect; `restart` alone keeps old variables.
- Databases in containers: back them up with a dump, not by copying the live folder (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)).

## Related

- [Docker and compose basics](kb:basics/docker-compose)
- [Update containers without fear](kb:howto/container-updates)
- [Deploy stacks from git](kb:howto/stacks-from-git)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
