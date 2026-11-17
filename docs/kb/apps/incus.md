# Incus containers and VMs

System containers (a whole distro in a container, like LXC on Proxmox) and VMs on any Linux host, driven by one command. A lightweight alternative to a full hypervisor install, and a good fit for a lab on a single box.

![Instances on a bridge, with LAN addresses.](fig:app-bridge)

## What you end up with

- `incus launch images:debian/13 web1` gives you a running Debian container in seconds
- VMs with the same commands (`--vm`)
- Snapshots, profiles and a bridge network

## Before you start

- Debian 13, Ubuntu 24.04 or newer, or Fedora (Incus is packaged on all three)
- Virtualization enabled in the BIOS for VMs

## Part 1 · Install

```sh
sudo apt install incus qemu-system
sudo adduser $USER incus-admin
newgrp incus-admin
incus admin init --minimal
```

- [ ] `--minimal` creates a directory storage pool and a NAT bridge `incusbr0`. Run plain `incus admin init` instead to choose ZFS or btrfs storage.

## Part 2 · Containers and VMs

```sh
incus launch images:debian/13 web1
incus launch images:ubuntu/24.04 vm1 --vm -c limits.cpu=2 -c limits.memory=4GiB
incus list
incus exec web1 -- bash
```

- [ ] `incus snapshot create web1 before-upgrade` and `incus snapshot restore web1 before-upgrade`.
- [ ] Put a container on the LAN instead of NAT with a bridged profile, or `incus config device add web1 eth0 nic nictype=macvlan parent=eno1`.

## Part 3 · Reach them from Nightgrid

- [ ] The `images:` containers ship without an SSH server. Install one and add your public key (copy the `.pub` file to the Incus host first):

```sh
incus exec web1 -- apt install -y openssh-server
incus file push -p --mode 600 mykey.pub web1/root/.ssh/authorized_keys
```

- [ ] Containers on `incusbr0` are only reachable through the Incus host: add them as hosts with the Incus host as **jump host** in the host editor.

#### Check

- [ ] `incus list` shows both running with addresses, and Nightgrid connects to `web1` through the jump host.

## In Nightgrid

- Nightgrid has no Incus view; manage instances with `incus` in a [Terminal](view:terminal) tab, and save common commands as snippets in [Automation](view:automation).
- Each instance you add as a host gets the normal monitoring, [Docker](view:docker) and [System](view:system) views.

## Limits and gotchas

- Incus is the community fork of LXD; `lxc` command examples online translate to `incus` almost one to one.
- macvlan instances can't talk to their own host over the LAN; use a bridge if they need to.

## Related

- [LXC containers on Proxmox](kb:apps/proxmox-lxc)
- [Virtual machines with KVM and libvirt](kb:apps/libvirt-kvm)
- [Import hosts and use jump hosts](kb:howto/jump-hosts)
