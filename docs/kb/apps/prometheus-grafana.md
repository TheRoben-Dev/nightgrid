# Prometheus and Grafana

Collect detailed metrics from every host and container with Prometheus and node_exporter, and build long-term dashboards in Grafana. Complements Nightgrid's built-in monitoring when you want custom queries or exporter metrics.

![Prometheus pulls from exporters on each host; Grafana reads Prometheus.](fig:prometheus-pull)

## What you end up with

- node_exporter on each host, cAdvisor for containers
- Prometheus scraping them every 30 s with a retention you choose
- Grafana dashboards, including the well-known "Node Exporter Full"

## Part 1 · Exporters on every host

- [ ] A small stack per host (or push it with a playbook in [Automation](view:automation)):

```yaml
services:
  node-exporter:
    image: prom/node-exporter:latest
    restart: unless-stopped
    command: ["--path.rootfs=/host"]
    network_mode: host
    pid: host
    volumes:
      - /:/host:ro,rslave
```

- [ ] It listens on port 9100. Allow it only from the Prometheus host in [System](view:system) → **FIREWALL**.

## Part 2 · Prometheus and Grafana

- [ ] On the monitoring host, a stack `monitoring`:

```yaml
services:
  prometheus:
    image: prom/prometheus:latest
    restart: unless-stopped
    command:
      - --config.file=/etc/prometheus/prometheus.yml
      - --storage.tsdb.retention.time=90d
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - ./prom-data:/prometheus
    ports: ["127.0.0.1:9090:9090"]
  grafana:
    image: grafana/grafana:latest
    restart: unless-stopped
    volumes:
      - ./grafana:/var/lib/grafana
    ports: ["3000:3000"]
```

- [ ] `prometheus.yml`:

```yaml
global:
  scrape_interval: 30s
scrape_configs:
  - job_name: node
    static_configs:
      - targets: ["192.168.1.10:9100", "192.168.1.11:9100"]
```

- [ ] `sudo chown -R 65534:65534 prom-data && sudo chown -R 472:472 grafana` before the first start.
- [ ] Grafana on port 3000: log in (admin/admin, then change it), add Prometheus at `http://prometheus:9090`, import dashboard 1860 (Node Exporter Full).

## Part 3 · With Nightgrid

- [ ] Nightgrid already keeps host history for a year and alerts on it; use Grafana for exporter metrics (SNMP, UPS, apps) and custom panels.
- [ ] [Services](view:services) → **Add integration** → **Prometheus** shows scrape targets that are down and alerts firing; **Grafana** (a Viewer service account token) shows alert rules firing or pending (see [Add app cards](kb:howto/integrations)).
- [ ] A command widget can show a Prometheus query result: `curl -s 'localhost:9090/api/v1/query?query=up' | grep -o '"value":\[[^]]*' | grep -c '"1"'` counts targets that are up.

#### Check

- [ ] Prometheus → Status → Targets shows every host UP.

## Limits and gotchas

- Prometheus disk use grows with targets × metrics × retention; watch the forecast in [Fleet](view:fleet) → **Disks**.
- Don't expose Prometheus without auth; it reveals a lot about your network.

## Related

- [Grafana Loki for long-term logs](kb:apps/loki)
- [Uptime history and monthly reports](kb:howto/uptime-reports)
- [Build dashboards with command widgets](kb:howto/command-widgets)
