# Run Headscale

Host your own Tailscale control server with Headscale, so your tailnet's coordination never leaves your hardware. The normal Tailscale clients (and Nightgrid's built-in one) connect to it.

![Headscale coordinates; devices talk to each other directly.](fig:app-headscale)

## What you end up with

- A Headscale server behind HTTPS on a public name
- Your devices and servers joined with pre-auth keys
- Nightgrid's built-in Tailscale pointed at it

## Before you start

- A small server with a public address (a cheap VPS is common) and a domain name for it, such as `hs.example.com`.
- A reverse proxy with HTTPS in front of it (see [Caddy reverse proxy](kb:apps/caddy)).

## Part 1 · Install

- [ ] [Docker](view:docker) → **+ New stack** named `headscale`:

```yaml
services:
  headscale:
    image: headscale/headscale:stable
    restart: unless-stopped
    command: serve
    volumes:
      - ./config:/etc/headscale
      - ./data:/var/lib/headscale
    ports:
      - "127.0.0.1:8080:8080"
```

- [ ] Copy the example `config.yaml` from Headscale's repository for your version into `./config`, and set at least `server_url` to your public HTTPS name, `listen_addr: 0.0.0.0:8080`, and a `base_domain` for MagicDNS (it must differ from `server_url`'s domain).
- [ ] Proxy it with Caddy: a site `hs.example.com` with `reverse_proxy 127.0.0.1:8080` (or the container name on a shared network).

## Part 2 · Users and keys

- [ ] Create a user and a reusable key (run in a [Terminal](view:terminal) on the host):

```sh
docker exec headscale-headscale-1 headscale users create home
docker exec headscale-headscale-1 headscale users list
docker exec headscale-headscale-1 headscale preauthkeys create --user <user-id> --reusable --expiration 24h
```

- [ ] Older Headscale versions take the user name instead of the id; `headscale preauthkeys create --help` shows which.
- [ ] On each Linux machine: `sudo tailscale up --login-server https://hs.example.com --authkey <key>`. {check: tailscale}

## Part 3 · Nightgrid

- [ ] **Settings → Connections → Advanced**: set the control server to your Headscale URL. Saving restarts the client. Then **LOG IN** and approve the node with `headscale nodes register` as the login page tells you.
- [ ] Subnet routes: approve with `headscale nodes approve-routes` (or `headscale routes enable` on older versions).

#### Check

- [ ] `headscale nodes list` shows your devices online, and Nightgrid's **DEVICES** list matches.

## Limits and gotchas

- Headscale implements most, not all, of Tailscale's features. Check its docs for Tailscale SSH and Funnel support before relying on them.
- Back up `./data` (the database and keys). Losing it means re-joining every device.
- Pin the image to a version; config options change between releases.

## Related

- [Reach your servers away from home](kb:howto/remote-access)
- [Tailscale subnet router](kb:apps/tailscale-subnet-router)
- [Tailscale and WireGuard](kb:basics/tailscale-wireguard)
