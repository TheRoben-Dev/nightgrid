# Caddy reverse proxy

Put all your web apps behind one Caddy container with automatic HTTPS, then add and change sites from Nightgrid's route editor.

![One proxy in front, apps on internal ports behind it.](fig:reverse-proxy)

## What you end up with

- Caddy on ports 80 and 443 with Let's Encrypt certificates renewed by itself
- Apps reached by name (`photos.example.com`) instead of ports
- Routes, upstreams and certificate days listed in Docker → Routes

## Before you start

- A domain you control, with DNS records pointing at the server (public IP for internet-facing sites).
- For LAN-only sites: either a DNS challenge (needs a Caddy build with your DNS provider's module) or Caddy's internal CA (`tls internal`).
- Ports 80 and 443 free on the host.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `caddy`, paste:

```yaml
services:
  caddy:
    image: caddy:2
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
      - "443:443/udp"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - ./data:/data
      - ./config:/config
    networks: [proxy]
networks:
  proxy:
    name: proxy
```

- [ ] Create `Caddyfile` next to it with [Files](view:files):

```
{
    email you@example.com
}

app.example.com {
    reverse_proxy app:8080
}
```

- [ ] Put each app's container on the `proxy` network (add `networks: [proxy]` and the same `networks:` block with `external: true` in its compose), so Caddy reaches it by service name and the app needs no published port.

## Part 2 · Manage routes in Nightgrid

- [ ] [Docker](view:docker) → **Routes** → **Edit routes…** → **Add site…**: domains and target (`app:8080`).
- [ ] Review the diff and **Save and reload Caddy**. Caddy validates first and keeps the old config if the new one fails.

## Part 3 · Common additions

- [ ] LAN-only site with Caddy's own CA: add `tls internal` inside the site block (devices must trust Caddy's root certificate from `./data/caddy/pki`).
- [ ] Restrict a site to your LAN and tailnet:

```
admin.example.com {
    @outside not remote_ip 192.168.0.0/16 100.64.0.0/10
    respond @outside 403
    reverse_proxy app:9000
}
```

- [ ] Single sign-on in front of an app: see [Authelia](kb:apps/authelia) or [Authentik](kb:apps/authentik).

#### Check

- [ ] The site loads with a valid certificate, and Docker → Routes shows its certificate days.
- [ ] Add it to [Services](view:services) for up/down checks.

## Limits and gotchas

- Let's Encrypt needs port 80 or 443 reachable from the internet for the default challenge. LAN-only names need the DNS challenge or `tls internal`.
- `caddy:2` doesn't include DNS provider modules; build one with `xcaddy` or use a community image that does.
- Behind Cloudflare's proxy, use Cloudflare's "Full (strict)" mode so traffic is HTTPS end to end.

## Related

- [Reverse proxies](kb:basics/reverse-proxies)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
- [Traefik reverse proxy](kb:apps/traefik)
- [Authelia single sign-on](kb:apps/authelia)
