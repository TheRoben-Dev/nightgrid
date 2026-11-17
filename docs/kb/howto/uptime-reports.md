# Uptime history and monthly reports

Read 30- and 90-day uptime for every host and service, see each outage with its cause, and export a monthly availability report.

![From uptime bars to a monthly report.](fig:ng-uptime-report)

## What you end up with

- Daily bars per host and service (green, yellow, red, grey for unknown)
- Outage lists with duration and the error each started with
- A monthly HTML report against a target such as 99.9%

## Part 1 · Uptime

- [ ] [Fleet](view:fleet) → **Uptime**: one bar per day for 30 or 90 days. Hover a bar for that day; click a row for its outages.
- [ ] Uptime is recorded while Nightgrid runs, including in the tray. A host asleep or rebooting on purpose isn't down.

## Part 2 · Monthly report

- [ ] **Monthly report…**: pick a month and a target. Each host and service shows availability, downtime, outage count and the longest outage, and whether it met the target.
- [ ] **Export HTML…** saves it as one page; print or save it as PDF from your browser.

## Part 3 · Metric history

- [ ] Click a host on the [Overview](view:overview) for charts from 1 hour to 1 year. Drag across a chart to zoom; double-click to reset.
- [ ] Minute detail is kept for the days set in **Settings → Monitoring**, 15-minute averages for at least 90 days and hourly for 400 days.
- [ ] GPUs (NVIDIA through `nvidia-smi`, AMD amdgpu) get their own charts.

#### Check

- [ ] Last month's report opens and lists any outage you remember.

## Limits and gotchas

- Availability only counts time Nightgrid was watching. Long periods with the app closed show grey.
- History lives on this computer and isn't in profile exports.

## Related

- [Phone status page and snapshots](kb:howto/status-page)
- [Watch services and expiry dates](kb:howto/service-checks)
- [Prometheus and Grafana](kb:apps/prometheus-grafana)
