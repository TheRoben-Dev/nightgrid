# Reverse proxies

A reverse proxy sits in front of your apps and routes requests by name: `photos.example.com` goes to Immich, `git.example.com` to Forgejo, all on port 443 with one certificate setup. Apps then don't need their own ports open or their own TLS.

![One proxy on 443 in front of every app, each app listening only on 127.0.0.1.](fig:reverse-proxy)

## What it does for you

- **One entry point**: only 80 and 443 are open; everything else listens on localhost or a Docker network.
- **TLS certificates**: Caddy and Traefik get and renew Let's Encrypt certificates on their own.
- **Names instead of ports**: `https://jellyfin.home.example` instead of `http://192.168.1.20:8096`.
- **A place for access rules**: allow only the LAN or the tailnet, add basic auth or single sign-on.

## The common choices

- **Caddy**: a short config file, HTTPS by default. Good first choice.
- **Traefik**: reads routes from Docker labels, so each stack declares its own route.
- **Nginx Proxy Manager**: a web UI over nginx, handy if you prefer clicking.

A Caddy example:

```
photos.home.example {
	reverse_proxy 127.0.0.1:2283
}
git.home.example {
	reverse_proxy 127.0.0.1:3000
}
```

## Certificates at home

Let's Encrypt has to verify you own the domain. With an HTTP challenge, port 80 must be reachable from the internet. For services you only use at home or over your tailnet, use the **DNS challenge** instead: the proxy proves ownership through your DNS provider's API, and nothing has to be exposed. Tailscale can also issue certificates for `*.ts.net` names.

## In Nightgrid

- [Docker](view:docker) → **ROUTES** reads Caddy, Traefik and Nginx Proxy Manager routes and maps each domain to its container, with certificate expiry. The route editor adds or edits Caddy and Traefik routes with a reload check.
- Certificate expiry also feeds Services → **EXPIRY WATCH** and alerts.
- The network map marks proxy routes as internet-facing when the proxy is reachable from outside.

## Pitfalls

- An app that builds its own links needs to know its public URL (often an env var like `APP_URL`), or redirects go to the internal address.
- WebSockets and large uploads may need extra proxy settings (nginx's `client_max_body_size`).
- Bind the apps behind the proxy to localhost (`127.0.0.1:port`) so they can't be reached around it.

## Related

- [Tailscale and WireGuard](kb:basics/tailscale-wireguard): keep private apps off the internet entirely.
- [Docker and compose basics](kb:basics/docker-compose): binding ports to localhost.
