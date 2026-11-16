# Proxmox VM templates with cloud-init

Make a Debian or Ubuntu cloud image into a Proxmox template, then clone new VMs in seconds with your SSH key, user and network already set.

## What you end up with

- A template VM built from an official cloud image
- New VMs from **Clone…** in Nightgrid that boot ready to log in

## Part 1 · Build the template

- [ ] On the node, in a [Terminal](view:terminal) (Debian 12 shown; Ubuntu publishes similar `cloudimg` files):

```sh
cd /var/lib/vz/template/iso
wget https://cloud.debian.org/images/cloud/bookworm/latest/debian-12-genericcloud-amd64.qcow2
qm create 9000 --name debian12-tpl --memory 2048 --cores 2 --net0 virtio,bridge=vmbr0 --scsihw virtio-scsi-single
qm importdisk 9000 debian-12-genericcloud-amd64.qcow2 local-lvm
qm set 9000 --scsi0 local-lvm:vm-9000-disk-0,discard=on --boot order=scsi0 --ide2 local-lvm:cloudinit --serial0 socket --vga serial0 --agent enabled=1
qm set 9000 --ciuser admin --sshkeys ~/.ssh/authorized_keys --ipconfig0 ip=dhcp
```

- [ ] Change `local-lvm` to your storage.
- [ ] [Proxmox](view:proxmox) → GUESTS → **⋯** → **Convert to template…**.

## Part 2 · Clone

- [ ] **⋯** on the template → **Clone…** (full clone for independence, linked for speed and space).
- [ ] In the clone's **HARDWARE**, **Grow…** the disk to the size you need; the guest grows its filesystem on first boot.
- [ ] Start it; add it as a host in Nightgrid once it has an address (the QEMU agent reports it, so install `qemu-guest-agent` in the template image or via cloud-init).

#### Check

- [ ] A new clone accepts SSH with your key a minute after starting.

## Limits and gotchas

- Cloud-init settings (user, keys, IP) are set per VM with `qm set <id> --ciuser/--sshkeys/--ipconfig0`; Nightgrid's HARDWARE tab doesn't edit cloud-init fields yet.
- The generic cloud image has no console password; serial console works only after you set one via cloud-init.

## Related

- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Ansible for your homelab](kb:apps/ansible)
