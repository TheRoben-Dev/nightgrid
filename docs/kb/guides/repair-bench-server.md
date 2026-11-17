# Repair bench server

![Network boot, drive tests and job tracking for the bench.](fig:guide-repair-bench-server)

A shop server for a repair and sales counter: network boot for diagnostics and installers, a library of ISOs and drivers, a Windows test VM, a short-lived staging area for customer data, and runbooks for the repairs you do every week. Nightgrid is the bench console, and the server keeps serving PXE and shares with it closed.

## What you end up with

- netboot.xyz on the bench network: boot any bench machine into memtest, installers or live tools without USB sticks
- An ISO and driver library on the node, filled with **Download ISO…**
- A Windows test VM you drive from [Desktops](view:desktops), reverted to a clean snapshot after each job
- An encrypted staging share for customer data, emptied automatically after a fixed number of days and never copied off-site
- SMART and drive health checks for drives under test
- Runbooks in [Knowledge](view:notes) and command snippets in [Automation](view:automation) for routine jobs

## Before you start

- Hardware: any small PC with two NICs is ideal (one for the shop LAN, one for an isolated bench network). Add a hot-swap bay or a USB-SATA dock on the node if you test customer drives there.
- Keep the bench network separate from the shop's office and payment network. Customer machines can be infected.
- Settings → **FEATURES**: turn on **Proxmox** and **VM screens** (the **Standard** preset leaves both out).
- Know your data-handling obligations (for example GDPR in the EU): how long you may keep customer data, and that you must be able to show it was deleted.

## Part 1 · Bench network

- [ ] [Proxmox](view:proxmox) → **NETWORK**: add a bridge `vmbr1` on the second NIC, with no gateway. Changes are staged and shown as a diff. **Apply…** and press **Keep** within 90 seconds.
- [ ] Guests that serve the bench (netboot.xyz, the staging share) get a NIC on `vmbr1`.
- [ ] The DHCP server on the bench network must point PXE clients at netboot.xyz (DHCP options `next-server` and the boot file name). A small dnsmasq in the netboot container can do both if the bench network has no other DHCP server.

#### Check

- [ ] `vmbr1` shows in **NETWORK** with the second NIC as its port.

## Part 2 · netboot.xyz

- [ ] **+ Container** (Debian 12, unprivileged, nesting on), NIC on `vmbr1`, and add it as a host.
- [ ] In [Docker](view:docker), **+ New stack** from the `netbootxyz/netbootxyz` image. It serves TFTP on 69/udp, its web UI on 3000 and assets on 8080.
- [ ] Bind the web UI to localhost (`127.0.0.1:3000:3000`) and open it through a saved tunnel in [Tunnels](view:tunnels).

Example stack. Paste this into the editor (Docker → Stacks); don't send it to a terminal.

```yaml
services:
  netbootxyz:
    image: ghcr.io/netbootxyz/netbootxyz:latest
    restart: unless-stopped
    ports:
      - "69:69/udp"
      - "8080:80"
      - "127.0.0.1:3000:3000"
    volumes:
      - ./config:/config
      - ./assets:/assets
```

- [ ] Pin the image tag once it works.

#### Check

- [ ] A bench PC set to network boot shows the netboot.xyz menu.

## Part 3 · ISO and driver library

- [ ] [Proxmox](view:proxmox) → **STORAGE** → **Download ISO…**: paste the vendor's URL and its SHA-256, and the node fetches and verifies it. Use this for Windows, Linux installers, GParted, Clonezilla and memtest86+.
- [ ] **Get CT template…** for container templates.
- [ ] Drivers and tools that aren't ISOs: a folder on the node (for example `/srv/library`), filled by dragging files in [Files](view:files). Share it read-only to the bench network with Samba or NFS from a container.
- [ ] Keep a runbook note listing where each tool came from and its version, so you know when to refresh it.

#### Check

- [ ] The ISOs show under the node's storage with the expected sizes.

## Part 4 · Windows test VM

