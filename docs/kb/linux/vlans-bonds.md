# VLANs and bonded NICs on a server

Put a server on several VLANs over one cable, or join two NICs for redundancy or more bandwidth. Examples use netplan (Ubuntu) and NetworkManager's `nmcli` (Fedora, Debian desktops); Proxmox has its own section.

## What you end up with

- A VLAN interface with its own address on a trunked switch port
- A bond of two NICs (active-backup or LACP)
- A rollback plan so a typo doesn't lock you out

## Before you start

- Console access (IPMI, Proxmox console or a monitor) in case the network change fails
- The switch port set as a trunk (tagged VLANs) or a LACP group, matching what you configure

## Part 1 · A VLAN with netplan

- [ ] In `/etc/netplan/50-vlans.yaml` (mode 600):

```yaml
network:
  version: 2
  ethernets:
    eno1:
      dhcp4: true
  vlans:
    vlan20:
      id: 20
      link: eno1
      addresses: [10.20.0.10/24]
```

- [ ] `sudo netplan try` applies it and rolls back after 120 seconds unless you confirm: use it every time.

## Part 2 · A VLAN with nmcli

```sh
sudo nmcli con add type vlan con-name vlan20 dev eno1 id 20 ipv4.method manual ipv4.addresses 10.20.0.10/24
sudo nmcli con up vlan20
```

## Part 3 · Bonding

- [ ] netplan, active-backup (works with any switch):

```yaml
network:
  version: 2
  ethernets:
    eno1: {}
    eno2: {}
  bonds:
    bond0:
      interfaces: [eno1, eno2]
      parameters:
        mode: active-backup
        primary: eno1
      dhcp4: true
```

- [ ] `mode: 802.3ad` (LACP) gives more total bandwidth across several clients, but needs a LACP group on the switch. One single transfer still uses one link.

#### Check

- [ ] `ip -d link show vlan20` shows `vlan id 20`; `cat /proc/net/bonding/bond0` shows both slaves up. Unplug one cable: pings keep going.

## In Nightgrid

- On Proxmox nodes, use [Proxmox](view:proxmox) → **NETWORK**: VLAN-aware bridges and bonds are staged as a diff and applied with an automatic rollback if you lose the connection.
- After the change, the [Network](view:network) map scanned from this host shows the VLAN's devices; **routed ranges** cover VLANs reached through a router.

## Limits and gotchas

- Netplan files must not be world-readable, or it warns on every apply.
- A server on several VLANs can bypass your firewall between them; only give it the VLANs it needs, and don't route between them on the server.

## Related

- [Give a server a static IP](kb:linux/static-ip)
- [OPNsense firewall in a VM](kb:apps/opnsense)
- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
