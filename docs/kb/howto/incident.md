# Handle an outage step by step

Something is down. This walk-through takes you from the first notification to the fix and a runbook for next time, using the views in the order that usually finds the cause fastest.

## Part 1 · Find the cause, not the symptoms

- [ ] Open the alerts list (the ALERTS counter in the status bar). With dependencies set, follow-on alerts are folded under the cause ("explains 3 more alerts"). Start with the top one.
- [ ] **▤ Runbook** on the alert, if you wrote one, has the steps. Otherwise **✦ Triage** gives DRIFT's usual causes and first checks.

## Part 2 · Host down?

- [ ] The host's row says **OFFLINE**, **ASLEEP** or an error. See [When a host won't connect](kb:howto/troubleshoot-connection).
- [ ] Proxmox guest? [Proxmox](view:proxmox) → GUESTS shows its state; **Start** or **Console**.
- [ ] Physical box off? **WAKE** in the [System](view:system) toolbar if Wake-on-LAN is set up.

## Part 3 · Container or service down?

- [ ] [Docker](view:docker): is it exited, restarting or unhealthy? Open its logs; **✦ What broke?** on a log alert or **✦ Explain** on the logs.
- [ ] [System](view:system) → **timeline** on that host: a deploy, upgrade, OOM kill or reboot just before is the likely cause.
- [ ] Recent update? **Snapshots** → **Roll back…** in Docker gets you running again, then investigate.
- [ ] Proxy error (502/504)? The app behind it is the problem, not the proxy ([Manage reverse proxy routes](kb:howto/proxy-routes)).

## Part 4 · Disk, memory or network?

- [ ] Disk full: [Fleet](view:fleet) → **Disks**, then free space (logs, old images: `docker image prune -a`).
- [ ] Memory: the host chart and OOM kills in the timeline ([Why is this server slow?](kb:linux/slow-server)).
- [ ] Network: [Fleet](view:fleet) → **Speed** and the [Network](view:network) map.

## Part 5 · Search wider

- [ ] [Fleet](view:fleet) → **Logs** across all hosts for the error text; one failing component often logs on several machines.

#### Check

- [ ] The alert clears by itself once the check passes again, and the service is green in [Services](view:services).

## Part 6 · Make next time faster

- [ ] **+ Runbook** on the alert kind, or **Save as runbook note** under the DRIFT answer that helped.
- [ ] Add a missing dependency (Settings → Alerts → Dependencies) if you got more notifications than causes.
- [ ] Add a log alert (saved search with **◉**) for the error that first showed the problem.

## Related

- [Write runbooks for alerts](kb:howto/runbooks)
- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Track config drift and host history](kb:howto/config-drift)
