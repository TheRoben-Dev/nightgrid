# OpenWrt router

Replace a router's stock firmware (or run on a small x86 box) with OpenWrt: a real Linux router with a package manager, VLANs, WireGuard, ad blocking and SQM to fix bufferbloat, configured from the LuCI web UI or the `uci` command line.

![SQM shapes the line inside the router.](fig:app-openwrt-sqm)

## What you end up with

- OpenWrt installed on supported hardware, with a backup of its config
- SSH key login and the router reachable from Nightgrid's terminal
- SQM for lag-free gaming and calls while the line is busy, and optional WireGuard

## Before you start

- Check the **Table of Hardware** on the OpenWrt site for your exact model and hardware revision. A wrong image can brick a router.
- Have a way back: the stock firmware file, and the device's recovery method (TFTP, a recovery web page or serial).

## Part 1 · Install and secure

- [ ] Flash the **factory** image from the stock firmware's upgrade page (later upgrades use **sysupgrade** images).
- [ ] Open `http://192.168.1.1` (LuCI), set a root password. **System → Administration → SSH-Keys**: paste your public key, and set Dropbear to listen on the LAN interface only.
- [ ] **System → Backup / Flash Firmware → Generate archive**. Do it again after every change you care about.

## Part 2 · Packages

- [ ] **System → Software** (LuCI) or SSH. OpenWrt uses `opkg` up to 24.10 and `apk` from 25.x on:

```bash
opkg update && opkg install luci-app-sqm
apk update && apk add luci-app-sqm
```

- [ ] Useful packages: `luci-app-sqm` (bufferbloat), `luci-proto-wireguard` (VPN), `luci-app-adblock` or `adblock-lean` (DNS ad blocking), `openssh-sftp-server` (lets Nightgrid's Files view work).

## Part 3 · SQM against bufferbloat

- [ ] Run a bufferbloat test (for example Waveform's) first and note the grade.
- [ ] **Network → SQM QoS**: interface = WAN, download and upload set to about 90 % of your measured speeds, queue discipline `cake` with `piece_of_cake.qos`.
- [ ] Test again: latency under load should barely rise.

## Part 4 · Updates

- [ ] **Attended Sysupgrade** (`luci-app-attendedsysupgrade`) builds an image with your installed packages and keeps your settings. Back up first.

#### Check

- [ ] A bufferbloat test shows A or better, a config backup is stored off the router, and SSH with your key works from your computer.

## In Nightgrid

- Add the router as a host (user `root`). The [Terminal](view:terminal) works fully; [Files](view:files) needs `openssh-sftp-server`. OpenWrt is BusyBox without systemd, so views built for full Linux servers (System services, Fleet updates) show little or nothing for it.
- Add `http://192.168.1.1` to [Services](view:services), and the router as a dependency of everything else in **Settings → Alerts → Dependencies**: when it's down, you get one alert instead of twenty.
- [Network](view:network) → **Tools** run from a LAN host checks DNS, ports and paths through the router.

## Limits and gotchas

- Flash and RAM are small on most routers; install only what you need. An x86 mini PC avoids this.
- Hardware offloading and SQM don't mix well on some chips; SQM may cap speed on gigabit lines with weak CPUs.
- Some Wi-Fi chips have weaker open-source drivers than the stock firmware; check the model's wiki page.

## Related

- [OPNsense firewall in a VM](kb:apps/opnsense)
- [WireGuard VPN server](kb:apps/wireguard)
- [VLANs and bonded NICs on a server](kb:linux/vlans-bonds)
