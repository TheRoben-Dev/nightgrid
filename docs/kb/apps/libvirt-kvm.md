# Virtual machines with KVM and libvirt

Run VMs on a plain Debian, Ubuntu or Fedora server without Proxmox, using KVM and libvirt, and manage them from Nightgrid's System → VMs and Desktops.

## What you end up with

- libvirt with a bridged network so VMs get LAN addresses
- VMs created with `virt-install`, autostarting with the host
- Start, stop, console and graphical screen from Nightgrid

## Before you start

- A CPU with virtualization enabled in the BIOS: `grep -cE 'vmx|svm' /proc/cpuinfo` should be above 0.

## Part 1 · Install

- [ ] Debian/Ubuntu:

```sh
sudo apt install -y qemu-system-x86 libvirt-daemon-system virtinst ovmf
sudo usermod -aG libvirt $USER
```

- [ ] Log out and back in, then `virsh list --all` works without sudo.

## Part 2 · A bridge for LAN addresses

- [ ] With NetworkManager (replace `eth0` with your NIC):

```sh
sudo nmcli con add type bridge ifname br0 con-name br0
sudo nmcli con add type bridge-slave ifname eth0 master br0
sudo nmcli con up br0
```

- [ ] Do this from a console or with a rollback plan: the host's address moves to `br0` and SSH drops for a moment.

## Part 3 · Create a VM

- [ ] Download an installer ISO to `/var/lib/libvirt/images`, then:

```sh
virt-install --name debian1 --memory 2048 --vcpus 2 --disk size=20 --cdrom /var/lib/libvirt/images/debian.iso --os-variant debian12 --network bridge=br0 --graphics vnc,listen=127.0.0.1 --boot uefi
```

- [ ] `virsh autostart debian1` so it starts with the host.

## Part 4 · In Nightgrid

- [ ] [System](view:system) → **VMs** lists guests with **Start**, **Shut down**, **Reboot**, **Force stop**, **Console** (serial) and **Screen**.
- [ ] **Screen** opens the VNC display in [Desktops](view:desktops) over SSH; no port is opened.
- [ ] Install `qemu-guest-agent` in each guest for clean shutdowns and addresses (used by **RDP** for Windows guests).

#### Check

- [ ] The installer shows in Desktops, and after install the VM answers on a LAN address.

## Limits and gotchas

- Snapshots of UEFI VMs with `virsh snapshot-create` need external snapshots on recent libvirt; or snapshot the storage (ZFS) instead.
- For many VMs, clustering and backups, Proxmox is less work (see [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)).

## Related

- [Open VM screens and RDP](kb:howto/vm-desktops)
- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
