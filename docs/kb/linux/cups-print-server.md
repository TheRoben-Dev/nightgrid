# Share a printer with CUPS

Turn a USB printer (or an old network printer with poor drivers) into one every device can use: Linux, macOS and iOS find it with AirPrint, Windows adds it by address, and nobody installs vendor software.

![One CUPS server sharing a printer.](fig:lx-cups)

## What you end up with

- CUPS on an always-on server with the printer shared on the LAN
- AirPrint/IPP Everywhere discovery for phones and Macs
- A web admin page on port 631

## Before you start

- The printer connected by USB to the server, or reachable on the network.
- Check whether the printer is "driverless" (IPP Everywhere or AirPrint): most printers from the last ten years are, and then no driver is needed at all.

## Part 1 · Install

- [ ] On Debian or Ubuntu:

```bash
sudo apt install cups cups-browsed avahi-daemon printer-driver-all
sudo usermod -aG lpadmin $USER
```

- [ ] `printer-driver-all` pulls in Gutenprint, HPLIP, foo2zjs and others for older printers. Skip it for driverless printers.
- [ ] Allow admin and sharing from the LAN:

```bash
sudo cupsctl --remote-admin --remote-any --share-printers
sudo systemctl restart cups
```

## Part 2 · Add the printer

- [ ] Open `https://<server>:631/admin` (accept the self-signed certificate) → **Add Printer**, log in with your Linux user.
- [ ] Pick the USB printer or the **IPP Everywhere** entry for a network one. For older printers choose the matching driver (HPLIP for HP, Gutenprint for many inkjets).
- [ ] Tick **Share This Printer**, give it a short name without spaces (`office`).
- [ ] Print a test page from the printer's page → **Maintenance → Print Test Page**.

## Part 3 · Clients

- [ ] **macOS, iOS, Linux**: the printer appears by itself through Avahi (mDNS). If not, check `avahi-daemon` is running and UDP 5353 isn't blocked.
- [ ] **Windows**: Add printer → "The printer that I want isn't listed" → **Select a shared printer by name** → `http://<server>:631/printers/office`, then pick **Microsoft IPP Class Driver** or a generic driver.
- [ ] **Android**: the built-in Default Print Service finds IPP printers on the same network.

#### Check

- [ ] A phone on Wi-Fi sees the printer without any app and prints a page.

## When Nightgrid is closed

- CUPS runs on the server on its own. Nightgrid only helps you set it up and watch it.

## In Nightgrid

- [System](view:system) → **services** shows `cups` and `avahi-daemon`; a failed unit appears there and in the host's timeline.
- Add `https://<server>:631` to [Services](view:services) to know when the print server is down.
- [Network](view:network) → **Scan from …** reads mDNS, so network printers show up on the map with their current address.

## Limits and gotchas

- If the firewall is on, allow TCP 631 and UDP 5353 from the LAN only ([System](view:system) → **firewall** → **Add…** on ufw hosts).
- USB printers that go into deep sleep sometimes disappear from USB; `lsusb` shows whether the server still sees it. Setting the printer's auto-off to "never" helps.
- Scanners on multifunction printers are a separate service (SANE with `saned`, or the `scanservjs` web UI).

## Related

- [Read fans and sensors](kb:linux/sensors-fans)
- [Write your own systemd service](kb:linux/systemd-units)
- [Raspberry Pi as a home server](kb:linux/raspberry-pi)
