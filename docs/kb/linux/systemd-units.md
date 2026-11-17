# Write your own systemd service

Turn a script or a binary into a proper service that starts at boot, restarts when it crashes, logs to the journal and shows up in Nightgrid's System → services.

![A timer, its service and the journal.](fig:ng-timers)

## What you end up with

- A unit file in `/etc/systemd/system`
- Automatic restarts with back-off, a dedicated user and basic sandboxing
- Logs in `journalctl -u <name>`

## Part 1 · A minimal service

- [ ] Create `/etc/systemd/system/myapp.service` (with `sudoedit`, which opens in Nightgrid's editor when **Edit server files in Nightgrid** is on):

```ini
[Unit]
Description=My app
Wants=network-online.target
After=network-online.target

[Service]
User=myapp
WorkingDirectory=/opt/myapp
ExecStart=/opt/myapp/bin/myapp --port 8080
Restart=on-failure
RestartSec=5
EnvironmentFile=-/etc/myapp/env

[Install]
WantedBy=multi-user.target
```

- [ ] Create the user: `sudo useradd --system --home /opt/myapp --shell /usr/sbin/nologin myapp`.
- [ ] Load and start:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now myapp
systemctl status myapp
```

## Part 2 · Sandboxing that rarely breaks anything

- [ ] Add under `[Service]`, then restart and test:

```ini
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/myapp/data
```

- [ ] `systemd-analyze security myapp` scores the unit and lists more options.

## Part 3 · Overrides instead of edits

- [ ] To change a packaged unit (nginx, docker), don't edit its file in `/lib/systemd/system`. Use a drop-in:

```sh
sudo systemctl edit nginx
```

- [ ] That opens an override file; settings there survive package updates.

## Part 4 · In Nightgrid

- [ ] [System](view:system) → **services** lists the unit with start, stop, restart, enable and disable.
- [ ] [System](view:system) → **journal** filtered to the unit shows its logs live; **✦ Explain** sends errors to DRIFT.
- [ ] Log alerts (Settings → Alerts → Log anomalies) cover journal errors too.

#### Check

- [ ] `sudo kill -9 $(systemctl show -p MainPID --value myapp)`: the service comes back within 5 seconds.

## Limits and gotchas

- `Restart=always` restarts even on a clean exit; `on-failure` is usually what you want.
- `StartLimitBurst` stops restarts after repeated crashes in a short time. `systemctl reset-failed myapp` clears it.
- `After=network-online.target` only helps if a wait-online service is enabled (NetworkManager or systemd-networkd provide one).

## Related

- [Schedule jobs with cron and timers](kb:howto/schedules)
- [Reading logs](kb:basics/reading-logs)
- [Journald: keep logs, but not forever](kb:linux/journald)
