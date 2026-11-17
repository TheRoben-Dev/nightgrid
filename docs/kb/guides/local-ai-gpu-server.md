# Local AI GPU server

Run large language models (Ollama or llama.cpp) and image generation (Stable Diffusion front ends such as ComfyUI) in a VM that has a whole NVIDIA GPU passed through. Nightgrid sets up the passthrough, watches the GPU, keeps the APIs private behind SSH tunnels, and powers the box down when you don't need it.

This is separate from DRIFT. DRIFT runs inside the Nightgrid app on your own computer and doesn't use this server.

## What you end up with

- A Linux VM with an NVIDIA card passed through, close to bare-metal speed
- Ollama (and optionally ComfyUI) as compose stacks, with models on a dataset of their own
- GPU load, VRAM, temperature and power in Nightgrid's monitor and the **GPUS** widget
- APIs reachable only from your machines, through saved tunnels
- A box that wakes on demand and shuts itself down at night

## Before you start

- Hardware: a motherboard and CPU with working IOMMU (VT-d on Intel, AMD-Vi on AMD), an NVIDIA card with enough VRAM for your models (12 GB runs 7–14B models comfortably, 24 GB runs 30B-class quantized), and a second GPU or iGPU for the node's own console, or run the node headless.
- A PSU with headroom: inference GPUs run at full power for long periods.
- Settings → **FEATURES**: turn on **Proxmox** and **VM screens** (the **Standard** preset leaves both out).
- Fast storage for models. A 70B model is 40 GB or more, and loading it from HDD takes minutes.

## Part 1 · IOMMU and vfio on the node

- [ ] Turn on VT-d / AMD-Vi (and "Above 4G decoding") in the BIOS.
- [ ] Check that IOMMU is active (node Terminal). On PVE 8 with kernel 6.8+, it's on by default for Intel and AMD:

```sh
dmesg | grep -e DMAR -e IOMMU -e AMD-Vi
```

- [ ] If nothing shows, add `intel_iommu=on iommu=pt` (Intel) or `iommu=pt` (AMD) to the kernel command line: `/etc/default/grub` with GRUB, or `/etc/kernel/cmdline` with systemd-boot (ZFS root on UEFI). Edit the file in [Files](view:files), then:

```sh
proxmox-boot-tool refresh
```

- [ ] Load the vfio modules at boot:

```sh
printf 'vfio\nvfio_iommu_type1\nvfio_pci\n' >> /etc/modules
```

- [ ] Keep the node's own drivers off the card:

```sh
printf 'blacklist nouveau\nblacklist nvidia\n' > /etc/modprobe.d/blacklist-gpu.conf
```

```sh
update-initramfs -u -k all
```

- [ ] Reboot the node: [System](view:system) → **REBOOT…**.
- [ ] Note the card's IOMMU group. The GPU and its audio function usually share a group, and that's fine:

```sh
lspci -nnk | grep -A3 -i nvidia
```

#### Check

- [ ] `lspci -nnk` shows `Kernel driver in use: vfio-pci` for the card, or no driver at all.

## Part 2 · The VM

