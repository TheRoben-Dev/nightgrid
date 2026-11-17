# Keep watching with the laptop closed

One home server, one laptop that's often shut. Put [Nightgrid Node](kb:basics/nightgrid-node) on the server so outages, crashes, full disks and failed backups are recorded while you're away, and read the story the next morning in a minute.

![Recorded overnight, read in the morning.](fig:ng-node-away)

## What you end up with

- Gap-free metric history, uptime and disk forecasts on the server, kept for months
- Alerts that fire while the app is closed: disk full, disk filling, container down, failing disk, degraded pool, failed or stale backup
- A morning routine: one command that lists what broke, what changed and what is filling up

## Before you start

- A Linux server with systemd that stays on (mini PC, NAS, old desktop), and sudo on it
- About 10 minutes

## Part 1 · Install

- [ ] Follow [Install Nightgrid Node](kb:howto/node-install) Parts 1–3.
- [ ] Grant what this box needs: `sudo nightgrid-node allow journal`, plus `sudo nightgrid-node allow docker` if it runs containers, and `sudo nightgrid-node allow health` for SMART, pools and pending updates. Read [Choose what Nightgrid Node may do](kb:howto/node-grants) first: the Docker grant is root-equivalent.
- [ ] `nightgrid-node status`: the grants you made show `ok`.

## Part 2 · Set your alert limits

- [ ] See the defaults: `nightgrid-node query alerts.rules.get | jq`.
- [ ] Tighten them if you like, for example warn at 85% and forecast two weeks ahead: `nightgrid-node query alerts.rules.set '{"enabled":true,"diskFull":true,"diskPercent":85,"diskForecast":true,"forecastDays":14,"smartWarning":true,"poolDegraded":true,"backupStale":true}'`.
- [ ] Container-down alerts use the same grace periods as the app, so a quick restart doesn't page you.

## Part 3 · Tell it about your backups

- [ ] A restic, borg, rsync or rclone job run by a systemd timer: `nightgrid-node query jobs.set '{"backups":[{"id":1,"name":"nightly","tool":"restic","source":"unit","target":"restic-backup.timer","staleHours":26}]}'`.
- [ ] A job that only drops files somewhere (a USB copy, a NAS share): use `"source":"path"` and the folder as `target`; the newest file inside counts as the last run.
- [ ] `nightgrid-node query jobs.latest | jq`: `result`, `lastOk` and `stale` per job. A failed run raises `backupFailed`; no good run within `staleHours` raises `backupStale`.
- [ ] Jobs you scheduled from Nightgrid's System → **Jobs** (`nightgrid-job-*` timers) are found automatically: every run lands in the node's timeline.

## Part 4 · Your morning check

- [ ] SSH in (or open a [Terminal](view:terminal) tab) and run `nightgrid-node query summary.daily | jq -r '.level, .facts[]'`. It reads like "Uptime: down (crash or power loss) for 2 h from 7 h ago", "Containers: jellyfin restarted (×2)", "Disks: /srv 91% full, full in about 12 days".
- [ ] Anything odd? `nightgrid-node query events.sync '{"after":0}' | jq '.events[-20:]'` shows the last 20 timeline entries in detail.
- [ ] Ask DRIFT about it: paste the facts into the DRIFT panel with "what should I look at first?". The summary is written as short facts on purpose, so a small local model handles them well.
- [ ] Save the summary command as a snippet or a [command widget](kb:howto/command-widgets) so it's one click away.

## Part 5 · Check it really works

- [ ] Stop a test container with `docker stop <name>` and wait two minutes: `nightgrid-node query alerts.active` shows it down. Start it again and the alert clears with a `recovered` event.
- [ ] Reboot the server once: after it comes back, the timeline has a `Restarted` outage, not `Down`. A power cut or crash shows as `Down`.
- [ ] `nightgrid-node status` a day later: the minute tier covers the whole day.

## Tips

- History lives in `/var/lib/nightgrid-node`; with defaults it stays small (a few MB).
- Want more detail? `[metrics] interval_secs = 5` in `/etc/nightgrid-node/config.toml`, then `sudo systemctl restart nightgrid-node`.
- When Nightgrid's integration with the node arrives, this history and timeline are what fill the app's charts for the time it was closed. Nothing you set up here is wasted.
