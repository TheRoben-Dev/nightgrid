# Monitor backups and test restores

See every restic, borg, rsync and rclone job on one board, get an alert when one fails or goes stale, and prove a backup can actually be read back.

![The 3-2-1 rule the backup board helps you keep.](fig:backups-321)

## What you end up with

- A board with last run, size, result and a stale flag per job
- **Backup failed or stale** alerts
- A dated record of the last successful test restore

## Before you start

- Backups that run on the hosts themselves (systemd timers or cron). Nightgrid never runs a backup; it watches them. Set one up with [restic backups with systemd timers](kb:apps/restic) or [Borg backups with borgmatic](kb:apps/borgmatic). Background: [Backups](kb:basics/backups).

## Part 1 · Add the jobs

- [ ] [Fleet](view:fleet) → **Backups**, pick a host, **Find jobs**: suggestions come from its timers and cron tables.
- [ ] Or **+ Add job**: a systemd timer (Nightgrid reads last run, exit status and the totals the tool printed) or a path such as a repository or rsync target (newest file and size).

## Part 2 · Alerts

- [ ] **Settings → Alerts → Backup failed or stale**. Jobs are checked every 30 minutes on connected hosts.
- [ ] Set each job's stale window to match its schedule (a nightly job is stale after about a day and a half).

## Part 3 · Test restore

- [ ] **Test restore** on a restic, borg or folder-copy job restores one random file under 20 MB from the latest snapshot to a new folder under `/tmp` on the host, checks it, shows its SHA-256 and deletes the folder.
- [ ] It runs as the backup unit's own user and environment, so the repository password never leaves the host.
- [ ] Do it monthly. The job shows when it was last tested.

#### Check

- [ ] Every job is green with a recent test-restore date.

## Part 4 · The 3-2-1 rule

- [ ] Three copies, two kinds of storage, one offsite. A second repository with [Offsite copies with rclone](kb:apps/rclone) covers the last one.
- [ ] Back up your Nightgrid setup too: [Back up and move your Nightgrid setup](kb:howto/profiles).

## Limits and gotchas

- A job pointing at a bare folder path can't find a restic or borg password; test the unit job instead.
- Container databases must be dumped first; copying a live database folder gives a backup that may not open (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)).
- Proxmox guest backups (vzdump) are on Proxmox → **BACKUPS** instead.

## Related

- [Backups](kb:basics/backups)
- [restic backups with systemd timers](kb:apps/restic)
- [Borg backups with borgmatic](kb:apps/borgmatic)
- [Offsite copies with rclone](kb:apps/rclone)
