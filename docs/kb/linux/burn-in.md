# Burn in a server: CPU, memory and stability

A new build, a used server off eBay or a machine back from repair: load it hard for a few hours and watch for errors and heat before it runs anything you care about.

## What you end up with

- Memory checked for bad cells
- CPU and VRMs proven under full load, with temperatures recorded
- Kernel logs free of machine-check errors

## Before you start

- `sudo apt install stress-ng memtester lm-sensors` (Fedora: `dnf install stress-ng memtester lm_sensors`)
- A USB stick with Memtest86+ for the boot-time memory test (the Debian package `memtest86+` adds it to GRUB instead)

## Part 1 · Memory, before the OS

- [ ] Boot Memtest86+ (GRUB menu entry, or the USB stick) and let it run at least one full pass; four passes overnight for a server with lots of RAM.
- [ ] Any error is a fail. Test sticks one at a time to find the bad one, and recheck XMP/EXPO settings: an unstable memory profile looks exactly like bad RAM.

## Part 2 · Memory, inside Linux

- [ ] A quick check of most free memory while the OS runs (here 80% of 16 GB):

```sh
sudo memtester 12G 2
```

- [ ] ECC machines: corrected errors show in `sudo journalctl -k | grep -i -E "edac|mce"` and `sudo apt install rasdaemon && ras-mc-ctl --summary`.

## Part 3 · CPU and power delivery

- [ ] Start temperature logging in one terminal:

```sh
sudo sensors-detect --auto
watch -n 2 sensors
```

- [ ] Load every core, with checks on the results, for an hour:

```sh
stress-ng --cpu 0 --cpu-method matrixprod --verify --timeout 1h --metrics-brief
```

- [ ] Add memory pressure and disk I/O for a mixed load:

```sh
stress-ng --cpu 0 --vm 2 --vm-bytes 75% --hdd 1 --verify --timeout 2h
```

- [ ] Watch for: thermal throttling (clocks dropping in `watch -n1 "grep MHz /proc/cpuinfo"`), package temps near TjMax, and any `--verify` failure.

## Part 4 · Read the logs

```sh
sudo journalctl -k -p warning --since "3 hours ago"
sudo dmesg | grep -i -E "mce|machine check|hardware error|thermal"
```

#### Check

- [ ] Memtest86+ passes clean, stress-ng finishes with no failed verifications, no MCE or thermal messages, and temperatures level off below the CPU's limit.

## In Nightgrid

- Watch the host's CPU, memory and temperature live on [Overview](view:overview) while the test runs; the monitor chart keeps the curve for later.
- Run the same burn-in on several machines at once with a snippet in [Automation](view:automation).
- Keep results in a [Knowledge](view:notes) note per machine.

## Limits and gotchas

- A pass proves the machine is stable today at today's room temperature. Summer adds 5 to 10 °C.
- Laptops and mini PCs will throttle under stress-ng by design; what matters is that they don't crash.
- Don't burn in a machine whose fans or cooler you haven't checked: a seized fan plus full load can cook a VRM.

## Related

- [Test a drive before you trust it](kb:linux/disk-testing)
- [Read fans and sensors](kb:linux/sensors-fans)
- [Inventory a machine's hardware](kb:linux/hardware-inventory)
