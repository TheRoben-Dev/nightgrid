# Zabbix monitoring

The classic all-in-one monitoring server: agents collect hundreds of metrics per host, templates bring ready-made triggers for Linux, Docker, databases, SNMP switches and UPSes, and everything has history, graphs and escalations. Heavier than Nightgrid's own monitoring, and a good fit when you want SNMP devices and years of data in one place.

![Zabbix agents reporting to the server.](fig:app-hub-agent)

## What you end up with

- Zabbix 7.0 LTS server, web UI and PostgreSQL in one stack
- Zabbix agent 2 on each Linux host, linked to the Linux template
- SNMP polling for a switch, printer or UPS

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `zabbix`. Put `DB_PASSWORD` in `.env`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: zabbix
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: zabbix
    volumes:
      - ./db:/var/lib/postgresql/data

  server:
    image: zabbix/zabbix-server-pgsql:alpine-7.0-latest
    restart: unless-stopped
    depends_on: [db]
    environment:
      DB_SERVER_HOST: db
      POSTGRES_USER: zabbix
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: zabbix
    ports:
      - "10051:10051"

  web:
    image: zabbix/zabbix-web-nginx-pgsql:alpine-7.0-latest
    restart: unless-stopped
    depends_on: [server]
    environment:
      ZBX_SERVER_HOST: server
      DB_SERVER_HOST: db
      POSTGRES_USER: zabbix
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: zabbix
      PHP_TZ: Etc/UTC
    ports:
      - "8085:8080"
```

- [ ] Open port 8085, log in as `Admin` / `zabbix` and change the password under **User settings → Profile** at once.

## Part 2 · Agents

- [ ] On each host, add the Zabbix 7.0 repository for its distro (the download page generates the exact commands), then install `zabbix-agent2`.
- [ ] Edit `/etc/zabbix/zabbix_agent2.conf`:

```ini
Server=192.0.2.10
ServerActive=192.0.2.10
Hostname=web-01
```

- [ ] `Server` is who may poll the agent (TCP 10050), `ServerActive` is where it pushes to (TCP 10051). Use the Docker host's address, then `sudo systemctl enable --now zabbix-agent2`.
- [ ] In the web UI: **Data collection → Hosts → Create host**, same host name, interface **Agent**, template **Linux by Zabbix agent**. Add **Docker by Zabbix agent 2** on Docker hosts (the agent's user must be in the `docker` group).

## Part 3 · SNMP devices

- [ ] Enable SNMP v2c (read-only, a non-default community) or v3 on the switch, printer or UPS card.
- [ ] Create host → interface **SNMP**, version and community, and a template such as **Network Generic Device by SNMP** or the vendor's.

#### Check

- [ ] **Monitoring → Hosts** shows the agent icon green for each host, and **Latest data** has fresh CPU and disk values.

## In Nightgrid

- Add the web UI to [Services](view:services). Nightgrid keeps watching hosts on its own; Zabbix adds SNMP and long retention. Use one place for alerting to avoid double notifications.
- The agent config lives in `/etc`, so [System](view:system) → **config** → **Compare with** another host shows when two agents are set up differently.

## Limits and gotchas

- History tables grow fast. **Administration → Housekeeping** sets retention; keep raw history short (7–14 days) and trends long.
- The agent's `Hostname` must match the host name in the UI exactly, or active checks are refused with "host not found".
- Templates bring many triggers; disable the noisy ones instead of ignoring alerts.

## Related

- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [Netdata per-second monitoring](kb:apps/netdata)
- [Track config drift and host history](kb:howto/config-drift)
