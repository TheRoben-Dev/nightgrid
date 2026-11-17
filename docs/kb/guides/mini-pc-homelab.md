# Mini PC homelab

Turn one or two small, quiet, low-power mini PCs (Intel N100/N150, older 8th–10th gen business minis like the Lenovo Tiny, Dell Micro or HP Mini, or a Ryzen mini) into a homelab that idles at 6–15 W, runs dozens of containers and is managed entirely from Nightgrid.

## What you end up with

- A mini PC chosen for the job, with RAM, SSD and BIOS set up right
- Debian or Proxmox installed headless, with a static address
- A starter set of services, power-saving settings and monitoring in Nightgrid

## Before you start

- Decide what it will run. Containers for home services, a media server with hardware transcoding, Home Assistant and a few VMs all fit; a big ZFS pool with many drives does not (there's nowhere to put the disks).
- A USB stick for the installer, a keyboard and screen for the first boot only.

## Part 1 · Pick the hardware

- [ ] **Intel N100/N150 boxes**: 6–10 W idle, Quick Sync for Plex/Jellyfin transcoding (including AV1 decode), usually one SO-DIMM slot. Officially 16 GB max, but most run 32 GB fine.
- [ ] **Used business minis** (Lenovo M720q/M920q, Dell OptiPlex Micro, HP EliteDesk Mini): cheap, very reliable, two RAM slots (up to 64 GB), often an M.2 slot plus a 2.5" bay, sometimes a PCIe slot for a 10 GbE or 2.5 GbE card.
- [ ] **Ryzen minis**: more cores for VMs, but transcoding support in Plex/Jellyfin is weaker than Intel Quick Sync.
- [ ] Prefer Intel i226-V or Realtek RTL8125 network chips (both in-kernel). Two NICs make a mini PC a router or firewall candidate.
- [ ] Storage: an NVMe SSD with DRAM or HMB from a known brand for the system. Put bulk media on a NAS over NFS or SMB rather than USB disks, which drop out under load.

## Part 2 · BIOS settings

- [ ] **Restore on AC power loss: Power On**, so it comes back after an outage.
- [ ] **Wake on LAN: enabled**, for Nightgrid's **WAKE** button.
- [ ] Enable virtualisation (VT-x/AMD-V, and VT-d/IOMMU for passthrough), disable Secure Boot only if your distro or drivers need it.
- [ ] Set C-states and ASPM to **Auto/Enabled** for low idle power; disable devices you won't use (serial, audio, Wi-Fi if wired).
- [ ] Update the BIOS first; early N100 firmwares had power and USB bugs.

## Part 3 · Install

- [ ] **Debian 12/13 (or Ubuntu Server)** if it's a Docker box: minimal install with only SSH server ticked. Then follow [Prepare a Linux Docker host](kb:apps/docker-host).
- [ ] **Proxmox VE** if you want VMs and LXC containers, or plan a second node later. See [LXC containers on Proxmox](kb:apps/proxmox-lxc).
- [ ] Give it a fixed address (DHCP reservation on the router is simplest, or [Give a server a static IP](kb:linux/static-ip)).

## Part 4 · Bring it into Nightgrid

- [ ] **+** in the sidebar, add the host (leave **Client, not a server** unticked), and connect once to accept the fingerprint. See [Add your first host](kb:howto/first-host).
- [ ] Put your key on it from **Settings → Connections → SSH keys** so password logins can be turned off.
- [ ] In the host editor, fill in the **MAC address** (the [Network](view:network) map shows it) and a **Wake from** host, so **WAKE** works.
- [ ] Proxmox? Once the node is a host, the [Proxmox](view:proxmox) view manages its VMs and containers. See [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox).

## Part 5 · Starter services

- [ ] [Docker](view:docker) → **+ New stack** for each: a dashboard ([Homepage dashboard](kb:apps/homepage)), DNS filtering ([Pi-hole](kb:apps/pihole)), a password manager ([Vaultwarden password manager](kb:apps/vaultwarden)) and a media server ([Jellyfin media server](kb:apps/jellyfin)) cover most homes.
- [ ] For hardware transcoding pass the iGPU into the media container (`devices: ["/dev/dri:/dev/dri"]`) and enable QSV/VA-API in its settings.
- [ ] Add each web UI to [Services](view:services) so outages alert.

## Part 6 · Keep it cool, quiet and frugal

- [ ] Follow [Cut a home server's power draw](kb:linux/power-saving): `powertop --auto-tune` as a service, ASPM, and spin-down for any HDD.
- [ ] Clean the fan and replace the thermal paste on used minis; many were sitting in dusty offices for years.
- [ ] Check temperatures with `sensors` under load; [Read fans and sensors](kb:linux/sensors-fans) covers fan curves on the boards that allow it.
- [ ] [Fleet](view:fleet) → **Disks** shows the NVMe's SMART wear level; consumer SSDs under heavy database writes wear faster.

#### Check

- [ ] Pull the power, plug it back in: the mini PC boots by itself, the stacks come back, and Nightgrid shows it online with all services green within a few minutes.

## When Nightgrid is closed

- Everything here runs on the mini PC itself. Power schedules are systemd timers on the host; only live charts and alerts need Nightgrid open.

## Limits and gotchas

- Single-channel RAM on N100 boxes limits iGPU and memory-heavy workloads; it's fine for containers.
- External USB disks are the most common cause of "the server froze". Use a NAS or a mini with a SATA bay for anything important.
- Power bricks fail more than the PCs themselves; keep a spare of the right voltage and plug type.
- One mini PC is one point of failure. Back up to a second machine, and see [Backups](kb:basics/backups).

## Related

- [First homelab weekend](kb:guides/starter-homelab)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [Upgrade the homelab to 10 GbE](kb:guides/10gbe-upgrade)
