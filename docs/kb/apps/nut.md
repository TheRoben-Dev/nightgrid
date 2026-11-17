# NUT server for your UPS

Set up Network UPS Tools on the machine the UPS is plugged into, share it with your other servers, and have every server shut itself down cleanly on low battery, with or without Nightgrid running.

![The UPS host runs the NUT server; other hosts watch it and shut down in time.](fig:nut-network)

## What you end up with

- `upsd` reading the UPS over USB and serving it on the LAN
- `upsmon` on every server, shutting down at low battery
- The UPS in Nightgrid's Fleet → UPS with alerts and a group shutdown plan

## Part 1 · The UPS host

- [ ] `sudo apt install -y nut`, then find the UPS: `sudo nut-scanner -U`.
- [ ] `/etc/nut/ups.conf`:

```ini
[ups]
    driver = usbhid-ups
    port = auto
    desc = "Rack UPS"
```

- [ ] `/etc/nut/nut.conf`: `MODE=netserver`.
- [ ] `/etc/nut/upsd.conf`: `LISTEN 0.0.0.0 3493`.
- [ ] `/etc/nut/upsd.users`:

```ini
[monuser]
    password = <a-password>
    upsmon primary
```

- [ ] `/etc/nut/upsmon.conf`: `MONITOR ups@localhost 1 monuser <a-password> primary`.
- [ ] `sudo systemctl restart nut-server nut-monitor` and test: `upsc ups@localhost`.
- [ ] Allow TCP 3493 from your LAN in [System](view:system) → **FIREWALL**.

## Part 2 · Other servers

- [ ] `sudo apt install -y nut-client`, `MODE=netclient` in `nut.conf`, and in `upsmon.conf`:

```
MONITOR ups@ups-host.example.lan 1 monuser <a-password> secondary
```

- [ ] `sudo systemctl restart nut-monitor`. Secondaries shut down first when the battery is low; the primary waits for them.

## Part 3 · In Nightgrid

- [ ] [Fleet](view:fleet) → **UPS** → **+ UPS**: the UPS host and `ups@localhost`, **Test**.
- [ ] Optional earlier shutdown of a group (for example everything non-essential after 5 minutes on battery): see [UPS monitoring with NUT](kb:howto/ups).

#### Check

- [ ] `upsc ups@ups-host.example.lan` works from a secondary, and Nightgrid shows charge and runtime.

## When Nightgrid is closed

- `upsmon` on each server handles low-battery shutdowns by itself. Nightgrid adds alerts and an earlier, ordered group shutdown while it runs.

## Limits and gotchas

- Test with a real pull of the plug at a quiet time, once.
- Some consumer UPS models report runtime badly; set `override.battery.charge.low` in `ups.conf` if they shut down too late.
- NAS units with built-in NUT often use user `monuser` with a fixed password; check the NAS docs.

## Related

- [UPS monitoring with NUT](kb:howto/ups)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