- [ ] [Proxmox](view:proxmox) → **GUESTS** → **+ VM**: Ubuntu Server 24.04 or Debian 12, machine type q35, OVMF (UEFI) firmware, CPU type `host`, at least 16 GB RAM (more than the largest model you'll load into RAM).
- [ ] In the VM's **HARDWARE**, add the GPU under PCI passthrough. Devices in the same IOMMU group go along. Tick all functions and PCI-Express. RAM is pinned for passthrough VMs, so ballooning is off.
- [ ] Install the OS through **Screen** in [Desktops](view:desktops), then add the VM as a host in Nightgrid.
- [ ] Install the NVIDIA driver and the NVIDIA Container Toolkit in the VM, following NVIDIA's instructions for the distribution.
- [ ] Turn on **start at boot** in **HARDWARE** boot options.

#### Check

- [ ] In a Terminal on the VM, `nvidia-smi` lists the card.
- [ ] Nightgrid's monitor for the VM shows the GPU, and it appears in the **GPUS** widget on the [Overview](view:overview).

## Part 3 · Model storage

- [ ] Give the VM a second disk for models in **HARDWARE** (Grow… only grows, so start generous), or mount a node dataset over NFS or virtiofs.
- [ ] Put it at `/srv/models` in the VM and use it for every model folder below.

#### Check

- [ ] `df -h /srv/models` in the VM shows the size you gave it.

## Part 4 · Ollama and ComfyUI stacks

- [ ] In [Docker](view:docker) on the VM, **+ New stack** for Ollama. Bind the API to localhost only.

Paste this into the editor (Docker → Stacks); don't send it to a terminal.

```yaml
services:
  ollama:
    image: ollama/ollama:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:11434:11434"
    volumes:
      - /srv/models/ollama:/root/.ollama
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
```

- [ ] Pull a model (in a Terminal on the VM):

```sh
docker exec -it ollama ollama pull llama3.1:8b
```

- [ ] ComfyUI or another Stable Diffusion front end: a second stack with the same GPU reservation, its web UI bound to `127.0.0.1`, and its models folder under `/srv/models`. Two stacks share one GPU fine, but VRAM doesn't stretch: a big LLM and an SDXL model loaded together may not fit.
- [ ] Pin image tags once the setup works.

#### Check

- [ ] `curl -s http://127.0.0.1:11434/api/tags` in the VM lists the model.

## Part 5 · Private access through tunnels

- [ ] [Tunnels](view:tunnels) → **Local** (`-L`): local port 11434 → `127.0.0.1:11434` on the VM host. Tick start with the app. Your apps on this computer then use `http://127.0.0.1:11434` as if Ollama were local.
- [ ] Do the same for ComfyUI's port.
- [ ] Away from home, the same tunnels work over the built-in Tailscale (Settings → Tailnet).
- [ ] A tunnel listening anywhere other than localhost is flagged as public. Keep it on localhost unless another LAN device needs it.
- [ ] [System](view:system) → **security** on the VM should show 11434 as local only.

#### Check

- [ ] With the tunnel running, `curl http://127.0.0.1:11434/api/tags` works on your own computer.

## Part 6 · Power

- [ ] Idle power with a big GPU is often 60–120 W. Shut the node down when you don't use it.
- [ ] [System](view:system) → Schedules → **POWER SCHEDULE** on the node: Shut down at 01:00. It's a systemd timer on the node, so it works with Nightgrid closed.
- [ ] Wake-on-LAN: put the node's MAC in the host editor and enable WoL in the BIOS. **WAKE** in the System toolbar, with **Wake from** another always-on host when you're away.
- [ ] The VM starts with the node (start at boot), and Ollama starts with the VM (restart policy), so after **WAKE** the API is back within a minute or two.
- [ ] Watch the GPU's temperature and power in the monitor's history (**TEMP** and **GPU W** charts) during long jobs. For a tile that turns orange and red, use **+ Widget → COMMAND…** on the VM with `nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader` and warn and critical thresholds (for example 80 and 87). Tiles only change colour; they don't raise alerts.

#### Check

- [ ] After a scheduled shutdown and a **WAKE**, `curl` through the tunnel works without touching the server.

## When Nightgrid is closed

- Keeps running: the VM, Ollama and ComfyUI, start at boot, the POWER SCHEDULE timer.
- Needs Nightgrid open (or in the tray): the tunnels (they end when Nightgrid quits), GPU monitoring and history, alerts, **WAKE**.
- If other devices need the API with Nightgrid closed, they need their own route: plain `ssh -L`, or exposing the port to the LAN behind the guest's **FIREWALL**.

## Limits and gotchas

- Consumer NVIDIA cards pass through fine on current drivers. Some cards (especially older AMD) have reset bugs that hang the node on a VM restart.
- A card in a group with other devices drags them along. Moving it to another slot often gives it its own group.
- A passthrough VM can't be live-migrated, and snapshots **with RAM** aren't possible while the GPU is attached.
- Nightgrid reads NVIDIA through `nvidia-smi` inside the VM, which must be a Nightgrid host. The node itself doesn't see the card once vfio has it. Intel GPUs aren't read yet.
- The Ollama API has no authentication. Never expose 11434 to the internet.
- Kernel command-line mistakes can stop the node from booting. Have physical or IPMI console access before editing them.

## Related

- Proxmox media server
- Game server host
- Family cloud and photos
- Repair bench server
- Smart home hub
- Remote family IT
- Dev and git deploy lab
