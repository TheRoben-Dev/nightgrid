# Read fans and sensors

See CPU, board, drive and NVMe temperatures and fan speeds on a Linux server, and quiet down fans on boards that let Linux control them.

## What you end up with

- Every temperature and fan the board exposes, with names you understand
- Drive temperatures without SMART tools
- A fan curve where the BIOS one is too loud

## Before you start

- `sudo apt install lm-sensors fancontrol` (Fedora: `lm_sensors`)

## Part 1 · Find the sensors

```sh
sudo sensors-detect --auto
sensors
```

- [ ] `k10temp` (AMD) and `coretemp` (Intel) are the CPU; `nvme-pci-*` the NVMe drives; `drivetemp` adds SATA drive temperatures after `sudo modprobe drivetemp`.
- [ ] Board chips like `nct6775` or `it87` hold fan speeds and voltages. If `sensors-detect` finds one but no fans appear, the driver may need loading at boot: add it to `/etc/modules`.

## Part 2 · Make sense of the names

- [ ] Labels like `temp3` or `in4` depend on the board. Compare readings with the BIOS hardware monitor page, then name them in `/etc/sensors.d/board.conf`:

```
chip "nct6798-*"
  label temp1 "Motherboard"
  label fan2 "CPU fan"
  ignore temp7
```

## Part 3 · Fan control

- [ ] Set the fan headers to manual or PWM mode in the BIOS first.
- [ ] `sudo pwmconfig` maps each PWM output to its fan by stopping them one at a time, then writes `/etc/fancontrol`.
- [ ] Start it: `sudo systemctl enable --now fancontrol`.

#### Check

- [ ] Under load (`stress-ng --cpu 0 --timeout 2m`) the fans spin up and temperatures settle; idle, they slow down again.

## In Nightgrid

- [Overview](view:overview) reads temperatures with each sample, and the monitor chart keeps the curve, so you can see a fan change helped.
- Nightgrid has no temperature alert. For a closet NAS, save a [Fleet](view:fleet) → **Logs** search for `thermal|critical temperature` (journal, regex) and turn on its **◉** alert: the kernel logs throttling there.
- A command widget on the Overview can show one sensor line (see [Make Overview tiles from commands](kb:howto/command-widgets)).

## Limits and gotchas

- Many prebuilt and laptop boards don't expose fan control at all; Dell, HP and Lenovo servers manage fans through the BMC.
- `pwmconfig` briefly stops fans; don't run it under load.
- Don't set a curve that lets drives sit above about 50 °C for long.

## Related

- [Burn in a server: CPU, memory and stability](kb:linux/burn-in)
- [Manage a server with IPMI](kb:linux/ipmi)
- [Keep an eye on disks and pools](kb:howto/disks)
