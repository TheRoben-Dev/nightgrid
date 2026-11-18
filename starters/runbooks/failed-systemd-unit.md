---
id: failed-systemd-unit
title: "Runbook: failed systemd unit"
summary: Find out why a service or timer on the host failed and restart it cleanly.
tags: [runbook, starter, systemd]
library: [linux/systemd-units, errors/systemd, linux/journald]
---
A systemd service or timer on the host ended in "failed". Nightgrid shows these in System → TIMELINE and on the host's page. They're not an alert kind of their own yet, so this runbook isn't offered on alerts: open it from Knowledge, or tie it to a host or service in Edit.

- [ ] List what failed:

```sh
systemctl --failed --no-pager
```

- [ ] See why (replace `UNIT`):

```sh
systemctl status UNIT --no-pager -l
journalctl -u UNIT -b --no-pager -n 80
```

- [ ] "code=exited, status=203/EXEC": the program in `ExecStart` doesn't exist or isn't executable.
- [ ] "start request repeated too quickly": it crashed several times in a row. Fix the cause, then clear the counter.
- [ ] Changed its unit file? Reload systemd first:

```sh
sudo systemctl daemon-reload
```

- [ ] Start it again and clear the failed state:

```sh
sudo systemctl reset-failed UNIT
sudo systemctl restart UNIT
```

More in [systemd units](kb:linux/systemd-units) and [systemd errors](kb:errors/systemd).
