# Jitsi Meet video calls

Your own video meeting server: open a link in a browser or the Jitsi apps, no accounts needed for guests, with screen sharing, chat and lobby. Good for family calls and small teams that don't want a cloud service.

## What you end up with

- The official Jitsi Meet Docker setup (web, Prosody, Jicofo, JVB)
- HTTPS on your domain, and media flowing on UDP 10000
- Meeting creation restricted to your accounts, guests by link

## Before you start

- A DNS name (`meet.example.com`) and the ability to forward UDP 10000 to the server. Calls with more than two people go through the server's video bridge on that port.
- 4 GB RAM and a decent upload: each participant receives every other video stream through your server.

## Part 1 · Get the release

- [ ] Download the latest stable release of the `docker-jitsi-meet` repository (the release zip from its GitHub releases page) and unpack it into `/opt/stacks/jitsi`.
- [ ] Set up the config:

```bash
cd /opt/stacks/jitsi
cp env.example .env
./gen-passwords.sh
mkdir -p ~/.jitsi-meet-cfg/{web,transcripts,prosody/config,prosody/prosody-plugins-custom,jicofo,jvb,jigasi,jibri}
```

## Part 2 · Configure `.env`

- [ ] Set at least:

```ini
PUBLIC_URL=https://meet.example.com
TZ=Etc/UTC
JVB_ADVERTISE_IPS=203.0.113.10
ENABLE_AUTH=1
AUTH_TYPE=internal
ENABLE_GUESTS=1
```

- [ ] `JVB_ADVERTISE_IPS` is your public IP (and optionally the LAN IP, comma-separated) so clients know where to send media.
- [ ] Behind your own reverse proxy, point it at the web container's HTTP port (`HTTP_PORT`, 8000 by default) with WebSocket support, and set `DISABLE_HTTPS=1`. Without a proxy, set `ENABLE_LETSENCRYPT=1` with `LETSENCRYPT_DOMAIN` and `LETSENCRYPT_EMAIL`.
- [ ] Start it: `docker compose up -d`.

## Part 3 · Accounts

- [ ] With `ENABLE_AUTH=1`, only registered users can start a meeting; guests join once it's started. Create a user in Prosody:

```bash
docker compose exec prosody prosodyctl --config /config/prosody.cfg.lua register alice meet.jitsi 'a long password'
```

#### Check

- [ ] Three people (one outside your network) join the same meeting with video, and the server's `jvb` container logs show the participants.

## In Nightgrid

- Add `https://meet.example.com` to [Services](view:services).
- The stack is plain compose, so it shows in [Docker](view:docker) with logs and restarts. Image updates come with new releases; keep `.env` from the release you installed and compare it with the new `env.example` when you update.
- [Network](view:network) → **Tools** → **Ports** from an outside host confirms UDP 10000 reaches the server.

## Limits and gotchas

- If video works for two people but not three, UDP 10000 isn't reaching the server or `JVB_ADVERTISE_IPS` is wrong (two-person calls go peer to peer).
- Very large meetings need more CPU and upload than a home line has; Jitsi scales with more bridges, not one bigger box.
- Keep all images on the same release tag; mixing versions breaks signalling.

## Related

- [Matrix chat with Synapse](kb:apps/matrix-synapse)
- [Mumble voice chat](kb:apps/mumble)
- [Caddy reverse proxy](kb:apps/caddy)
