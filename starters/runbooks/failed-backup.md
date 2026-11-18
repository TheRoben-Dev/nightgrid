---
id: failed-backup
title: "Runbook: backup failed or stale"
summary: Read why a backup job failed or stopped running, and prove the next one restores.
alertKinds: [backupFailed, backupStale]
tags: [runbook, starter, backup]
library: [howto/backups, apps/restic, howto/restore-drills]
---
A backup job failed, or hasn't finished for longer than it should. Until it's fixed, every change since the last good run has no copy.

- [ ] See which job, its last good run and its last error in [Fleet → BACKUPS](view:fleet).
- [ ] Read the job's log (replace `UNIT` with the job's unit):

```sh
systemctl status UNIT --no-pager -l
journalctl -u UNIT --no-pager -n 80
```

- [ ] "repository is already locked": a run was interrupted. Make sure no other run is going, then remove the stale lock:

```sh
restic unlock
```

- [ ] Target unreachable or full: check the backup server or disk is up and has space (see the disk full runbook).
- [ ] Wrong password or key: the repository password lives in the job's environment or the `.env` vault. Never re-init a repository to fix this; you'd lose the history.
- [ ] Stale but no error: the timer isn't firing. Check `systemctl list-timers` and that the host was on at the scheduled time (`Persistent=true` catches up).
- [ ] Run it once by hand, then run a test restore from Fleet → BACKUPS to prove the data comes back.

More in [Backups](kb:howto/backups) and [Restore drills](kb:howto/restore-drills).
