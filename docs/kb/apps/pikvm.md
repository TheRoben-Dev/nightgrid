# PiKVM remote console

Control a server's keyboard, video and mouse over the network, from power-on: change BIOS settings, watch a boot hang, reinstall the OS from an ISO, press the power button. PiKVM does on any PC what IPMI does on server boards, with a Raspberry Pi and an HDMI capture.

## What you end up with

- A PiKVM (ready-made V3/V4 or a DIY Pi 4 build) attached to one server, or to several through a KVM switch
- The server's screen in a browser, with keyboard, mouse and virtual USB media
- Power and reset buttons through an ATX adapter

## Before you start

- **Ready-made** (PiKVM V4 Mini/Plus) is the least work. **DIY** needs a Pi 4, an HDMI-to-CSI bridge (or a USB capture dongle) and a USB-C cable that can do OTG.
- Plan where it plugs in: HDMI out of the server, USB from the PiKVM into the server (for keyboard, mouse and virtual drives), network to your LAN.

## Part 1 · First boot

- [ ] Flash the PiKVM OS image for your hardware, boot it, and find its address in the router or with [Network](view:network) → **Scan from …**.
- [ ] Open `https://<pikvm>`: default web login `admin` / `admin`, SSH `root` / `root`.
- [ ] Change both right away over SSH:

```bash
rw
passwd root
kvmd-htpasswd set admin
ro
```

- [ ] The root filesystem is read-only by default (`rw` / `ro` switch it) so a power cut can't corrupt the SD card.

## Part 2 · Use it

- [ ] **KVM** shows the server's screen. Keyboard and mouse go to the server; **Ctrl+Alt+Del** and other combos are in the menu.
- [ ] **Drive**: upload an ISO and connect it as a virtual USB CD-ROM or flash drive, then boot the server from it to install an OS.
- [ ] **ATX**: with the ATX board wired to the motherboard's front-panel header, the **Power** and **Reset** buttons work, and the power LED state shows in the UI.

## Part 3 · Updates and access

- [ ] Update from SSH with `rw`, `pikvm-update`, then reboot.
- [ ] For access away from home, PiKVM's OS has a Tailscale package; install it and keep the web UI off the internet.

#### Check

- [ ] Reboot the server from the PiKVM and enter its BIOS setup from the browser.

## In Nightgrid

- Add the PiKVM itself as a host (SSH as `root`) to reach its shell from the [Terminal](view:terminal) and keep an eye on it like any other machine. Remember its root filesystem stays read-only until you run `rw`; PiKVM OS is Arch-based: [Fleet](view:fleet) → **Updates** can list pending pacman updates, but apply them with `rw` and `pikvm-update` on the device.
- Add `https://<pikvm>` to [Services](view:services). The certificate is self-signed, so expect the "untrusted" note.
- When a host goes offline in Nightgrid, the PiKVM screen shows you why (kernel panic, waiting at a disk check, a BIOS prompt).

## Limits and gotchas

- Some servers only output video on the iGPU's port when no graphics card is installed; plug HDMI into the port that shows the BIOS.
- A DIY build without a proper USB-C splitter can back-power the Pi from the server and cause odd resets.
- One PiKVM watches one screen at a time; a cheap HDMI/USB KVM switch extends it to several machines.

## Related

- [Manage a server with IPMI](kb:linux/ipmi)
- [Fix a server that won't boot](kb:linux/boot-rescue)
- [Repair bench server](kb:guides/repair-bench-server)
