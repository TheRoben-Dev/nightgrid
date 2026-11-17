# VictoriaMetrics long-term metrics

A single-binary time-series database that speaks the Prometheus query language and stores months or years of metrics in a fraction of the disk and RAM Prometheus needs. Use it as Prometheus's long-term store, or let it scrape targets itself and skip Prometheus entirely.

![The scrape model VictoriaMetrics shares with Prometheus.](fig:prometheus-pull)

## What you end up with

- VictoriaMetrics single-node with a 12-month retention
- Either Prometheus writing into it, or VictoriaMetrics scraping your exporters directly
- Grafana reading from it as an ordinary Prometheus data source

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `victoriametrics`:

```yaml
services:
  victoriametrics:
    image: victoriametrics/victoria-metrics:latest
    restart: unless-stopped
    command:
      - -storageDataPath=/storage
      - -retentionPeriod=12
      - -promscrape.config=/etc/vm/scrape.yml
    volumes:
      - ./storage:/storage
      - ./scrape.yml:/etc/vm/scrape.yml:ro
    ports:
      - "127.0.0.1:8428:8428"
```

- [ ] `-retentionPeriod` without a unit is months; `90d` or `2y` also work.
- [ ] Create `scrape.yml` in the stack folder with [Files](view:files) before the first deploy. It uses Prometheus's format:

```yaml
scrape_configs:
  - job_name: node
    scrape_interval: 30s
    static_configs:
      - targets: ["192.0.2.10:9100", "192.0.2.11:9100"]
```

## Part 2 · Or keep Prometheus

- [ ] Already run Prometheus? Drop `-promscrape.config` and its volume, and add to `prometheus.yml`:

```yaml
remote_write:
  - url: http://victoriametrics:8428/api/v1/write
```

- [ ] Then shorten Prometheus's own retention (`--storage.tsdb.retention.time=2d`) so it only buffers, and keep history in VictoriaMetrics. Both need to share a Docker network, or use the host address.

## Part 3 · Grafana

- [ ] Grafana → **Connections → Data sources → Add → Prometheus**, URL `http://victoriametrics:8428` (or the host address). Existing PromQL dashboards work unchanged.
- [ ] `http://127.0.0.1:8428/vmui` (through a tunnel or the proxy) is a built-in query UI, and `/targets` lists scrape targets and errors.

#### Check

- [ ] In vmui, `up` returns 1 for each target, and after a day `du -sh storage` is small compared with what Prometheus used for the same data.

## In Nightgrid

- [Tunnels](view:tunnels) → **OPEN WEB UI VIA** reaches vmui without exposing port 8428.
- Nightgrid keeps its own host history (up to a year at hourly resolution) without any of this; VictoriaMetrics is for exporter metrics beyond what Nightgrid collects, such as per-app or SNMP data.
- Watch the `storage` folder's growth in [Fleet](view:fleet) → **Disks**.

## Limits and gotchas

- Single-node has no built-in authentication. Keep 8428 bound to `127.0.0.1` or a private network, or put basic auth in front at the reverse proxy.
- Alerting needs the separate `vmalert` component, or keep alert rules in Prometheus.
- Retention is applied in whole partitions, so old data disappears a little later than the exact cut-off.

## Related

- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [InfluxDB and Telegraf metrics](kb:apps/influxdb-telegraf)
- [Grafana Loki for long-term logs](kb:apps/loki)
