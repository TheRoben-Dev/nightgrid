# Healthchecks dead man's switch

Get alerted when a scheduled job does *not* run: backups, scrubs, certificate renewals. Each job pings a URL when it finishes; Healthchecks alerts when a ping is late.

![Dead man's switch: jobs ping, silence alerts.](fig:app-healthchecks)

## What you end up with

- A self-hosted Healthchecks instance with a check per job
- Alerts for jobs that silently stopped, failed or hung
- A complement to Nightgrid's backup board, which needs the app open

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `healthchecks`:

```yaml
services:
  healthchecks:
    image: healthchecks/healthchecks:latest
    restart: unless-stopped
    environment:
      SITE_ROOT: https://hc.example.com
      SITE_NAME: Homelab checks
      SECRET_KEY: ${SECRET_KEY}
      ALLOWED_HOSTS: hc.example.com
      DB: sqlite
      DB_NAME: /data/hc.sqlite
      REGISTRATION_OPEN: "False"
      EMAIL_HOST: 172.17.0.1
      DEFAULT_FROM_EMAIL: checks@example.com
    volumes:
      - ./data:/data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] `SECRET_KEY` in the **Vault**. Route `hc.example.com` to `healthchecks:8000`.
- [ ] Create the admin: `docker exec -it healthchecks-healthchecks-1 /opt/healthchecks/manage.py createsuperuser`.
- [ ] `./data` must be writable by the container's user (uid 999): `sudo chown 999 data`.

## Part 2 · Ping from jobs

- [ ] Create a check per job with its schedule (cron expression or period) and a grace time.
- [ ] In a systemd service, add:

```ini
ExecStartPost=/usr/bin/curl -fsS -m 10 --retry 3 https://hc.example.com/ping/<uuid>
```

- [ ] Report failures too: `ExecStopPost=/bin/sh -c 'test "$SERVICE_RESULT" = success || curl -fsS -m 10 https://hc.example.com/ping/<uuid>/fail'`.
- [ ] Integrations: ntfy, email, Discord and many more.

#### Check

- [ ] Disable a job's timer: the check goes late, then down, and you get notified.

## Limits and gotchas

- Run it somewhere other than the hosts it watches, or the monitor dies with them.
- The hosted healthchecks.io free tier is an option if you'd rather not run another service.

## Related

- [Monitor backups and test restores](kb:howto/backups)
- [ntfy push notifications](kb:apps/ntfy)
- [restic backups with systemd timers](kb:apps/restic)
