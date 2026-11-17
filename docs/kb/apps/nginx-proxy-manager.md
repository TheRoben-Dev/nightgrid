# Nginx Proxy Manager

A reverse proxy you click together in a web UI, with Let's Encrypt built in. Good if you prefer forms to config files.

![One proxy in front, apps on internal ports behind it.](fig:reverse-proxy)

## What you end up with

- Proxy hosts, redirects and certificates managed in NPM's UI
- Routes and certificate days visible in Nightgrid's Docker → Routes

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `npm`:

```yaml
services:
  npm:
    image: jc21/nginx-proxy-manager:latest
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
      - "127.0.0.1:81:81"
    volumes:
      - ./data:/data
      - ./letsencrypt:/etc/letsencrypt
    networks: [proxy]
networks:
  proxy:
    name: proxy
```

- [ ] The admin UI is on port 81, bound to localhost. Reach it through a saved tunnel in [Tunnels](view:tunnels): local port 8181 → `127.0.0.1:81` on the host.
- [ ] Log in and create your admin account as the first-run screen asks.

## Part 2 · Add a proxy host

- [ ] Hosts → Proxy Hosts → Add: domain, scheme, forward hostname (the container name on the `proxy` network) and port.
- [ ] SSL tab: request a new certificate, force SSL.
- [ ] Turn on **Block common exploits** and **Websockets support** where the app needs it.

## Part 3 · In Nightgrid

- [ ] [Docker](view:docker) → **Routes** lists NPM's hosts, where they forward and certificate days. Editing stays in NPM's UI.
- [ ] Add each site to [Services](view:services) for checks and expiry alerts.

#### Check

- [ ] The site loads with a valid certificate.

## Limits and gotchas

- Pin the image to a version tag after setup; `latest` updates can change the UI or database.
- Back up `./data` and `./letsencrypt` together.
- Advanced settings (custom nginx snippets) are powerful but easy to break; test after each change.

## Related

- [Reverse proxies](kb:basics/reverse-proxies)
- [Caddy reverse proxy](kb:apps/caddy)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
