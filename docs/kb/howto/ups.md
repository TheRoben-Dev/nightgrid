# UPS monitoring with NUT

Watch a UPS's charge, runtime and load in Nightgrid, get alerted on battery, and shut a group of hosts down cleanly when the power stays out.

![What happens from a power cut to a clean group shutdown.](fig:ng-ups-shutdown)

## What you end up with

- Every UPS with state, charge, runtime, load and input voltage, read every 30 seconds
- **ON BATTERY** and "replace battery" alerts
- A countdown banner and an ordered group shutdown when the outage lasts

## Before you start

- NUT running on the host the UPS is plugged into by USB, or a NAS with a built-in NUT server. Server-side setup: [NUT server for your UPS](kb:apps/nut).
- Settings → **FEATURES**: UPS is off in the Standard preset; turn it on.

## Part 1 · Add the UPS

- [ ] [Fleet](view:fleet) → **UPS** → **+ UPS**.
- [ ] Pick the host and the **NUT name**: `ups@localhost`, or `ups@nas.example.lan` for a NAS's NUT server (any host can read it). `upsc -l` on the host lists names.
- [ ] **Test** reads it once.

## Part 2 · Shutdown plan

- [ ] Tick **Shut a group down when the power stays out**, pick a host group and when to act: after some minutes on battery, when runtime drops under a limit, and always when the UPS reports low battery.
- [ ] Type the group's name to arm it.
- [ ] When it triggers, a red banner counts down (2 minutes by default) and names the hosts. **Cancel shutdown** stops it; the power returning also stops it.
- [ ] Hosts shut down one by one, the UPS's own host last.

#### Check

- [ ] Pull the UPS plug briefly: **ON BATTERY** fires. Plug it back before the shutdown threshold.

## When Nightgrid is closed

- Nightgrid's group shutdown needs the app running (the tray is enough). For protection that never depends on this computer, also configure `upsmon` on every server so each shuts itself down (see [NUT server for your UPS](kb:apps/nut)).

## Limits and gotchas

- A host whose sudo needs a password that isn't remembered can't be shut down unattended; it's listed in the final notification. Allow passwordless `shutdown` there.
- Runtime estimates are optimistic on old batteries. Act on time-on-battery, not just runtime.

## Related

- [NUT server for your UPS](kb:apps/nut)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [Tune alerts so only real problems notify](kb:howto/alerts)
