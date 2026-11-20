# OPNsense firewall in a VM

Run your router and firewall as a VM: OPNsense gives you VLANs, a stateful firewall, DHCP, DNS (Unbound), WireGuard and traffic graphs. Virtualising it on a Proxmox host with two NICs saves a box, as long as you plan for the host being down.

![OPNsense between the internet and your VLANs.](fig:app-opnsense)

## What you end up with

- A WAN NIC passed to OPNsense, a LAN bridge for everything else
- VLANs for IoT, guests and servers, with rules between them
- Config backups you can restore onto new hardware in minutes

## Before you start

- A host with two NICs (Intel NICs are the least trouble with FreeBSD), running Proxmox
- The ISP modem in bridge mode, or accept double NAT for now
- A way back in when you lock yourself out: the Proxmox console

## Part 1 · The VM

- [ ] Proxmox networking: `vmbr0` on the LAN NIC (Proxmox's own address lives here), `vmbr1` on the WAN NIC with no IP.
- [ ] Create a VM from the OPNsense installer ISO: 2 vCPU, 4 GB RAM, 20 GB disk, NICs on `vmbr1` (WAN) and `vmbr0` (LAN), both **VirtIO**, with the firewall checkbox off on both NICs.
- [ ] Install, then from the console assign interfaces (vtnet0/vtnet1) and set the LAN address.
- [ ] Start at boot: on, with startup order 1 so it comes up before other guests.

## Part 2 · Basics in the web UI

- [ ] Run the wizard: hostname, DNS, WAN type (DHCP for most ISPs), LAN subnet, a new root password.
- [ ] System → Firmware → Updates, then reboot.
- [ ] VLANs: make `vmbr0` VLAN-aware in Proxmox, then Interfaces → Other Types → VLAN in OPNsense. Assign each, give it a subnet and DHCP.
- [ ] Firewall → Rules per VLAN: IoT can reach the internet and the MQTT broker, nothing else.

## Part 3 · Backups

- [ ] System → Configuration → Backups → Download configuration. Keep a copy off the host; it's the whole router in one XML file.

#### Check

- [ ] A client on each VLAN gets an address and internet, and IoT can't open the Proxmox web UI.

## In Nightgrid

- [Proxmox](view:proxmox) shows the VM; **NETWORK** stages bridge changes with an automatic rollback, which is exactly what you want when changing the bridge your router sits on.
- [Network](view:network) → scans from a host on each VLAN show what's there; **Watch the network** alerts on new devices.
- The firewall itself isn't a Nightgrid host (FreeBSD, no Docker); add its web UI to [Services](view:services).
- [Services](view:services) → **Add integration** → **OPNsense** with an API key and secret (System → Access → Users → API keys): gateways with delay and loss, firmware updates, and stopped services you can start from the card (see [Add app cards](kb:howto/integrations)).

## Limits and gotchas

- When the Proxmox host reboots, the house has no internet. Keep a spare small router or a mini PC ready.
- Don't give the Proxmox host an address on the WAN bridge.
- Hardware offloading settings (Interfaces → Settings) should stay disabled for VirtIO NICs.

## Related

- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Map your network and find exposed ports](kb:howto/network-map)
- [WireGuard VPN server](kb:apps/wireguard)
