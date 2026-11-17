# Graylog central log server

Collect syslog and app logs from every host, network device and container into one searchable place, with streams that sort messages, extractors that pull fields out, and alerts on patterns. Use it when you need to keep logs for months or take in logs from devices Nightgrid can't SSH into (routers, switches, NAS appliances).

## What you end up with

- Graylog, MongoDB and OpenSearch in one stack
- A syslog input that routers, switches and Linux hosts send to
- Streams per source and an alert on a log pattern

## Before you start

- 4 GB RAM is the practical minimum; OpenSearch alone wants 1–2 GB of heap.
- On the Docker host: `sudo sysctl -w vm.max_map_count=262144` and save it in `/etc/sysctl.d/99-opensearch.conf`.
- Check Graylog's compatibility page for which OpenSearch and MongoDB versions your Graylog release supports, and pin those tags.

## Part 1 · The stack

- [ ] Generate the two secrets: a password pepper and the admin password hash.

```bash
openssl rand -hex 48
echo -n 'your-admin-password' | sha256sum | cut -d' ' -f1
```

- [ ] [Docker](view:docker) → **+ New stack** named `graylog`, with `GL_SECRET` and `GL_ROOT_SHA2` in `.env`:

```yaml
services:
  mongodb:
    image: mongo:7.0
    restart: unless-stopped
    volumes:
      - ./mongo:/data/db

  opensearch:
    image: opensearchproject/opensearch:2.15.0
    restart: unless-stopped
    environment:
      OPENSEARCH_JAVA_OPTS: "-Xms1g -Xmx1g"
      discovery.type: single-node
      DISABLE_SECURITY_PLUGIN: "true"
      DISABLE_INSTALL_DEMO_CONFIG: "true"
      action.auto_create_index: "false"
    volumes:
      - ./opensearch:/usr/share/opensearch/data

  graylog:
    image: graylog/graylog:6.1
    restart: unless-stopped
    depends_on: [mongodb, opensearch]
    environment:
      GRAYLOG_PASSWORD_SECRET: ${GL_SECRET}
      GRAYLOG_ROOT_PASSWORD_SHA2: ${GL_ROOT_SHA2}
      GRAYLOG_HTTP_EXTERNAL_URI: http://192.0.2.10:9000/
      GRAYLOG_ELASTICSEARCH_HOSTS: http://opensearch:9200
      GRAYLOG_MONGODB_URI: mongodb://mongodb:27017/graylog
    volumes:
      - ./graylog:/usr/share/graylog/data
    ports:
      - "9000:9000"
      - "1514:1514/udp"
      - "1514:1514/tcp"
```

- [ ] The `./opensearch` and `./graylog` folders must be writable by the containers' users (uid 1000 and 1100). If a container restarts with "permission denied", `sudo chown -R 1000:1000 opensearch` and `sudo chown -R 1100:1100 graylog`.
- [ ] Log in on port 9000 as `admin` with the password you hashed.

## Part 2 · Inputs

- [ ] **System → Inputs** → **Syslog UDP** → Launch new input, port 1514, global. Add a **Syslog TCP** on 1514 as well for hosts that should not lose messages.
- [ ] Linux hosts with rsyslog: create `/etc/rsyslog.d/90-graylog.conf` and restart rsyslog.

```text
*.* @192.0.2.10:1514;RSYSLOG_SyslogProtocol23Format
```

- [ ] `@` is UDP, `@@` is TCP. Routers, switches and NAS appliances: point their remote syslog setting at the same address and port.

## Part 3 · Streams and alerts

- [ ] **Streams → Create stream** per source (for example `source:router*`) and tick "remove matches from default stream" to keep searches tidy.
- [ ] **Alerts → Event definitions**: a filter like `message:"authentication failure"` counted over 5 minutes, with a notification.
- [ ] **System → Indices**: set retention (for example 30 indices of 1 day each) so the disk can't fill.

#### Check

- [ ] `logger -n 192.0.2.10 -P 1514 "graylog test"` from a host appears in search within seconds.

## In Nightgrid

- Add `http://<host>:9000` to [Services](view:services) and back up the `mongo` folder (settings) with the stack folder. Losing OpenSearch data loses old logs, not configuration.
- For logs on hosts Nightgrid already reaches, [Fleet](view:fleet) → **Logs** searches journals live with nothing installed; Graylog is for long retention and devices without SSH.

## Limits and gotchas

- Docker's UDP port publishing rewrites the source address in some setups, so every message seems to come from the Docker gateway. Use TCP or `network_mode: host` for Graylog if source IPs matter.
- Upgrading Graylog major versions can need an OpenSearch upgrade first; read the upgrade notes.
- The admin password lives only as a hash in `.env`; changing it means a new hash and a redeploy.

## Related

- [Grafana Loki for long-term logs](kb:apps/loki)
- [Search logs across all hosts](kb:howto/log-search)
- [Journald: keep logs, but not forever](kb:linux/journald)
