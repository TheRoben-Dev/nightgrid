# Grafana Loki for long-term logs

Ship journal and container logs from every host to Loki with Grafana Alloy, keep them for months, and search them in Grafana. Nightgrid searches live logs; Loki is for history.

![Logs from every host into Loki, searched from Grafana.](fig:app-loki)

## What you end up with

- Loki storing logs with a retention you choose
- Alloy on each host sending the journal and Docker logs
- Log search and panels in Grafana next to your metrics

## Part 1 · Loki

- [ ] Add to the `monitoring` stack from [Prometheus and Grafana](kb:apps/prometheus-grafana):

```yaml
  loki:
    image: grafana/loki:3.5
    restart: unless-stopped
    command: -config.file=/etc/loki/local-config.yaml
    ports: ["3100:3100"]
    volumes:
      - ./loki-data:/loki
```

- [ ] For retention, copy Loki's default config out of the image, set `limits_config.retention_period: 744h` and enable the compactor's retention, then mount your version over `local-config.yaml`.
- [ ] Allow port 3100 only from your hosts.

## Part 2 · Alloy on each host

- [ ] A stack `alloy` with `grafana/alloy:latest`, mounting `/var/log/journal:/var/log/journal:ro`, `/run/log/journal:/run/log/journal:ro`, `/etc/machine-id:/etc/machine-id:ro` and the Docker socket read-only, plus a `config.alloy`:

```
loki.write "default" {
  endpoint { url = "http://monitor.example.lan:3100/loki/api/v1/push" }
}

loki.source.journal "journal" {
  forward_to = [loki.write.default.receiver]
  labels     = { job = "journal", host = constants.hostname }
}

discovery.docker "containers" {
  host = "unix:///var/run/docker.sock"
}

loki.source.docker "containers" {
  host       = "unix:///var/run/docker.sock"
  targets    = discovery.docker.containers.targets
  forward_to = [loki.write.default.receiver]
  labels     = { job = "docker", host = constants.hostname }
}
```

- [ ] Run Alloy with `command: run /etc/alloy/config.alloy` and that file mounted.

## Part 3 · Search

- [ ] Grafana → add Loki as a data source → Explore: `{host="media", job="docker"} |= "error"`.

#### Check

- [ ] Logs from every host appear in Explore within a minute.

## Limits and gotchas

- Promtail is deprecated in favour of Alloy; old guides using Promtail still work but won't get updates.
- Keep labels few (host, job, container). High-cardinality labels make Loki slow.
- Nightgrid's [Fleet → Logs](kb:howto/log-search) needs no setup and is often enough; add Loki when you need weeks or months.

## Related

- [Search logs across all hosts](kb:howto/log-search)
- [Prometheus and Grafana](kb:apps/prometheus-grafana)
- [Reading logs](kb:basics/reading-logs)
