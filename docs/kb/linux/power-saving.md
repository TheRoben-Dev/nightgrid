# Cut a home server's power draw

A server that idles at 60 W instead of 25 W costs real money over a year. Measure, then let the CPU, disks and devices sleep when there's nothing to do.

![Where the power savings are.](fig:lx-power)

## What you end up with

- Idle power measured at the wall, before and after
- CPU and PCIe power management turned on
- Disks that spin down when they're not needed

## Before you start

- A plug-in power meter (the single most useful tool here)
- `sudo apt install powertop linux-cpupower hdparm` (Fedora: `kernel-tools` for cpupower)

## Part 1 · Measure and find the culprits

- [ ] Note the idle watts at the wall with everything running as usual.
- [ ] `sudo powertop`: the **Idle stats** tab shows how often the CPU reaches deep package C-states (C6, C8, C10). Stuck at C2 or C3 means something blocks deeper sleep.
- [ ] Common blockers: some add-in NICs and SATA controllers without ASPM, a dedicated GPU, and USB devices that never sleep.

## Part 2 · Turn on power management

- [ ] Try powertop's suggestions once: `sudo powertop --auto-tune`, then measure again.
- [ ] To keep them, a small unit `/etc/systemd/system/powertop.service`:

```ini
[Unit]
Description=powertop auto-tune

[Service]
Type=oneshot
ExecStart=/usr/sbin/powertop --auto-tune

[Install]
WantedBy=multi-user.target
```

- [ ] `sudo systemctl enable --now powertop`. If a USB keyboard or NIC misbehaves afterwards, that device's autosuspend is the cause; exclude it with a udev rule instead of skipping auto-tune.
- [ ] CPU governor: `cpupower frequency-info`. `powersave` with `intel_pstate`/`amd_pstate` still boosts under load; `performance` keeps clocks high for little gain on a home server.
- [ ] BIOS: enable C-states, ASPM (PCIe power management), and disable anything you don't use (extra SATA controller, audio, serial).

## Part 3 · Disks

- [ ] Spin down a data HDD after 20 minutes idle (`-S 240` = 240 × 5 s):

```sh
sudo hdparm -S 240 /dev/disk/by-id/ata-MODEL_SERIAL
```

- [ ] Make it permanent in `/etc/hdparm.conf`. Only for disks that really sit idle; spinning up every few minutes wears them faster than spinning.

#### Check

- [ ] The wall meter shows a lower idle number, and powertop reaches deeper package C-states.

## In Nightgrid

- Nightgrid samples hosts over SSH while it's open; a longer **SAMPLE EVERY** in **Settings → Monitoring** lets them idle longer between samples.
- Monitoring commands that read SMART can wake sleeping disks; [Fleet](view:fleet) → **Disks** checks only when you click or hourly while shown.
- A server that only needs to be on in the evening can sleep or shut down on a schedule: [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power).

## Limits and gotchas

- ZFS pools rarely let disks sleep (they write regularly); mergerfs pools with SnapRAID suit spin-down much better.
- Some NICs drop link with aggressive power saving; test network transfers after auto-tune.

## Related

- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [Read fans and sensors](kb:linux/sensors-fans)
- [mergerfs and SnapRAID](kb:apps/mergerfs-snapraid)
