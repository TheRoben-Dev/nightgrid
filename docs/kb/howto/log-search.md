# Search logs across all hosts

Find an error in every host's journal and container logs at once, save the searches you repeat, and turn one into an alert.

![One search over every host, saved and turned into an alert.](fig:ng-log-search)

## What you end up with

- One newest-first stream of matching lines from all hosts
- Saved searches as chips you click to rerun
- **LOG MATCH** alerts from a saved search, checked every 5 minutes

## Part 1 · Search

- [ ] [Fleet](view:fleet) → **Logs**. Type text, or press `.*` for a regular expression and `Aa` to match case.
- [ ] Pick how far back to look (up to 7 days) and **Search**. Hosts that aren't connected are connected first; a host that fails is listed with its error.
- [ ] **More filters**: a systemd unit, containers whose name contains some text, or journal priority (`3 err` and worse).
- [ ] The chips above the results show one source at a time.

## Part 2 · Good searches to save

- [ ] `Out of memory` (journal): OOM kills anywhere.
- [ ] `I/O error|blk_update_request` (regex, journal): failing disks or cables.
- [ ] `authentication failure|Failed password` (unit `ssh`): brute-force attempts.
- [ ] `panic|FATAL` (containers): apps crashing.
- [ ] **Save as…** keeps the current search under a name; **Update** saves changes, `×` deletes.

## Part 3 · Make it an alert

- [ ] Turn on **◉** on a saved search. Every 5 minutes it runs over new lines on connected hosts and raises **LOG MATCH** per host with matches.
- [ ] Keep alert searches specific; a broad one fires constantly.

#### Check

- [ ] Run `logger "nightgrid test match"` on a host and search for it.

## Limits and gotchas

- At most 2000 lines are shown; narrow the search if it says there were more.
- Lines that look like they contain a password or token are hidden.
- The journal must be persistent to search past a reboot: `sudo mkdir -p /var/log/journal` and restart `systemd-journald`.
- For long-term log storage across months, run [Grafana Loki](kb:apps/loki); Nightgrid searches live logs.

## Related

- [Reading logs](kb:basics/reading-logs)
- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Grafana Loki for long-term logs](kb:apps/loki)
