# Install Proxmox VE on a new server

From a blank machine to a Proxmox node that's updated, reachable with your SSH key and managed from Nightgrid's Proxmox view. Covers the installer choices that are hard to change later: disks, filesystem and network.

## What you end up with

- Proxmox VE installed on ZFS (mirror) or ext4/LVM-thin
- The free **no-subscription** update repository instead of the enterprise one
- Root SSH key login and the node in Nightgrid

## Before you start

- **BIOS/UEFI**: enable virtualisation (VT-x/AMD-V) and IOMMU (VT-d/AMD-Vi) if you'll pass devices through. Set "restore on AC power loss" to on.
- **Disks**: two SSDs for a ZFS mirror is the comfortable choice for the system and VM disks. One SSD works with ext4 + LVM-thin. Keep bulk HDDs for later; don't select them in the installer.
- A fixed IP address for the node, its gateway and DNS server, and a name (`pve1.home.lan`).

## Part 1 · Installer USB

- [ ] Download the Proxmox VE ISO from the official site and check its SHA256.
- [ ] Write it to a USB stick with Ventoy, balenaEtcher, Rufus (DD mode) or `dd`:

```bash
sudo dd if=proxmox-ve_*.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

## Part 2 · The installer

- [ ] **Target harddisk → Options**: choose **ZFS (RAID1)** and tick both SSDs, or **ext4** on one disk. On ZFS, `ashift=12` is right for almost all SSDs and HDDs.
- [ ] Country, time zone, keyboard. Root password and an email for system alerts.
- [ ] **Management network**: the NIC that's cabled, the FQDN (`pve1.home.lan`), static IP in CIDR form (`192.168.1.10/24`), gateway and DNS. This becomes the bridge `vmbr0` that VMs use.
- [ ] Install, remove the USB stick, reboot.

## Part 3 · First steps

- [ ] Open `https://192.168.1.10:8006`, log in as `root` with realm **Linux PAM**.
- [ ] Node → **Updates → Repositories**: disable the **enterprise** entries (pve and ceph) and **Add** the **No-Subscription** repository. Without a subscription the enterprise repos fail on update.
- [ ] **Updates → Refresh**, then **Upgrade** (or in a shell: `apt update && apt full-upgrade`). Reboot if a new kernel was installed.
- [ ] The "No valid subscription" dialog at login is expected; it doesn't limit anything.

## Part 4 · Into Nightgrid

- [ ] **+** in the sidebar: address `192.168.1.10`, user `root`, port 22. Connect once and compare the fingerprint with `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` on the node's console.
- [ ] **Settings → Connections → SSH keys → CHECK HOSTS**, then **+** to put your key on it. Proxmox root logins with a key are normal; turn off root password login later in `sshd_config` if you like.
- [ ] Open the [Proxmox](view:proxmox) view (Mod+Shift+P): the node, storage and guests appear.

#### Check

- [ ] **Updates** shows no pending packages, and Nightgrid's [Proxmox](view:proxmox) view lists the node with its storage.

## When Nightgrid is closed

- The node runs on its own; Nightgrid only talks to it over SSH when you open it.

## Limits and gotchas

- Changing the node's hostname or IP after VMs exist is possible but fiddly (`/etc/hosts`, `/etc/network/interfaces`, cluster config). Pick them well now.
- Consumer SSDs wear faster under ZFS with many VMs; watch the wear level in [Fleet](view:fleet) → **Disks**.
- A cluster needs at least three votes for quorum; two nodes need a QDevice. Stay single-node until you need more.

## Related

- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Proxmox VM templates with cloud-init](kb:apps/cloud-init-templates)
- [Proxmox Backup Server](kb:apps/proxmox-backup-server)
