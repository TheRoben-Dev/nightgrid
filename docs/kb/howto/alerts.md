# Tune alerts so only real problems notify

Decide what alerts, fold follow-on alerts under their cause, mute noisy log patterns, set quiet hours and send alerts to your phone through ntfy or Discord.

## What you end up with

- One notification per real outage, not ten
- Log alerts that fire on genuine errors only
- Alerts on your phone, quiet at night

## Part 1 · Pick the alert kinds

- [ ] **Settings → Alerts**: host offline, container stopped or unhealthy, disk almost full (with **DISK FULL AT**), disk full soon (forecast, **FULL WITHIN**), storage pool degraded, service down, certificate or watched item expiring, SMART warning, backup failed or stale.
- [ ] **Log anomalies** is opt-in. Only ERROR, FATAL and panic lines count; **ALERT AFTER** and **WITHIN** set how often a new error must repeat, and **GRACE AFTER RESTART** ignores start-up noise.

## Part 2 · Dependencies

- [ ] **Settings → Dependencies**: add links such as "Media server depends on the NAS" or "everything depends on the router".
- [ ] When the cause is down, dependent alerts fold under it ("explains 3 more alerts") and only the cause notifies.
- [ ] Containers, disks, backups and services checked on a host already depend on that host.

## Part 3 · Mute noise

- [ ] **Mute pattern** on a log alert stops that pattern on that container.
- [ ] **MUTED PATTERNS** in Settings → Alerts lists mutes and takes regex mutes for a container (or any).
- [ ] Turn on **404s are noise** for web apps that log every missing page.

## Part 4 · Notifications

- [ ] **Desktop notifications** on: one per new alert, a burst becomes one summary.
- [ ] **Quiet hours** with **FROM** and **UNTIL**: alerts still show in the app, notifications and webhooks wait.
- [ ] **WEBHOOK**: pick **ntfy**, **Discord** or **JSON POST**, paste the URL, **Save**. Tick **Send resolved messages** to hear when it's fixed.
- [ ] **Send test**.

#### Check

- [ ] The test arrives on your phone (ntfy app subscribed to your topic, or the Discord channel).

## Part 5 · Act on alerts

- [ ] **✦ Triage** on an alert gives DRIFT the usual causes and first checks.
- [ ] **+ Runbook** starts a note for that kind of alert (see [Write runbooks for alerts](kb:howto/runbooks)).

## When Nightgrid is closed

- No alerts are evaluated. Keep Nightgrid in the tray, or add an always-on watcher such as [Uptime Kuma](kb:apps/uptime-kuma) for the services that matter most.

## Limits and gotchas

- The host-offline alert needs host metrics on and waits 2 minutes.
- A public ntfy topic is readable by anyone who guesses its name. Use a long random topic or your own server (see [ntfy push notifications](kb:apps/ntfy)).

## Related

- [ntfy push notifications](kb:apps/ntfy)
- [Write runbooks for alerts](kb:howto/runbooks)
- [Search logs across all hosts](kb:howto/log-search)
- [Watch services and expiry dates](kb:howto/service-checks)
