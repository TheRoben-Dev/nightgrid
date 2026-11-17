# Write runbooks for alerts

Turn "what do I do when this breaks" into notes that open from the alert itself, with commands you can send to the right host's terminal and steps you tick off.

## What you end up with

- Knowledge notes tied to alert kinds, a host or a service
- **▤ Runbook** on matching alerts in the alerts list
- Steps as checkboxes and commands with **Send to terminal**

## Part 1 · Start from an alert

- [ ] Open the alerts list (the ALERTS counter in the status bar). An alert without a runbook offers **+ Runbook**, which starts a note for that kind of alert with a step list.
- [ ] Or create a note in [Knowledge](view:notes) and tick **Runbook for alerts**, then pick the alert kinds, and optionally a host and a service.

## Part 2 · Write it so it works at 3 a.m.

- [ ] First line: what this alert usually means.
- [ ] `- [ ] step` lines become checkboxes (they reset each time you open the note).
- [ ] Put commands in fenced code blocks. Each block gets **Send to terminal** (typed at the prompt of a shell on the note's host; you press Enter) and **Copy**.
- [ ] Link views with `[open Docker](view:docker)` and articles with `[Reading logs](kb:basics/reading-logs)`.

A short example, as it looks once saved: a first line saying "Disk almost full on the media host.", then:

- [ ] See what grew:

```sh
sudo du -xh --max-depth=2 /srv | sort -h | tail -20
```

- [ ] Clear old Docker images:

```sh
docker image prune -a --filter "until=168h"
```

- [ ] If it's downloads, check the [Docker](view:docker) logs of the download client.

## Part 3 · Use it

- [ ] When the alert fires, **▤ Runbook** opens the note.
- [ ] **✦ Triage** tells DRIFT about your runbook too.
- [ ] After an incident, **Save as runbook note** under a DRIFT answer turns the conversation into a note to tidy up.

## Limits and gotchas

- Saving refuses passwords and tokens; **Mask them** swaps values for `••••`.
- A runbook with no kinds, host or service attached shows on no alert.
- Notes travel in encrypted profile exports.

## Related

- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Work faster in the terminal](kb:howto/terminal)
- [Get the most out of DRIFT](kb:howto/drift)
