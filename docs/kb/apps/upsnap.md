# UpSnap wake-on-LAN dashboard

A small web page with a button per machine: wake it, see if it's up, shut it down, or schedule wake-ups. Useful for family members who need to turn on the gaming PC or the NAS without asking you, and for phones on the LAN.

## What you end up with

- UpSnap on an always-on host, with your devices and their MAC addresses
- Wake, status ping and optional shutdown per device
- Cron-style wake schedules and per-user permissions

## Part 1 · The stack

- [ ] UpSnap needs the host's network to send Wake-on-LAN broadcasts. [Docker](view:docker) → **+ New stack** named `upsnap`:

```yaml
services:
  upsnap:
    image: ghcr.io/seriousm4x/upsnap:5
    restart: unless-stopped
    network_mode: host
    environment:
      TZ: Etc/UTC
    volumes:
      - ./data:/app/pb_data
```

- [ ] Open `http://<host>:8090`, create the admin account.

## Part 2 · Devices

- [ ] **+** → name, IP and MAC address. **Scan** can find devices on the network for you (needs `nmap` in the container, which the image includes).
- [ ] Optional **shutdown command**, run from the UpSnap container, for example SSH to the device with a key mounted into the container, or `net rpc shutdown` for Windows.
- [ ] **Wake cron**: `0 7 * * 1-5` wakes a workstation on weekday mornings.
- [ ] **Users**: add family members and give each only the devices they may wake.

#### Check

- [ ] Shut a device down, press its button in UpSnap: its dot turns green when it answers pings again.

## In Nightgrid

- Nightgrid already wakes hosts with **WAKE** in [System](view:system) (with a **Wake from** host on the LAN) and runs power schedules as systemd timers. UpSnap is for devices that aren't Nightgrid hosts (gaming PCs, a Windows box) and for people who don't use Nightgrid.
- Add `http://<host>:8090` to [Services](view:services). With host networking there's no **⧉** button.

## Limits and gotchas

- Wake-on-LAN must be enabled in the device's BIOS and network driver; see [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power).
- Magic packets don't cross routers or VLANs. Run UpSnap on the same network segment as the devices.
- The data folder holds password hashes and device list; back it up with the compose file.

## Related

- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [Cut a home server's power draw](kb:linux/power-saving)
- [Mini PC homelab](kb:guides/mini-pc-homelab)
