# Tune alerts so only real problems notify

Decide what alerts, fold follow-on alerts under their cause, acknowledge or mute alerts you already know about, mute noisy log patterns, set quiet hours and send alerts to your phone through ntfy or Discord.

![From check to phone: folding, quiet hours and the webhook.](fig:ng-alert-path)

## What you end up with

- One notification per real outage, not ten
- A NAS that is always nearly full stays quiet until it gets worse
- Log alerts that fire on genuine errors only
- Alerts on your phone, quiet at night

## Part 1 · Pick the alert kinds

- [ ] **Settings → Alerts**: host offline, container stopped or unhealthy, disk almost full (with **DISK FULL AT**), disk full soon (forecast, **FULL WITHIN**), storage pool degraded, service down, certificate or watched item expiring, SMART warning, backup failed or stale.
- [ ] **Log anomalies** is opt-in. Only ERROR, FATAL and panic lines count; **ALERT AFTER** and **WITHIN** set how often a new error must repeat, and **GRACE AFTER RESTART** ignores start-up noise.

## Part 2 · Dependencies

- [ ] **Settings → Alerts → Dependencies**: add links such as "Media server depends on the NAS" or "everything depends on the router".
- [ ] When the cause is down, dependent alerts fold under it ("explains 3 more alerts") and only the cause notifies.
- [ ] Containers, disks, backups and services checked on a host already depend on that host.

## Part 3 · Acknowledge or mute what you already know

- [ ] **✓ Ack** on an alert: "I know". Pick **Until it clears**, a time (**1 hour** to **30 days**, or **Custom**) or **Permanently**. It stays in the list without a badge, sound, notification, inbox entry, ntfy message or webhook. It comes back on its own if it gets worse: a disk at 93 % acknowledged as WARN alerts again at 98 % (CRITICAL).
- [ ] **Mute…** on an alert: pick what (**This alert only**, every alert of that kind on that host, **Everything on this host**, or that kind on every host) and for how long (a time, **Custom** or **Forever**). Mutes ignore severity, so use them for maintenance or a disk you never want to hear about.
- [ ] A container that is **unhealthy by design** (its healthcheck never passes in your setup): open it in **Docker** and tick **Ignore health check** in its panel. Unhealthy then stops alerting and counting as down, while exits, crashes and restart loops still alert. The row shows **HEALTH IGNORED**; click that chip to watch the check again. It's kept per host and compose service (or container name), so recreating the container keeps it, and it travels in a profile export.
- [ ] **ACKNOWLEDGED & MUTED** in the Alerts panel lists every acknowledgement and mute with how long it has left. **Undo** brings the alert back into the count (without a new notification).
- [ ] Acknowledged and muted alerts still go on the host's timeline (System → TIMELINE), tagged **ACKNOWLEDGED** or **MUTED**, and aren't counted as problems there.
- [ ] When a timed acknowledgement or mute runs out and the alert is still there, you get one notification saying so.
- [ ] **Skip system mounts** (Settings → Alerts, under **Disk almost full**, on by default) leaves out mounts that are always full by design: read-only mounts, WSL and Docker Desktop (`/mnt/wsl/…`, such as `/mnt/wsl/docker-desktop/cli-tools`), snaps, and CD/DVD or other disk images.

## Part 4 · Mute log noise

- [ ] **Mute pattern** on a log alert stops that pattern on that container.
- [ ] **MUTED PATTERNS** in Settings → Alerts lists mutes and takes regex mutes for a container (or any).
- [ ] Turn on **404s are noise** for web apps that log every missing page.

## Part 5 · Notifications

- [ ] **Desktop notifications** on: one per new alert, a burst becomes one summary.
- [ ] **Quiet hours** with **FROM** and **UNTIL**: alerts still show in the app, notifications and webhooks wait.
- [ ] **WEBHOOK**: pick **ntfy**, **Discord** or **JSON POST**, paste the URL, **Save**. Tick **Send resolved messages** to hear when it's fixed.
- [ ] **Send test**.

#### Check

- [ ] The test arrives on your phone (ntfy app subscribed to your topic, or the Discord channel).

## Part 6 · Act on alerts

- [ ] **✦ Triage** on an alert gives DRIFT the usual causes and first checks.
- [ ] **+ Runbook** starts a note for that kind of alert (see [Write runbooks for alerts](kb:howto/runbooks)).

## When Nightgrid is closed

- No alerts are evaluated. Keep Nightgrid in the tray, or add an always-on watcher such as [Uptime Kuma](kb:apps/uptime-kuma) for the services that matter most.

## Limits and gotchas

- The host-offline alert needs host metrics on and waits 2 minutes.
- Acknowledgements and mutes belong to this computer: they aren't part of a profile export. An alert acknowledged "until it clears" that doesn't come back within 20 minutes of Nightgrid starting loses its acknowledgement, since it cleared while the app was closed.
- A public ntfy topic is readable by anyone who guesses its name. Use a long random topic or your own server (see [ntfy push notifications](kb:apps/ntfy)).

## Related

- [ntfy push notifications](kb:apps/ntfy)
- [Write runbooks for alerts](kb:howto/runbooks)
- [Search logs across all hosts](kb:howto/log-search)
- [Watch services and expiry dates](kb:howto/service-checks)
