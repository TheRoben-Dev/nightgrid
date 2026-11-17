# Samba file shares

Share a folder with Windows, macOS and Linux machines over SMB, with per-user access and sensible security.

## What you end up with

- A share like `\\server\media` that works from every OS
- Logins per person, separate from their Linux passwords
- SMB only reachable from your LAN

## Before you start

- A folder to share (`/srv/share` or a ZFS dataset).
- A Linux user for each person, or one shared group.

## Part 1 · Install

- [ ] `sudo apt install -y samba`.
- [ ] Make a group and folder:

```sh
sudo groupadd -f smbshare
sudo mkdir -p /srv/share && sudo chgrp smbshare /srv/share && sudo chmod 2775 /srv/share
sudo usermod -aG smbshare <user>
```

## Part 2 · Configure

- [ ] Append to `/etc/samba/smb.conf` (open it with `sudoedit` so it opens in Nightgrid's editor):

```ini
[global]
   server min protocol = SMB2_10
   map to guest = never
   vfs objects = fruit streams_xattr
   fruit:metadata = stream
   fruit:model = MacSamba

[share]
   path = /srv/share
   valid users = @smbshare
   read only = no
   create mask = 0664
   directory mask = 2775
```

- [ ] Check it: `testparm -s`.
- [ ] Give each person an SMB password: `sudo smbpasswd -a <user>`.
- [ ] `sudo systemctl restart smbd`.

## Part 3 · Firewall

- [ ] Allow port 445 from the LAN only. In [System](view:system) → **FIREWALL** → **Add…**: allow, 445, tcp, from `192.168.1.0/24` (your LAN), comment "smb".
- [ ] Never forward 445 to the internet. Use a tailnet for access away from home.

## Part 4 · Connect

- [ ] Windows: `\\server\share` in Explorer. macOS: Finder → Go → Connect to Server, `smb://server/share`. Linux: `smb://server/share` in the file manager, or `mount -t cifs`.
- [ ] Time Machine target for Macs: add `fruit:time machine = yes` to a dedicated share.

#### Check

- [ ] Copy a large file from another machine; [Fleet](view:fleet) → **Speed** tells you what the network can do if it seems slow.

## Limits and gotchas

- Windows caches credentials per server name. If you change a password, remove the old entry in Credential Manager.
- SMB1 is off on purpose. Very old devices (some scanners, TVs) need it; give them a separate, restricted share instead of lowering the global setting.
- File ownership: files created over SMB belong to the person's Linux user. Containers that read the share need matching uid/gid.

## Related

- [NFS shares](kb:apps/nfs)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Find slow links with iperf3](kb:howto/speed-test)
