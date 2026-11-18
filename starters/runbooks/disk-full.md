---
id: disk-full
title: "Runbook: disk full or filling up"
summary: Find what grew, free space safely, and stop it happening again.
alertKinds: [diskFull, diskFilling]
tags: [runbook, starter, disk]
library: [linux/disk-full, linux/docker-cleanup, linux/journald]
---
A mount is almost full, or will be soon at its current growth. When a root disk fills up, databases corrupt, logs stop and containers fail to start, so act before 100%.

- [ ] See which mount it is and how fast it grows in [Fleet → DISKS](view:fleet).
- [ ] Find the biggest folders on that mount (change `/` to the mount from the alert):

```sh
sudo du -xh --max-depth=2 / 2>/dev/null | sort -h | tail -20
```

- [ ] If it's Docker, see what it holds, then clear stopped containers, unused images and build cache:

```sh
docker system df
docker system prune -a --filter "until=168h"
```

- [ ] If it's the journal, cap it:

```sh
sudo journalctl --vacuum-size=500M
```

- [ ] Check for files that are deleted but still held open (space only comes back when the process restarts):

```sh
sudo lsof -nP +L1 2>/dev/null | head -20
```

- [ ] Make it last: cap Docker's container logs and the journal, and move big data (media, downloads, backups) to its own disk.

More in [Disk full](kb:linux/disk-full) and [Docker cleanup](kb:linux/docker-cleanup).
