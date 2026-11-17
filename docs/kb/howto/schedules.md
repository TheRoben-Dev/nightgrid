# Schedule jobs with cron and timers

Read, add and change cron jobs and systemd timers on a host, with schedules explained in plain words, so recurring work runs on the server with Nightgrid closed.

![A systemd timer starts a oneshot service; its output goes to the journal.](fig:ng-timers)

## What you end up with

- Every timer and cron job on a host in one list with next and last run
- New jobs as systemd timers with logs in the journal
- Crontab edits that never overwrite someone else's change

## Part 1 · See what runs

- [ ] [System](view:system) → **schedules**: systemd timers (schedule, next and last run, whether the last run failed, with **Journal**) and cron jobs from your crontab, `/etc/crontab` and `/etc/cron.d`, in plain words.
- [ ] Commands that look like they hold a token are masked until **Reveal**.

## Part 2 · Add a timer (preferred)

- [ ] **+ Timer**: a name, the command and an `OnCalendar` schedule. `*-*-* 03:00:00` is daily at 03:00, `Mon..Fri 08:00` is weekdays, `Sun 04:00` is weekly. **Common** has more.
- [ ] **Check** shows when it will run. **Save** needs sudo.
- [ ] **Run now** tries it; **Journal** shows the output.

Why timers over cron: output goes to the journal, missed runs can catch up after downtime (`Persistent=true`), and a failure shows in the list.

## Part 3 · Edit a crontab

- [ ] **Crontab…** opens yours, or **ROOT** / **OTHER USER** (asks for sudo).
- [ ] Untick a job to switch it off without deleting it. **Edit**, **Delete…**, or type a new schedule (or pick from **Common**) and command.
- [ ] Write `%` as `\%`: cron treats a bare `%` as a line break (this bites `date +%F`). Commands in **+ Timer** are different: write `%` as-is, Nightgrid escapes it for systemd.
- [ ] If the crontab changed elsewhere since you opened it, Nightgrid asks you to reload. The previous version is saved in `~/.nightgrid/cron-backups`.

## Part 4 · Ask DRIFT

- [ ] "make a cron schedule for every Sunday at 4am" gets the expression; a pasted expression gets plain words, its next runs and the matching `OnCalendar=` line.

#### Check

- [ ] The new job's **next run** is when you expect, and after it runs the last result is OK.

## Limits and gotchas

- Timers use the host's time zone. Check with `timedatectl`.
- Cron runs with a tiny `PATH`. Use full paths (`/usr/bin/docker`) in cron commands.

## Related

- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [restic backups with systemd timers](kb:apps/restic)
- [Reading logs](kb:basics/reading-logs)
