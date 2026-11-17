# ntfy push notifications

Self-host ntfy so alerts from Nightgrid, scripts, backups and Uptime Kuma land on your phone, through topics only you can read.

![Notifications from Nightgrid and scripts to your phone.](fig:app-ntfy)

## What you end up with

- ntfy at `ntfy.example.com` with logins and private topics
- Nightgrid alerts on your phone via its ntfy webhook
- A one-liner any script can use to notify you

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `ntfy`:

```yaml
services:
  ntfy:
    image: binwiederhier/ntfy:latest
    restart: unless-stopped
    command: serve
    environment:
      NTFY_BASE_URL: https://ntfy.example.com
      NTFY_CACHE_FILE: /var/cache/ntfy/cache.db
      NTFY_AUTH_FILE: /var/lib/ntfy/auth.db
      NTFY_AUTH_DEFAULT_ACCESS: deny-all
      NTFY_BEHIND_PROXY: "true"
      NTFY_UPSTREAM_BASE_URL: https://ntfy.sh
    volumes:
      - ./cache:/var/cache/ntfy
      - ./lib:/var/lib/ntfy
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `ntfy.example.com` to `ntfy:80`.
- [ ] `NTFY_UPSTREAM_BASE_URL` lets the iOS app get instant notifications through ntfy.sh's push relay (only a message id passes through it).

## Part 2 · Users and topics

- [ ] In a [Terminal](view:terminal):

```sh
docker exec -it ntfy-ntfy-1 ntfy user add --role=admin you
docker exec -it ntfy-ntfy-1 ntfy token add you
```

- [ ] Subscribe in the phone app to `https://ntfy.example.com/homelab` with your login.

## Part 3 · Send from things

- [ ] Nightgrid: **Settings → Alerts → WEBHOOK → ntfy**, URL `https://ntfy.example.com/homelab`. Authenticated servers take the token as a query parameter: `?auth=` plus the base64 of `Bearer <token>` (ntfy's docs show how to build it). **Send test**.
- [ ] From a script or timer:

```sh
curl -H "Authorization: Bearer $NTFY_TOKEN" -d "Backup finished on $(hostname)" https://ntfy.example.com/homelab
```

- [ ] Uptime Kuma and many apps have ntfy built in.

#### Check

- [ ] Nightgrid's **Send test** reaches the phone.

## Limits and gotchas

- With `deny-all`, every publisher needs a token. Give each machine its own so you can revoke one.
- Messages are cached for 12 hours by default; the server isn't an archive.

## Related

- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Uptime Kuma](kb:apps/uptime-kuma)
- [Send mail from your servers](kb:apps/mail-relay)
