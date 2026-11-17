# Smart home hub

Run Home Assistant OS as a Proxmox VM with your Zigbee, Z-Wave or Thread dongle passed through, and manage it from Nightgrid. The VM starts by itself after a power cut, and you take a snapshot before every Home Assistant update, so a bad update costs you one rollback instead of an evening. A Docker-only variant (Home Assistant Container, Mosquitto, Zigbee2MQTT) is at the end.

## What you end up with

- Home Assistant OS in its own VM, with the radio dongle passed through by vendor:product id so it survives a replug into a different port.
- The VM starts at boot, first in the startup order, before anything that talks to it.
- A snapshot before every update and a nightly vzdump backup, on top of Home Assistant's own backups.
- A service check on Home Assistant, with a dependency link so a dead node raises one alert, not three.
- The UPS watched from Fleet, plus a shutdown that runs on the server itself.

## Before you start

- A Proxmox VE 8.x node with 2 cores and 4 GB of RAM to spare and about 32 GB of disk for the VM.
- A USB radio: a Sonoff, SkyConnect/ZBT-1, ConBee or Z-Wave stick. Plug it into the node, ideally on a short USB 2.0 extension cable away from USB 3 ports, which interfere with 2.4 GHz.
- The node added as a host in Nightgrid, logging in as root or with sudo.
- Settings → **FEATURES**: the **Standard** preset leaves out Proxmox, VM screens and UPS. Pick **Everything**, or turn those three on yourself.

## Part 1 · Import the Home Assistant OS disk

Nightgrid's **+ VM** builds a VM around an ISO installer. It can't import a ready-made disk image yet, so this part is a single `qm create` in a terminal on the node.

- [ ] Open a terminal on the node from [Terminal](view:terminal).
- [ ] On Home Assistant's release page, copy the address of the `haos_ova-<version>.qcow2.xz` file (the KVM image). Download it on the node:

```sh
wget -P /root '<haos-qcow2-xz-url>'
```

- [ ] Unpack it:

```sh
xz -d /root/haos_ova-<version>.qcow2.xz
```

