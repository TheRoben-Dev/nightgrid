# InfluxDB and Telegraf metrics

A time-series database (InfluxDB) fed by a collector agent (Telegraf) that has inputs for almost anything: SNMP switches, UPSes, MQTT sensors, Docker, Proxmox and more. Graph it in Grafana or InfluxDB's own UI.

![Telegraf into InfluxDB, read by Grafana.](fig:app-telegraf)

## What you end up with

- InfluxDB 2 on port 8086 with a bucket for homelab metrics
- Telegraf agents sending host, Docker and SNMP data
- A Grafana data source for dashboards

## Part 1 · InfluxDB

- [ ] [Docker](view:docker) → **+ New stack** named `influxdb`, secrets in `.env`:

```yaml
services:
  influxdb:
    image: influxdb:2
    restart: unless-stopped
    ports:
      - "8086:8086"
    environment:
      DOCKER_INFLUXDB_INIT_MODE: setup
      DOCKER_INFLUXDB_INIT_USERNAME: admin
      DOCKER_INFLUXDB_INIT_PASSWORD: ${ADMIN_PASSWORD}
      DOCKER_INFLUXDB_INIT_ORG: home
      DOCKER_INFLUXDB_INIT_BUCKET: homelab
      DOCKER_INFLUXDB_INIT_RETENTION: 90d
      DOCKER_INFLUXDB_INIT_ADMIN_TOKEN: ${ADMIN_TOKEN}
    volumes:
      - ./data:/var/lib/influxdb2
      - ./config:/etc/influxdb2
```

- [ ] In the UI (port 8086), Load Data → API Tokens: create a write token for the `homelab` bucket for Telegraf, and a read token for Grafana.

## Part 2 · Telegraf

- [ ] A `telegraf.conf` next to a Telegraf container (or the `telegraf` package on a host):

```toml
[agent]
  interval = "30s"

[[outputs.influxdb_v2]]
  urls = ["http://influx-host:8086"]
  token = "$INFLUX_TOKEN"
  organization = "home"
  bucket = "homelab"

[[inputs.cpu]]
[[inputs.mem]]
[[inputs.disk]]
  ignore_fs = ["tmpfs", "devtmpfs", "overlay"]

[[inputs.snmp]]
  agents = ["udp://192.168.1.2:161"]
  community = "public"
  [[inputs.snmp.field]]
    name = "uptime"
    oid = "RFC1213-MIB::sysUpTime.0"
```

- [ ] Pass `INFLUX_TOKEN` as an environment variable, not in the file.

#### Check

- [ ] InfluxDB's Data Explorer shows `cpu` and `mem` measurements arriving every 30 seconds.

## In Nightgrid

- Nightgrid already records CPU, memory, disk, network and temperatures for every host with long history; use Telegraf for what Nightgrid can't SSH into, such as switches over SNMP or sensors over MQTT.
- Add InfluxDB to [Services](view:services), and watch `./data` grow in [Fleet](view:fleet) → **Disks**.

## Limits and gotchas

- InfluxDB 3 Core is a different product with a different API; examples here are for version 2.
- SNMP community `public` is read-only but readable by anyone on the network; use SNMPv3 where the device supports it.

## Related

- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [Node-RED flows](kb:apps/node-red)
- [Build dashboards with command widgets](kb:howto/command-widgets)
