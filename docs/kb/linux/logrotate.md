# Keep logs from filling the disk

Logs grow until something rotates them. Set limits for the journal, for app log files, and for Docker's container logs, the usual cause of a full root disk on a homelab server.

![How a log moves through rotation.](fig:lx-logrotate)

## What you end up with

- The journal capped at a size you choose
- App logs rotated and compressed by logrotate
- Docker container logs limited per container

## Part 1 · Find what's big

```sh
sudo du -xh --max-depth=1 /var/log | sort -h | tail
journalctl --disk-usage
sudo du -sh /var/lib/docker/containers/*/*-json.log 2>/dev/null | sort -h | tail
```

## Part 2 · Cap the journal

- [ ] Create `/etc/systemd/journald.conf.d/size.conf`:

```ini
[Journal]
SystemMaxUse=500M
MaxRetentionSec=1month
```

- [ ] `sudo systemctl restart systemd-journald`. Shrink it now with `sudo journalctl --vacuum-size=500M`.

## Part 3 · Rotate app logs

- [ ] A file per app in `/etc/logrotate.d/` (here for logs under `/srv/myapp/logs`):

```
/srv/myapp/logs/*.log {
    weekly
    rotate 8
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
}
```

- [ ] `copytruncate` suits apps that keep the file open and can't be told to reopen it. Test without changing anything: `sudo logrotate -d /etc/logrotate.d/myapp`.

## Part 4 · Docker container logs

- [ ] Default limits for new containers in `/etc/docker/daemon.json`:

```json
{
  "log-driver": "local",
  "log-opts": { "max-size": "20m", "max-file": "5" }
}
```

- [ ] `sudo systemctl restart docker`. Existing containers keep their old settings until they're recreated (`docker compose up -d --force-recreate`).
- [ ] Or per service in compose:

```yaml
    logging:
      driver: local
      options:
        max-size: "20m"
        max-file: "5"
```

#### Check

- [ ] `journalctl --disk-usage` is under the cap, and `docker inspect -f '{{.HostConfig.LogConfig}}' <container>` shows the limits.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** shows the root disk's growth per day; it flattens once logs are capped.
- **Settings → Alerts** → **Disk full soon (forecast)** warns before logs fill a disk again.
- Container logs are still readable in [Docker](view:docker) and searchable in [Fleet](view:fleet) → **Logs** within the limits you set.

## Limits and gotchas

- The `local` log driver compresses and is faster than `json-file`, and `docker logs` still works with it.
- A crash-looping container can write gigabytes an hour; limits keep that from taking the host down.
- Rotated logs are gone for log search too. Keep the journal long enough to investigate last week's problem.

## Related

- [Reading logs](kb:basics/reading-logs)
- [Search logs across all hosts](kb:howto/log-search)
- [Journald: keep logs, but not forever](kb:linux/journald)
