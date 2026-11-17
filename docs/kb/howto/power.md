# Power schedules, Wake-on-LAN and group shutdown

Shut machines down at night, wake them in the morning or on demand from anywhere, and power off a whole group in one step.

![Wake-on-LAN sent by an always-on host on the same LAN.](fig:ng-wake)

## What you end up with

- Shutdown, suspend, reboot and wake schedules as systemd timers on the hosts
- **WAKE** from Nightgrid, sent from another host on the same LAN so it works away from home
- Hosts that are off on purpose shown as **ASLEEP** or **SHUT DOWN**, not as outages

## Part 1 · One host by hand

- [ ] [System](view:system) toolbar: **SUSPEND…**, **SHUT DOWN…**, **REBOOT…** after typing the host's name.
- [ ] **GROUP…** does the same for every host in a group, one at a time, after you type the group's name.
- [ ] A host whose sudo needs a password Nightgrid doesn't remember is skipped and listed. Tick "remember" once from its own System page, or allow passwordless `systemctl` for it.

## Part 2 · Wake-on-LAN

- [ ] Enable Wake-on-LAN in the machine's BIOS/UEFI and in Linux:

```sh
sudo ethtool -s <nic> wol g
```

- [ ] Make it stick with a NetworkManager setting (`nmcli connection modify <conn> 802-3-ethernet.wake-on-lan magic`) or a systemd `.link` file with `WakeOnLan=magic`.
- [ ] Put the host's **MAC address** in the host editor (the [Network](view:network) map shows MACs).
- [ ] Set **Wake from** to an always-on host on the same LAN.
- [ ] **WAKE** in the System toolbar or ⏻ on its overview tile.

## Part 3 · Schedules

- [ ] System → Schedules → **POWER SCHEDULE**: Shut down, Suspend, Reboot or Wake up, a time on the host's clock, and days. **ADD**.
- [ ] It's a systemd timer on the host, so it runs with Nightgrid closed. Scheduled wake-ups run on the **Wake from** host.
- [ ] **Sleep window** in the host editor (`23:00-07:00`) shows the host as **ASLEEP** at night instead of alerting.

#### Check

- [ ] Shut a host down, then **WAKE** it: it comes back and its status returns to normal on its own.

## When Nightgrid is closed

- Scheduled shutdowns and wake-ups keep working (they're timers on the hosts). Manual **WAKE** needs Nightgrid.

## Limits and gotchas

- Wake-on-LAN rarely works over Wi-Fi.
- Some boards only wake from S3 sleep or S5 off when "ErP" or "deep sleep" is disabled in the firmware.
- A reboot that takes over 15 minutes counts as an outage.

## Related

- [UPS monitoring with NUT](kb:howto/ups)
- [Schedule jobs with cron and timers](kb:howto/schedules)
- [Game server host](kb:guides/game-server-host)