- [ ] Pick a free VM id (**GUESTS** in [Proxmox](view:proxmox) shows the ids in use) and create the VM in one command. It uses UEFI with Secure Boot keys **not** enrolled (Home Assistant OS won't boot with them) and imports the disk as `scsi0`:

```sh
qm create <vmid> --name haos --machine q35 --bios ovmf --efidisk0 local-lvm:1,efitype=4m,pre-enrolled-keys=0 --cores 2 --memory 4096 --net0 virtio,bridge=vmbr0 --scsihw virtio-scsi-single --scsi0 local-lvm:0,import-from=/root/haos_ova-<version>.qcow2,discard=on,ssd=1 --boot order=scsi0 --ostype l26 --agent enabled=1
```

- [ ] Use your own storage if it isn't `local-lvm` (**STORAGE** lists what you have). Once it's imported, delete the image:

```sh
rm /root/haos_ova-<version>.qcow2
```

- [ ] In [Proxmox](view:proxmox) → **GUESTS**, click `haos` → **HARDWARE** → disk → **Grow…** to 32 GB. The image comes at about 6 GB and fills up quickly once add-ons and the recorder database grow.

#### Check

- [ ] **Start** the VM, then **Screen**. The Home Assistant banner shows its address. Open `http://<ha-ip>:8123` in your browser and finish onboarding.
- [ ] Give the VM a DHCP reservation on your router, so the address in your service check stays put.

## Part 2 · Pass the radio through

- [ ] Find the dongle's vendor:product id on the node:

```sh
lsusb
```

- [ ] Shut the VM down. In **HARDWARE** → USB passthrough, pick the dongle from the node's device list. Choose it by vendor:product id (for example `10c4:ea60` for CP210x-based sticks), not by port. The id follows the stick to any port; a port mapping breaks when someone replugs it.
- [ ] Start the VM. In Home Assistant, ZHA, Z-Wave JS or the Zigbee2MQTT add-on should find the stick under `/dev/serial/by-id/`.

#### Check

- [ ] **HARDWARE** lists the `usb0` entry without "after restart".
- [ ] Inside Home Assistant, Settings → System → Hardware shows the stick.

If two sticks share a vendor:product id (two identical CP210x boards), map one of them by port instead.

## Part 3 · Start order and recovery after a power cut

- [ ] In the VM's **HARDWARE** → boot options, turn on start at boot and give it startup order 1 with an up delay of about 60 s, so things that depend on it start after Home Assistant is listening. The same from a terminal:

```sh
qm set <vmid> --onboot 1 --startup order=1,up=60
```

- [ ] Give later guests (Frigate, an MQTT broker CT, the media server) order 2 and up.
- [ ] Install the QEMU guest agent. Home Assistant OS ships it, and `--agent enabled=1` above turns it on. With it, shutdowns are clean and the guest's IP shows up.

#### Check

- [ ] Reboot the node from System → **REBOOT…**. Home Assistant should be reachable again without you touching it.

## Part 4 · Snapshots, backups and updates

- [ ] Before every Home Assistant OS or Core update: click `haos` → **SNAPSHOTS** → take one named after the version. Tick **with RAM** only if you want to resume mid-session; a disk-only snapshot is smaller and enough.
- [ ] If the update breaks something: **Roll back…** to the snapshot. Everything since is lost, including history recorded after the snapshot.
- [ ] Delete old snapshots once the new version has run for a few days. Snapshots on LVM-thin and ZFS cost space as the disk changes.
- [ ] **BACKUPS** tab: check that a vzdump job covers `haos`. The tab warns about guests no job covers. Nightgrid lists jobs but can't create them yet; add one from the node:

```sh
pvesh create /cluster/backup --schedule '03:15' --vmid <vmid> --storage <backup-storage> --mode snapshot --prune-backups keep-daily=7,keep-weekly=4 --enabled 1
```

- [ ] Keep Home Assistant's own backups as well (Settings → System → Backups). They restore into a fresh install, which a vzdump can't do.

#### Check

- [ ] **Back up now** once, then confirm the backup shows up with today's date in the guest's **BACKUPS** list.

## Part 5 · Watch it from Nightgrid

- [ ] [Services](view:services) → add a check for `http://<ha-ip>:8123/`. Home Assistant answers the login page with 200.
- [ ] Settings → **Dependencies**: add "Home Assistant depends on `<node>`". Everything on the node itself already depends on the node, but a service check reached over HTTP doesn't know that until you link it.
- [ ] If the UPS is plugged into the node: [Fleet](view:fleet) → **UPS** → **+ UPS**, with `ups@localhost` and `nut-client` installed on the node.
- [ ] Keep an eye on the VM's disk in **SUMMARY** and **STORAGE**. A recorder database that balloons is the usual reason a Home Assistant VM fills up; shorten `recorder: purge_keep_days` in Home Assistant before you **Grow…** again.

## Variant · Docker instead of a VM

Pick this if the node already runs a Docker LXC and you don't need add-ons. You give up the Supervisor and add-on store and get one compose stack in [Docker](view:docker).

- [ ] Pass the stick into the Docker LXC (Proxmox VE 8.2+ supports device entries for containers). `gid=20` is the `dialout` group on Debian:

```sh
pct set <ctid> -dev0 /dev/serial/by-id/<your-stick>,gid=20
```

- [ ] Docker → **+ New stack** → paste YAML. Paste this into the editor; don't send it to a terminal:

```yaml
services:
  homeassistant:
    image: ghcr.io/home-assistant/home-assistant:stable
    network_mode: host
    volumes:
      - ./ha:/config
    restart: unless-stopped
  mosquitto:
    image: eclipse-mosquitto:2
    ports:
      - "1883:1883"
    volumes:
      - ./mosquitto:/mosquitto
    restart: unless-stopped
  zigbee2mqtt:
    image: koenkk/zigbee2mqtt:latest
    ports:
      - "8080:8080"
    devices:
      - /dev/serial/by-id/<your-stick>:/dev/ttyUSB0
    volumes:
      - ./z2m:/app/data
    restart: unless-stopped
    depends_on:
      - mosquitto
```

- [ ] Mosquitto 2 refuses anonymous clients by default. Create `./mosquitto/config/mosquitto.conf` in [Files](view:files) with a listener and a password file before the first **up**.
- [ ] Pin `zigbee2mqtt` to a version instead of `latest` once it works. DRIFT's compose review flags `:latest`.
- [ ] Updates go through Docker → **Updates**, which takes a snapshot first, so **Roll back…** covers a bad image.

## When Nightgrid is closed

- Runs without the app: the VM and its start order, vzdump jobs, Home Assistant's own backups and automations, Docker restart policies, and a **POWER SCHEDULE** (it's a systemd timer on the host).
- Needs Nightgrid running (the tray is enough): service checks, alerts and notifications, UPS monitoring and **UPS shutdown**, uptime history.
- For a shutdown that works with your computer off, run NUT on the node in standalone mode with its own `upsmon` shutdown, and use Nightgrid's UPS view only to watch it.

## Limits and gotchas

- **+ VM** can't import a disk image yet, so the HAOS import is a terminal step.
- Intel GPUs aren't read by monitoring yet. That matters if you add Frigate with OpenVINO on an iGPU.
- Snapshot rollback rewinds the Home Assistant database too. Note the time, so you know which history is gone.
- USB passthrough of a stick that resets during boot (some ConBee firmware) can drop out. A powered hub or the by-port mapping sometimes fixes it.
- Thread/Matter needs IPv6 and multicast on the VM's bridge. VLAN-aware bridges and IGMP snooping on the switch break discovery more often than the dongle does.

## Related

Proxmox media server · Game server host · Family cloud and photos · Repair bench server · Local AI GPU server · Remote family IT · Dev and git deploy lab