- [ ] **+ VM** with a Windows ISO and the VirtIO driver ISO attached. Open **Screen** on it to install through [Desktops](view:desktops). **Ctrl+Alt+Del** and **Type text…** handle the installer's keys.
- [ ] After install and updates, shut it down and take a snapshot called `clean`.
- [ ] Use it to test drivers, open suspicious attachments in isolation, or check that a customer's software works. Put its NIC on `vmbr1` only, never the office network.
- [ ] After each job: **Roll back…** to `clean`. Everything since the snapshot is lost, which is the point.
- [ ] For a smoother desktop, enable RDP in the guest and use **RDP**. Nightgrid tunnels it through the node.

#### Check

- [ ] Rolling back to `clean` returns a fresh Windows in seconds.

## Part 5 · Customer data staging

Customer data is the most sensitive thing on this box. Keep it encrypted, short-lived and local.

- [ ] Create an encrypted dataset on the node (it asks for a passphrase, which you then need after every reboot):

```sh
zfs create -o encryption=on -o keyformat=passphrase -o keylocation=prompt tank/customers
```

- [ ] One folder per job, named by ticket number, never by customer name.
- [ ] Share it on the bench network only, from a container with a bind mount. Never add it to restic, Syncthing or any off-site backup.
- [ ] Automatic purge: [System](view:system) → Schedules → **+ Timer** on the node, daily, with this command (14 days here; use your own policy):

```sh
find /tank/customers -mindepth 1 -maxdepth 1 -type d -mtime +14 -exec rm -rf {} +
```

- [ ] **Run now** and **Journal** on the timer's row let you test it and see what was removed.
- [ ] After a large job, also destroy old snapshots of the dataset so deleted data really is gone.

#### Check

- [ ] A folder older than the limit disappears after **Run now**, and the journal shows the timer ran.

## Part 6 · Drive health on the bench

- [ ] For drives in the node's dock, [System](view:system) → **disks** shows SMART health. Read it once with **remember** ticked so [Fleet](view:fleet) → **Disks** can include it.
- [ ] For a full read of one drive in a terminal (replace `sdX` and check twice that it's the customer drive):

```sh
smartctl -a /dev/sdX
```

```sh
smartctl -t long /dev/sdX
```

- [ ] Save the result in the job's runbook note, so you can show the customer.

#### Check

- [ ] The drive's SMART summary appears under **disks**.

## Part 7 · Runbooks and snippets

- [ ] [Knowledge](view:notes): one note per routine job (data transfer, Windows reinstall, malware cleanup, laptop handover checklist). Use `- [ ]` steps. Each command in a code block gets **Send to terminal**.
- [ ] [Automation](view:automation) → **Snippets** for commands with variables, for example a SMART check with `{{device}}`, or a folder purge with `{{ticket}}`.
- [ ] **Playbooks** for multi-step routines, such as preparing a new staging folder and the Windows VM for a job.
- [ ] Tag notes by job type, and pin the ones you use daily so they show on the Overview's NOTES widget.

#### Check

- [ ] Searching Knowledge for a job type finds its runbook.

## When Nightgrid is closed

- Keeps running: netboot.xyz, the shares, the Windows VM, the purge timer, the encrypted dataset (once unlocked).
- Needs Nightgrid open (or in the tray): alerts, monitoring history, SMART checks in Fleet, Desktops screens.

## Limits and gotchas

- After a reboot, the encrypted dataset stays locked until you run `zfs load-key tank/customers` and mount it. That's intended: a stolen box doesn't give away data.
- `rm -rf` in a timer is irreversible. Test the `find` command without `-exec` first, so it only lists what it would delete.
- Don't connect unknown customer drives to the node itself if you can avoid it. A separate bench PC is safer than the server.
- Nightgrid doesn't create Samba or NFS shares. Configure those in a container with the Files editor.
- netboot.xyz fetches boot images from the internet unless you mirror them in its assets folder.

## Related

- Proxmox media server
- Game server host
- Family cloud and photos
- Local AI GPU server
- Smart home hub
- Remote family IT
- Dev and git deploy lab
