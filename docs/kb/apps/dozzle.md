# Dozzle live container logs

A tiny web UI for live Docker logs across hosts, for people who want to watch logs from a phone or share read-only access.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `dozzle`:

```yaml
services:
  dozzle:
    image: amir20/dozzle:latest
    restart: unless-stopped
    environment:
      DOZZLE_AUTH_PROVIDER: simple
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./data:/data
    ports:
      - "127.0.0.1:8080:8080"
```

- [ ] Create `./data/users.yml` with a bcrypt-hashed password as Dozzle's docs show (`docker run amir20/dozzle generate` helps).
- [ ] Other hosts: run Dozzle in agent mode and add them to the main instance.

#### Check

- [ ] Logs stream in the browser.

## Limits and gotchas

- In Nightgrid, [Docker](view:docker) already follows and searches logs, and [Fleet](view:fleet) → **Logs** searches all hosts at once with no install. Dozzle is for browsers and other people.

## Related

- [Search logs across all hosts](kb:howto/log-search)
- [Grafana Loki for long-term logs](kb:apps/loki)
