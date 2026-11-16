# Gatus status checks as code

Health checks defined in one YAML file: HTTP, TCP, DNS, ICMP and certificate conditions, with a status page and alerts. Good if you like Uptime Kuma's idea but want config in git.

## What you end up with

- Endpoints and conditions in `config.yaml`, versioned
- A status page and alerting through ntfy, email or chat

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `gatus`:

```yaml
services:
  gatus:
    image: twinproduction/gatus:latest
    restart: unless-stopped
    volumes:
      - ./config:/config
      - ./data:/data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] `./config/config.yaml`:

```yaml
storage:
  type: sqlite
  path: /data/data.db
alerting:
  ntfy:
    url: https://ntfy.example.com
    topic: homelab
    token: ${NTFY_TOKEN}
endpoints:
  - name: Nextcloud
    group: apps
    url: https://cloud.example.com/status.php
    interval: 1m
    conditions:
      - "[STATUS] == 200"
      - "[BODY].installed == true"
      - "[CERTIFICATE_EXPIRATION] > 168h"
    alerts:
      - type: ntfy
  - name: DNS
    group: infra
    url: 192.168.1.2
    dns:
      query-name: example.com
      query-type: A
    conditions:
      - "[DNS_RCODE] == NOERROR"
```

- [ ] Route the status page to `gatus:8080`.

## Part 2 · With Nightgrid

- [ ] Keep `config.yaml` in the git repository you deploy stacks from ([Deploy stacks from git](kb:howto/stacks-from-git)); a review and deploy updates checks too.
- [ ] Nightgrid's own checks add history, folding and expiry watch; Gatus adds 24/7 coverage.

#### Check

- [ ] The status page shows every endpoint green.

## Limits and gotchas

- `${VAR}` in the config is filled from the container environment; put tokens in the stack's **Vault**.

## Related

- [Uptime Kuma](kb:apps/uptime-kuma)
- [Healthchecks dead man's switch](kb:apps/healthchecks)
