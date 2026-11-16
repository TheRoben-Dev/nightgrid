# Raspberry Pi as a home server

Make a Raspberry Pi a reliable small server for DNS, Home Assistant, a VPN or monitoring: boot from SSD, cut SD card wear, and keep it cool.

## What you end up with

- Raspberry Pi OS Lite (64-bit) with SSH key login
- An SSD as the boot drive (Pi 4/5) or a durable card with fewer writes
- Temperature and throttling visible in Nightgrid

## Part 1 · Install

- [ ] Use Raspberry Pi Imager: OS Lite 64-bit, and in its settings set the hostname, your user, your SSH public key and locale.
- [ ] Pi 4/5: boot from a USB SSD (or NVMe on a Pi 5 HAT). Update the bootloader first: `sudo rpi-eeprom-update -a`.
- [ ] Add it in Nightgrid; key login works from the first boot.

## Part 2 · Fewer writes

- [ ] Journal in RAM or capped: see [Journald: keep logs, but not forever](kb:linux/journald) (`SystemMaxUse=100M`, or `Storage=volatile` on SD).
- [ ] Swap: zram instead of a swap file ([Swap and zram](kb:linux/swap-zram)).
- [ ] `noatime` on the root mount in `/etc/fstab`.

## Part 3 · Power and heat

- [ ] Use the official power supply; undervoltage causes random corruption.
- [ ] Check throttling:

```sh
vcgencmd get_throttled
vcgencmd measure_temp
```

- [ ] A command widget on the Overview with `vcgencmd get_throttled`, read as text: anything but `throttled=0x0` deserves a look.

## Part 4 · Workloads

- [ ] Docker works on arm64; most images publish arm64 variants. [Prepare a Linux Docker host](kb:apps/docker-host) applies (use the Debian instructions).

#### Check

- [ ] The Pi shows on the Overview with temperature, and `get_throttled` reads `0x0` under load.

## Limits and gotchas

- Some images are amd64-only; `docker manifest inspect <image>` lists architectures.
- SD cards fail without warning. Back up `/etc` and app data off the Pi.

## Related

- [Journald: keep logs, but not forever](kb:linux/journald)
- [Pi-hole](kb:apps/pihole)
- [Build dashboards with command widgets](kb:howto/command-widgets)
