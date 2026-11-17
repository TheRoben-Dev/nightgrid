# netboot.xyz network boot menu

Boot any PC on the LAN into Linux installers, live systems and rescue tools over the network, without making USB sticks. A repair bench favourite: memtest, Clonezilla, GParted, Ubuntu, Debian and many more, all from one menu.

![The PXE boot sequence.](fig:app-pxe)

## What you end up with

- The netboot.xyz container serving the boot files over TFTP
- Your DHCP server telling PXE clients where to boot from
- A local mirror of the images you use often, for fast boots

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `netbootxyz`:

```yaml
services:
  netbootxyz:
    image: lscr.io/linuxserver/netbootxyz:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
    volumes:
      - ./config:/config
      - ./assets:/assets
    ports:
      - "3000:3000"
      - "69:69/udp"
      - "8080:80"
```

- [ ] Port 3000 is the web UI for editing menus and downloading local assets; keep it on the LAN only.

## Part 2 · DHCP

- [ ] On your DHCP server set the **next-server / TFTP server** to the netboot host's IP and the **boot file name** by client type:
  - legacy BIOS: `netboot.xyz.kpxe`
  - UEFI: `netboot.xyz.efi`
- [ ] OPNsense, pfSense and OpenWrt have these fields in their DHCP settings; Pi-hole's or a dnsmasq DHCP server use `dhcp-boot=`, with `dhcp-match` lines to pick the file by client architecture.
- [ ] Consumer routers often can't set the boot file. Then run DHCP from Pi-hole or dnsmasq instead, or use a netboot.xyz USB or ISO that chains to the network.

## Part 3 · Local assets

- [ ] Web UI → **Local Assets**: download the images you use (Clonezilla, GParted, memtest, a live distro). They're stored in `assets` and served from port 8080.
- [ ] Web UI → **Menus**: in `boot.cfg` set `live_endpoint` to `http://<host>:8080`, so those images load from your server instead of the internet.

#### Check

- [ ] Set a test PC to network boot (often F12 at power-on): it gets an address, shows the netboot.xyz menu, and a locally mirrored tool boots in seconds.

## In Nightgrid

- **⧉** in [Docker](view:docker) opens the web UI on port 3000.
- Watch the `assets` folder's size in [Fleet](view:fleet) → **Disks**; live images add up quickly.

## Limits and gotchas

- Secure Boot blocks the netboot.xyz EFI loader on most PCs; turn it off for network booting, or boot a signed ISO from USB instead.
- Only one DHCP server should answer on a LAN. Adding a second one just for PXE breaks addressing for everyone.
- Images not mirrored locally load from the internet, which can take minutes on slow links.

## Related

- [Repair bench server](kb:guides/repair-bench-server)
- [Burn in a server: CPU, memory and stability](kb:linux/burn-in)
- [Recover data from a failing drive](kb:linux/data-recovery)
