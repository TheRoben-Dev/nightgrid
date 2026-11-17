# iSCSI block storage from a NAS or server

Share a disk over the network as a raw block device instead of files: the client formats it and sees a local disk. Good for VM disks on Proxmox, a Steam library on a NAS, or a Windows PC that wants "a local drive" stored on the server. Linux serves it with the kernel's LIO target, managed by `targetcli`.

## What you end up with

- An iSCSI target on a Linux server backed by a file, LVM volume or ZFS zvol
- Access limited to named initiators with CHAP
- A Linux or Windows client that mounts it at boot

## Before you start

- A block device or space for one: an LVM logical volume (`lvcreate -L 200G -n steam vg0`) or a ZFS zvol (`zfs create -V 200G tank/steam`) performs better than a file.
- **One client per LUN.** iSCSI doesn't share a filesystem between machines; two clients mounting the same ext4 or NTFS volume will corrupt it. Use NFS or SMB for shared files.

## Part 1 · The target

- [ ] Install and open the shell:

```bash
sudo apt install targetcli-fb
sudo targetcli
```

- [ ] Inside `targetcli`:

```text
/backstores/block create steam /dev/zvol/tank/steam
/iscsi create iqn.2024-01.lan.server:steam
/iscsi/iqn.2024-01.lan.server:steam/tpg1/luns create /backstores/block/steam
/iscsi/iqn.2024-01.lan.server:steam/tpg1/acls create iqn.1991-05.com.microsoft:gaming-pc
/iscsi/iqn.2024-01.lan.server:steam/tpg1/acls/iqn.1991-05.com.microsoft:gaming-pc set auth userid=gamer password=LongChapSecret12
saveconfig
exit
```

- [ ] The ACL name is the client's initiator name: on Windows in **iSCSI Initiator → Configuration**, on Linux in `/etc/iscsi/initiatorname.iscsi`. CHAP passwords must be 12–16 characters for Windows.
- [ ] `sudo systemctl enable --now rtslib-fb-targetctl` (the unit is `target` on Fedora) restores the config at boot. Allow TCP 3260 from the client only.

## Part 2 · Linux client

```bash
sudo apt install open-iscsi
sudo iscsiadm -m discovery -t sendtargets -p 192.0.2.10
sudo iscsiadm -m node -T iqn.2024-01.lan.server:steam -o update -n node.session.auth.authmethod -v CHAP
sudo iscsiadm -m node -T iqn.2024-01.lan.server:steam -o update -n node.session.auth.username -v gamer
sudo iscsiadm -m node -T iqn.2024-01.lan.server:steam -o update -n node.session.auth.password -v LongChapSecret12
sudo iscsiadm -m node -T iqn.2024-01.lan.server:steam --login
sudo iscsiadm -m node -T iqn.2024-01.lan.server:steam -o update -n node.startup -v automatic
```

- [ ] A new disk appears (`lsblk`). Format it once, and mount it in `/etc/fstab` with `_netdev,nofail` so boot doesn't hang without the network.

## Part 3 · Windows client

- [ ] **iSCSI Initiator** → Discovery → **Discover Portal** → server address → Targets → **Connect** → Advanced → enable CHAP with the user and secret → tick **Add this connection to the list of Favorite Targets**.
- [ ] **Disk Management** shows the new disk: initialise, format NTFS, give it a letter.

#### Check

- [ ] The disk is still there after the client reboots, and a large file copy runs near the network's line speed.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** watches the ZFS pool a zvol LUN lives on, with SMART for the disks under it.
- [Fleet](view:fleet) → **Speed** measures the link first; iSCSI can't be faster than it.
- Proxmox can use the target directly as storage (Datacenter → Storage → iSCSI, or LVM on top) for VM disks.

## Limits and gotchas

- Snapshots and backups of a LUN happen on the server (ZFS snapshot of the zvol) and only capture a consistent filesystem if the client was idle or shut down.
- A thin zvol or file can overcommit the pool; when it fills, the client sees I/O errors, not "disk full".
- iSCSI traffic is unencrypted; keep it on a trusted LAN or a storage VLAN.

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [LVM: grow and snapshot volumes](kb:linux/lvm)
- [NFS shares](kb:apps/nfs)
