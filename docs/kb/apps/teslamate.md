# TeslaMate car data logger

Log everything your Tesla reports, on your own server: drives with maps, charging sessions with cost, battery degradation over time, efficiency, vampire drain and software updates, shown in ready-made Grafana dashboards. It also publishes live data over MQTT for Home Assistant.

## What you end up with

- TeslaMate, PostgreSQL, Grafana and Mosquitto in one stack
- Your car logged from the moment you sign in
- Dashboards for drives, charges, battery health and more

## Before you start

- Tesla API tokens: TeslaMate signs in with an access and refresh token that you generate with a third-party token app (the TeslaMate docs list options). Never give your Tesla password to a tool you don't trust.
- The web UI has no login of its own; keep it on the LAN or behind authentication.

## Part 1 · The stack

- [ ] Create a key with `openssl rand -hex 32`. [Docker](view:docker) → **+ New stack** named `teslamate`, with the key and `DB_PASSWORD` in `.env`:

```yaml
services:
  teslamate:
    image: teslamate/teslamate:latest
    restart: unless-stopped
    environment:
      ENCRYPTION_KEY: ${ENCRYPTION_KEY}
      DATABASE_USER: teslamate
      DATABASE_PASS: ${DB_PASSWORD}
      DATABASE_NAME: teslamate
      DATABASE_HOST: database
      MQTT_HOST: mosquitto
    ports:
      - "4000:4000"
    cap_drop:
      - all

  database:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_USER: teslamate
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: teslamate
    volumes:
      - ./db:/var/lib/postgresql/data

  grafana:
    image: teslamate/grafana:latest
    restart: unless-stopped
    environment:
      DATABASE_USER: teslamate
      DATABASE_PASS: ${DB_PASSWORD}
      DATABASE_NAME: teslamate
      DATABASE_HOST: database
    ports:
      - "3000:3000"
    volumes:
      - ./grafana:/var/lib/grafana

  mosquitto:
    image: eclipse-mosquitto:2
    restart: unless-stopped
    command: mosquitto -c /mosquitto-no-auth.conf
    volumes:
      - ./mosquitto-conf:/mosquitto/config
      - ./mosquitto-data:/mosquitto/data
```

- [ ] Check the TeslaMate docs for the PostgreSQL major version it currently supports, and keep that pinned; major upgrades need a dump and restore.
- [ ] `ENCRYPTION_KEY` encrypts the stored API tokens. Keep it with your other secrets.

## Part 2 · Sign in and settings

- [ ] Open port 4000, paste the access and refresh tokens. The car appears within a minute.
- [ ] **Settings**: units, the Grafana URL (`http://<host>:3000`) so dashboard links work, and **sleep mode** settings that let the car fall asleep (TeslaMate stops polling when it can).
- [ ] **Geo-fences**: name Home and Work, and set charging costs per location for cost reports.

## Part 3 · Dashboards

- [ ] Grafana on port 3000 (first login `admin` / `admin`, then change it). The TeslaMate folder has dashboards for **Drives**, **Charges**, **Battery Health**, **Efficiency**, **Vampire Drain**, **Updates** and more.
- [ ] Home Assistant: point its MQTT integration at the Mosquitto container (publish port 1883 if Home Assistant runs elsewhere) and use the `teslamate/cars/1/...` topics.

#### Check

- [ ] After a short drive, the **Drives** dashboard shows it on a map with distance and consumption.

## In Nightgrid

- Add both `http://<host>:4000` and `http://<host>:3000` to [Services](view:services).
- The database is the history; back up a PostgreSQL dump daily (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)). TeslaMate can't re-import drives it missed.

## Limits and gotchas

- If TeslaMate keeps the car awake, check sleep settings; a car that never sleeps loses a few % per day.
- Tokens expire if unused for a long time; TeslaMate refreshes them while it runs.
- The Mosquitto container above has no authentication; don't publish its port beyond your LAN.

## Related

- [evcc solar EV charging](kb:apps/evcc)
- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [Zigbee2MQTT and Mosquitto](kb:apps/zigbee2mqtt)
