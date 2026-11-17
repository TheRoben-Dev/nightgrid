# Proxmox media server, run from Nightgrid

A Proxmox VE box that streams, downloads and backs itself up on its own, and that you manage only through Nightgrid. The server never needs the app to stay up. Nightgrid is the front end: everything you change goes through it over SSH, and the Proxmox web UI is shut off from the network.

The split is simple. Anything that has to keep happening (guests starting after a power cut, backups, scrubs, UPS shutdown, the media apps' own schedules) is configured **on the server**. Anything you *do* (create, change, update, restore, inspect) happens **in Nightgrid**.

## What you end up with

- One Proxmox VE 8 node with a ZFS pool for media.
- An unprivileged LXC running Docker with Jellyfin (or Plex), the *arr apps and a download client, using the iGPU for hardware transcoding.
- Guests that start by themselves in the right order after a reboot or power loss.
- Nightly vzdump backups, monthly ZFS scrubs and SMART checks that run without Nightgrid.
- The Proxmox web UI reachable only from the node itself (or through a Nightgrid tunnel), SSH key-only, and the admin UIs of the media apps reachable only through tunnels.
- Remote access through Tailscale, with no ports open to the internet.
- A break-glass path for when SSH or Nightgrid isn't available.

## Before you start

- A machine with an Intel CPU that has Quick Sync (7th gen or newer for HEVC 10-bit; 11th gen or newer for AV1 decode) or an AMD APU. A dedicated NVIDIA card works too, but see Part 4.
- Proxmox VE 8.2 or newer installed, with root SSH login working by key from this computer. Container device passthrough (`dev0`) needs 8.2.
- At least two data disks for a ZFS mirror, or one disk if you accept no redundancy.
- In Nightgrid: Settings → FEATURES. The **Standard** preset leaves out Proxmox, VM screens and UPS, so pick **Everything** or turn on **Proxmox**, **Desktops** and **UPS** by hand.
- Add the node as a host (**+** in the sidebar) and connect once, so its host key is trusted.

## Part 1 · Storage

Keep media and downloads on **one** dataset with sub-folders. The *arr apps move finished downloads by hardlink, and hardlinks only work inside a single filesystem. Separate datasets turn every import into a full copy.

- [ ] Open a terminal on the node from [Terminal](view:terminal) and check the pool you made during install, or make one now (replace the disk ids with yours from `ls -l /dev/disk/by-id`).

```sh
zpool create -o ashift=12 tank mirror /dev/disk/by-id/<disk-a> /dev/disk/by-id/<disk-b>
```

- [ ] Create the media dataset with settings that suit large files.

```sh
zfs create -o recordsize=1M -o compression=lz4 -o atime=off tank/data
```

- [ ] Create the folder layout.

```sh
mkdir -p /tank/data/media/movies /tank/data/media/tv /tank/data/media/music /tank/data/downloads
```

- [ ] Give the folders to the user the container's apps will run as. In an unprivileged LXC, uid 1000 inside the container is uid 101000 on the host.

```sh
chown -R 101000:101000 /tank/data
```

#### Check

- [ ] [System → Fleet](view:fleet) → **Disks** shows `tank` as ONLINE with no errors. The **full in about N days** forecast starts after about a day of monitoring.

## Part 2 · The media container

One unprivileged LXC runs Docker and all the media apps. Compared with a VM, it shares the iGPU without passthrough, uses less RAM, and reaches the ZFS dataset through a bind mount instead of NFS.

- [ ] [Proxmox](view:proxmox) → **STORAGE** → **Get CT template…** → a Debian 12 template.
- [ ] **GUESTS** → **+ Container**. Debian 12, 4 cores, 4096 MB RAM, a 32 GB root disk, a static IP on `vmbr0`, and your SSH public key (a key is required; no password is set). Leave it **unprivileged**.
- [ ] Before you start it, turn on the features Docker needs inside an LXC. Replace `<ctid>` with the container's id.

```sh
pct set <ctid> --features nesting=1,keyctl=1
```

- [ ] Bind-mount the media dataset into the container at `/data`.

```sh
pct set <ctid> --mp0 /tank/data,mp=/data
```

- [ ] Start it from **GUESTS**, then add it to Nightgrid as its own host (its IP, user `root`, your key). From now on it shows up in Docker, System and monitoring like any other host.
- [ ] In a terminal on the container, install Docker Engine with the Compose plugin from Docker's own repository. Debian's `docker-compose` package is the old v1 tool.

```sh
apt update && apt install -y curl && curl -fsSL https://get.docker.com | sh
```

#### Check

- [ ] `ls /data` in the container lists `media` and `downloads`.
- [ ] [Docker](view:docker) shows the container host with the engine version in the header.

## Part 3 · Hardware transcoding (iGPU into the LXC)

- [ ] On the **node**, confirm the render device exists.

```sh
ls -l /dev/dri
```

- [ ] In the **container**, find the gid of its `render` group (it's usually 104 or 993 on Debian; use what this prints).

```sh
getent group render
```

- [ ] On the **node**, pass the render device into the container with that gid, then restart the container from **GUESTS**.

```sh
pct set <ctid> --dev0 /dev/dri/renderD128,gid=<render-gid>
```

#### Check

- [ ] In the container, `ls -l /dev/dri` shows `renderD128` owned by group `render`.

## Part 4 · Optional: NVIDIA card in a VM instead

Use this only if you have a dedicated NVIDIA card and want it in a VM rather than sharing an iGPU with an LXC. PCI passthrough needs IOMMU and binds the card to that one VM.

- [ ] On the node, check that the IOMMU is on. Kernels 6.8 and newer turn it on by default for Intel and AMD.

```sh
dmesg | grep -e DMAR -e IOMMU
```

- [ ] If nothing shows up, add `intel_iommu=on iommu=pt` (Intel) or `iommu=pt` (AMD) to the kernel command line. GRUB installs keep it in `/etc/default/grub` (then run `update-grub`). ZFS-on-root installs that boot with systemd-boot keep it in `/etc/kernel/cmdline` (then run `proxmox-boot-tool refresh`). Edit either file from [Files](view:files) and reboot.
- [ ] Load the VFIO modules at boot.

```sh
printf 'vfio\nvfio_iommu_type1\nvfio_pci\n' >> /etc/modules
```

- [ ] Rebuild the initramfs, then reboot the node from System → **REBOOT…**.

```sh
update-initramfs -u -k all
```

- [ ] Create a Debian VM with **+ VM**, then open its **HARDWARE** tab and add the card under PCI passthrough. Devices in the same IOMMU group come along.
- [ ] Install the NVIDIA driver inside the VM and add the VM as a Nightgrid host. Monitoring reads NVIDIA GPUs through `nvidia-smi`, so busy %, VRAM, temperature and power show up in the GPU charts.

#### Check

- [ ] `nvidia-smi` in the VM lists the card, and the **GPUS** widget on the [Overview](view:overview) shows it.

## Part 5 · The media stack

Make a stack folder on the container and create the stack with [Docker](view:docker) → **+ New stack** → pasted YAML. Keep secrets out of the compose file: put them in the stack's **Vault** and use **Deploy .env…**.

The admin UIs (the *arr apps, the download client) are bound to `127.0.0.1` so nothing on the LAN can reach them. You reach them through Nightgrid tunnels (Part 9). Only Jellyfin's playback port is open to the LAN.

Paste this into **+ New stack** (or the compose editor); don't send it to a terminal. Set `PUID`, `PGID` and `TZ` in the Vault, and `RENDER_GID` to the gid from Part 3.

```yaml
services:
  jellyfin:
    image: jellyfin/jellyfin:latest
    restart: unless-stopped
    ports:
      - "8096:8096"
    devices:
      - /dev/dri/renderD128:/dev/dri/renderD128
    group_add:
      - "${RENDER_GID}"
    volumes:
      - ./jellyfin/config:/config
      - ./jellyfin/cache:/cache
      - /data/media:/data/media:ro
  sonarr:
    image: lscr.io/linuxserver/sonarr:latest
    restart: unless-stopped
    environment:
      - PUID=${PUID}
      - PGID=${PGID}
      - TZ=${TZ}
    ports:
      - "127.0.0.1:8989:8989"
    volumes:
      - ./sonarr:/config
      - /data:/data
  radarr:
    image: lscr.io/linuxserver/radarr:latest
    restart: unless-stopped
    environment:
      - PUID=${PUID}
      - PGID=${PGID}
      - TZ=${TZ}
    ports:
      - "127.0.0.1:7878:7878"
    volumes:
      - ./radarr:/config
      - /data:/data
  prowlarr:
    image: lscr.io/linuxserver/prowlarr:latest
    restart: unless-stopped
    environment:
      - PUID=${PUID}
      - PGID=${PGID}
      - TZ=${TZ}
    ports:
      - "127.0.0.1:9696:9696"
    volumes:
      - ./prowlarr:/config
  qbittorrent:
    image: lscr.io/linuxserver/qbittorrent:latest
    restart: unless-stopped
    environment:
      - PUID=${PUID}
      - PGID=${PGID}
      - TZ=${TZ}
      - WEBUI_PORT=8080
    ports:
      - "127.0.0.1:8080:8080"
      - "6881:6881"
      - "6881:6881/udp"
    volumes:
      - ./qbittorrent:/config
      - /data/downloads:/data/downloads
```

- [ ] Ask DRIFT to review the file before the first **up**. It flags missing restart policies, ports on 0.0.0.0 and inline secrets. The `:latest` tags are deliberate here; pin versions if you prefer to update by hand.
- [ ] Bring the stack **up** from **Stacks**.
- [ ] In Sonarr and Radarr, use `/data/media/tv`, `/data/media/movies` and `/data/downloads` as their paths so hardlinks work.
- [ ] In Jellyfin: Dashboard → Playback → Transcoding → **Intel QuickSync (QSV)** or **VAAPI**, device `/dev/dri/renderD128`.

#### Check

- [ ] Play something that needs a transcode (force a lower bitrate in the client). Jellyfin's dashboard shows the stream as transcoding with hardware acceleration and the container's CPU stays low in [Docker](view:docker).
- [ ] Nightgrid doesn't read Intel GPUs yet. To watch the iGPU, add an [Overview](view:overview) **+ Widget → COMMAND…** tile that reads the GPU's current clock (0 when idle). The card number can be `card0` or `card1`.

```sh
cat /sys/class/drm/card*/gt_act_freq_mhz | head -1
```

## Part 6 · Make it survive on its own

This is the part that lets the box run with Nightgrid closed. All of it lives on the node.

- [ ] **Autostart and order.** In each guest's **HARDWARE** tab, set *start at boot* and a startup order: storage or network guests first, then the media container. From a terminal on the node it's one line per guest:

```sh
pct set <ctid> --onboot 1 --startup order=2,up=30
```

- [ ] **Container restarts.** Every service in the stack has `restart: unless-stopped`, so Docker brings them back after the LXC boots.
- [ ] **Backups.** Nightgrid lists backup jobs in [Proxmox](view:proxmox) → **BACKUPS** but doesn't create them yet. Create a nightly job on the node. This one backs up the container's root disk; the media dataset is a bind mount and is left out on purpose.

```sh
pvesh create /cluster/backup --schedule "03:00" --vmid <ctid> --storage local --mode snapshot --compress zstd --prune-backups keep-daily=7,keep-weekly=4 --enabled 1
```

- [ ] **Scrubs.** Proxmox ships the ZFS scrub cron job from Debian (second Sunday of the month). Confirm it's there in System → **schedules**, or:

```sh
cat /etc/cron.d/zfsutils-linux
```

- [ ] **SMART.** `smartmontools` is installed on Proxmox and `smartd` runs by default. Confirm it's active under System → **services**.
- [ ] **Host updates.** Never put `unattended-upgrades` on the Proxmox node: a half-applied PVE upgrade is worse than a late one. Update it by hand from [System → Fleet](view:fleet) → **Updates**. Unattended security updates *inside* the Debian container are fine.
- [ ] **UPS.** Nightgrid's Fleet → **UPS shutdown** only works while Nightgrid runs (the tray is enough). For a server that has to protect itself, let NUT on the node shut it down. Install it, then set `MODE=standalone` in `/etc/nut/nut.conf`, define the UPS in `/etc/nut/ups.conf` (driver `usbhid-ups`, port `auto`) and add a `MONITOR` line to `/etc/nut/upsmon.conf`. Edit all three from [Files](view:files).

```sh
apt install -y nut
```

- [ ] Add the UPS in Fleet → **UPS** as `ups@localhost` on the node, so you can watch it when the app is open.

#### Check

- [ ] Reboot the node from System → **REBOOT…**. Without touching anything, the media container starts and Jellyfin answers on port 8096 within a few minutes.
- [ ] [Proxmox](view:proxmox) → **BACKUPS** lists the job and shows no uncovered guests (or only ones you meant to leave out).

## Part 7 · Alerts while the app is closed

Nightgrid's alerts only fire while it runs. If you want to hear about a failed backup or a degraded pool at 3 AM, add a notify-only path on the server. It sends messages and can't change anything, so it doesn't open a second way to control the box.

- [ ] Proxmox's own notification system (8.1 and newer) reports backup failures, and an ntfy, Gotify or SMTP target can be added. Nightgrid has no screen for this yet: open the web UI through the tunnel in Part 9 → Datacenter → **Notifications**, add a target, and point the default matcher at it. Keep the token in the web form, not on a command line.
- [ ] For pool problems, ZFS's event daemon (ZED) mails on faults. Set `ZED_EMAIL_ADDR` in `/etc/zfs/zed.d/zed.rc` from [Files](view:files), then restart it.

```sh
systemctl restart zfs-zed
```

## Part 8 · Lock it down so Nightgrid is the way in

Nightgrid only needs SSH. Proxmox calls go through `pvesh` and VM screens through `qm vncproxy`, both over SSH, so port 8006 isn't needed from the network.

- [ ] **Web UI to localhost only.** Don't stop `pveproxy`; Proxmox's own tools still use it on the node. Bind it to `127.0.0.1` instead.

```sh
echo 'LISTEN_IP="127.0.0.1"' >> /etc/default/pveproxy
```

```sh
systemctl restart pveproxy spiceproxy
```

- [ ] **SSH by key only.** Write a drop-in file, test the config, and keep your current session open until a *new* login works. Nightgrid's 90-second rollback covers firewall and network changes, **not** sshd.

```sh
printf 'PasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin prohibit-password\n' > /etc/ssh/sshd_config.d/10-keys-only.conf
```

```sh
sshd -t && systemctl reload ssh
```

- [ ] Open a **new** terminal tab on the node from Nightgrid. If it connects, close the old one.
- [ ] **Firewall.** In [Proxmox](view:proxmox) → **FIREWALL**, add datacenter rules that accept TCP 22 from your LAN and from your tailnet (`100.64.0.0/10`), then turn the datacenter firewall on and press **Keep** within 90 seconds. With it on, Proxmox drops other inbound traffic to the node.
- [ ] Open the media container's own **FIREWALL** tab and allow TCP 8096 from your LAN, plus 6881 TCP and UDP if you seed. The guest's network device needs its firewall flag set for guest rules to apply.
- [ ] Run System → **security** on the node and on the container. The port list should show 8006 as local only, and the *arr ports as local only.

#### Check

- [ ] From another machine on the LAN, opening the node on port 8006 fails, and so does Sonarr on port 8989.
- [ ] [Proxmox](view:proxmox) in Nightgrid still shows every guest, and **Screen** on a VM still opens.

## Part 9 · Daily control from Nightgrid

- [ ] **Admin UIs through tunnels.** In [Tunnels](view:tunnels), add **Local** tunnels on the media container: 8989, 7878, 9696 and 8080, each to `127.0.0.1` on the same port. Tick start-with-app, then open them in a browser on `http://127.0.0.1:<port>`.
- [ ] **The Proxmox web UI, when you need it.** A **Local** tunnel on the node from `127.0.0.1:8006` to `127.0.0.1:8006` brings it back on this computer only. Keep it saved but stopped.
- [ ] **Health checks.** In [Services](view:services), add Jellyfin at `http://<ct-ip>:8096/health` (it answers `Healthy`). Add the *arr apps against their tunnel ports with the `/ping` path (`http://127.0.0.1:8989/ping`). These checks only pass while the tunnels are up, which they are whenever Nightgrid runs.
- [ ] If you use Plex, add it as an integration to see what's playing. Jellyfin and the *arr apps have no integration yet, so the service checks cover them.
- [ ] **Dependencies.** Settings → **Dependencies**: make the media services depend on the node, so a node outage raises one alert instead of six.
- [ ] **Updating the stack.** Docker → **Updates** shows which images moved. Updating takes a **Snapshot** first, and **Roll back…** undoes it.
- [ ] **Before risky changes** (a Debian upgrade inside the container, a big Jellyfin version jump), take a Proxmox snapshot from the guest's **SNAPSHOTS** tab.
- [ ] **Remote access.** Install Tailscale on the node and advertise your LAN as a subnet route, then use Settings → **Tailnet** → **Built-in Tailscale** in Nightgrid. Hosts saved with their LAN address keep working away from home.

```sh
curl -fsSL https://tailscale.com/install.sh | sh
```

```sh
tailscale up --advertise-routes=<lan-cidr> --ssh=false
```

## Part 10 · Break-glass

If SSH is the only way in, a broken sshd config, a lost key or a bad firewall rule leaves no way to manage the box. Set this up before you need it.

- [ ] Physical console: a monitor and keyboard, or a KVM-over-IP device on a headless box. The node's console login always works.
- [ ] A second SSH key, kept offline, already listed on the node. Settings → **CONNECTIONS** → **SSH KEYS** → **CHECK HOSTS** shows where each key is accepted.
- [ ] A note in [Knowledge](view:notes), tied to the node, with the undo commands, so they're one **Send to terminal** away:

```sh
sed -i '/^LISTEN_IP=/d' /etc/default/pveproxy && systemctl restart pveproxy spiceproxy
```

```sh
rm /etc/ssh/sshd_config.d/10-keys-only.conf && systemctl reload ssh
```

```sh
pve-firewall stop
```

- [ ] Export your Nightgrid profile with secrets (Settings → **DATA** → **BACKUP · PROFILE** → **Export**) to a sync folder, so a lost laptop doesn't take your host list and keys with it.

## When Nightgrid is closed

Keeps running on the server:

- Guests starting at boot in order, Docker restarting containers.
- Jellyfin/Plex streaming, the *arr apps searching and importing, the download client.
- vzdump backups, ZFS scrubs, `smartd`, NUT's UPS shutdown.
- Proxmox and ZED notifications, if you set them up.
- Power schedules made in System → **POWER SCHEDULE** (they're systemd timers on the host).

Needs Nightgrid running (the tray is enough):

- Nightgrid's alerts, uptime history and monitoring charts.
- Fleet → **UPS shutdown** of other hosts.
- Tunnels to the admin UIs, and so the service checks against them.

Needs you in Nightgrid:

- Every change: guests, hardware, network, firewall, stacks, updates, restores.

## Limits and gotchas

- Intel GPUs aren't shown in Nightgrid's GPU monitoring yet; use the command widget from Part 5.
- Nightgrid can't create vzdump jobs or Proxmox notification targets yet; use `pvesh` or the tunnelled web UI.
- Proxmox SDN and IP set entries stay in the web UI.
- Unprivileged LXC uid mapping means host-side files must be owned by 100000 + the uid inside the container. If an app can't write, check ownership first.
- Docker inside an LXC on ZFS needs ZFS 2.2 or newer (Proxmox 8.1+) for the overlay2 storage driver. On older installs, Docker falls back to the slow `vfs` driver.
- A container backup doesn't include bind mounts. Protect `/tank/data` with ZFS snapshots or a separate restic job and watch it in Fleet → **Backups**.
- Ports bound to `127.0.0.1` in compose are only reachable from inside the container's host, so Nightgrid's tunnels must be set up on the media container, not on the node.

## Related

- Game server host
- Family cloud and photos
- Smart home hub
- Remote family IT
- Repair bench server
- Local AI GPU server
- Dev and git deploy lab
