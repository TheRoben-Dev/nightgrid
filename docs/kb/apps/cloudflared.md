# Cloudflare Tunnel

Publish a web app to the internet through Cloudflare without opening any port on your router. The `cloudflared` connector dials out to Cloudflare and carries requests back.

## What you end up with

- `app.example.com` reachable from anywhere, with no port forward
- Optional Cloudflare Access login in front of it
- The tunnel shown on Nightgrid's network map as a way in from the internet

## Before you start

- A domain using Cloudflare's DNS.
- Know the trade-off: Cloudflare terminates TLS and sees the traffic. Fine for most apps; think twice for password managers and private photos.

## Part 1 · Create the tunnel

- [ ] In Cloudflare's dashboard: Zero Trust → Networks → Tunnels → create a tunnel, choose Docker, copy the token.
- [ ] [Docker](view:docker) → **+ New stack** named `cloudflared`:

```yaml
services:
  cloudflared:
    image: cloudflare/cloudflared:latest
    restart: unless-stopped
    command: tunnel --no-autoupdate run
    environment:
      TUNNEL_TOKEN: ${TUNNEL_TOKEN}
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Put `TUNNEL_TOKEN` in the stack's **Vault** and **Deploy .env…**.

## Part 2 · Publish a hostname

- [ ] In the tunnel's settings, add a public hostname: `app.example.com` → service `http`, URL `app:8080` (the container name on the `proxy` network).
- [ ] Protect admin apps with Zero Trust → Access → Applications (email one-time codes are free for small teams).

#### Check

- [ ] The site loads from mobile data, and the tunnel shows **HEALTHY** in Cloudflare.
- [ ] The [Network](view:network) map shows the tunnel as a red way in after **Scan from …** on that host.

## Part 3 · In Nightgrid

- [ ] Add the public URL in [Services](view:services) so outages alert.
- [ ] The container's logs (**✦ Explain**) are the first stop when the tunnel drops.

## Limits and gotchas

- Large uploads and video streaming through Cloudflare's free plan are against its terms for non-HTML content; use Tailscale for Jellyfin or Plex.
- The token is a credential: anyone with it can run your tunnel. Keep it in the Vault.
- Pin `cloudflared` to a version once it works.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [Reverse proxies](kb:basics/reverse-proxies)
- [Authentik single sign-on](kb:apps/authentik)
