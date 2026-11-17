# Traefik reverse proxy

Run Traefik v3 with Docker labels for automatic routes and a file provider folder that Nightgrid's route editor can write to.

## What you end up with

- Routes from container labels, picked up as containers start
- Extra routes (non-Docker targets) in a watched folder, editable from Nightgrid
- Automatic Let's Encrypt certificates

## Before you start

- A domain with DNS pointing at the host, ports 80 and 443 free.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `traefik`:

```yaml
services:
  traefik:
    image: traefik:v3
    restart: unless-stopped
    command:
      - --providers.docker=true
      - --providers.docker.exposedbydefault=false
      - --providers.file.directory=/etc/traefik/dynamic
      - --providers.file.watch=true
      - --entrypoints.web.address=:80
      - --entrypoints.web.http.redirections.entrypoint.to=websecure
      - --entrypoints.websecure.address=:443
      - --certificatesresolvers.le.acme.email=you@example.com
      - --certificatesresolvers.le.acme.storage=/letsencrypt/acme.json
      - --certificatesresolvers.le.acme.tlschallenge=true
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./letsencrypt:/letsencrypt
      - ./dynamic:/etc/traefik/dynamic
    networks: [proxy]
networks:
  proxy:
    name: proxy
```

## Part 2 · Routes from labels

- [ ] On an app's service (joined to the `proxy` network):

```yaml
    labels:
      - traefik.enable=true
      - traefik.http.routers.app.rule=Host(`app.example.com`)
      - traefik.http.routers.app.entrypoints=websecure
      - traefik.http.routers.app.tls.certresolver=le
      - traefik.http.services.app.loadbalancer.server.port=8080
```

## Part 3 · Routes from Nightgrid

- [ ] [Docker](view:docker) → **Routes** → **Edit routes…**: routes you add go to `nightgrid.yml` in the `dynamic` folder, and Traefik loads them by itself.
- [ ] Use this for targets that aren't containers (a NAS UI, a Home Assistant VM).

#### Check

- [ ] Docker → Routes lists both label routes and file routes with certificate days.

## Limits and gotchas

- The Docker socket gives Traefik root-equivalent access. Mount it read-only, or use a socket proxy container that only allows reads.
- Don't expose the Traefik dashboard (`--api.insecure`) to the network.
- Router names must be unique across labels and files, or one silently wins.

## Related

- [Reverse proxies](kb:basics/reverse-proxies)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
- [Caddy reverse proxy](kb:apps/caddy)
