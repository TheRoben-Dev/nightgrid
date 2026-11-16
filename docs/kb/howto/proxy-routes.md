# Manage reverse proxy routes

See every domain your reverse proxy serves, where it forwards and how long its certificate has left, and add or change Caddy and Traefik routes with a diff and a safe reload.

## What you end up with

- One list of domains, upstreams, containers and certificate days per host
- New sites added from Nightgrid, checked by the proxy before they go live
- Certificates feeding the expiry watch and its alerts

## Before you start

- A reverse proxy running on the host in Docker or as a service: Caddy, Traefik or Nginx Proxy Manager. New to this? Read [Reverse proxies](kb:basics/reverse-proxies) and pick one: [Caddy](kb:apps/caddy), [Traefik](kb:apps/traefik) or [Nginx Proxy Manager](kb:apps/nginx-proxy-manager).

## Part 1 · See the routes

- [ ] [Docker](view:docker) → **Routes** on the proxy's host.
- [ ] Each row: domain, where it forwards, the container behind it and certificate days left.
- [ ] Opening Routes once also adds the certificates to **Expiry watch** in [Services](view:services).

## Part 2 · Caddy

- [ ] **Edit routes…** → **Add site…**: the domains and where to send them (`app:8080` for a container on the same Docker network, or an address and port).
- [ ] **Change…** where a site sends traffic, **Remove…** a site, or **Edit as text** for anything else.
- [ ] Review the diff, then **Save and reload Caddy**. Caddy validates the file first and puts the old one back if it can't load it.

## Part 3 · Traefik

- [ ] Routes you add go to `nightgrid.yml` in Traefik's file provider directory, which Traefik watches.
- [ ] If Traefik has no file provider yet, add `--providers.file.directory=/etc/traefik/dynamic` and a volume for that folder (see [Traefik reverse proxy](kb:apps/traefik)).
- [ ] Routes defined by container labels are changed in their compose files, not here.

#### Check

- [ ] The new domain answers in a browser with a valid certificate.
- [ ] Add a service check for it in [Services](view:services) → **Add service**.

## Part 4 · When a site returns 502 or 504

- [ ] The service check names who answered: *HTTP 502 from caddy* means the proxy is fine but can't reach the app.
- [ ] Check the upstream address in Routes, that the app's container is running, and that both share a Docker network.

## Limits and gotchas

- Nginx Proxy Manager routes are shown but edited in NPM's own UI.
- A wildcard certificate via DNS challenge needs a Caddy build with your DNS provider's module, or Traefik's DNS resolver. Nightgrid edits routes, not ACME settings.
- Keep the proxy's admin API and dashboards off the public internet.

## Related

- [Reverse proxies](kb:basics/reverse-proxies)
- [Caddy reverse proxy](kb:apps/caddy)
- [Traefik reverse proxy](kb:apps/traefik)
- [Watch services and expiry dates](kb:howto/service-checks)
