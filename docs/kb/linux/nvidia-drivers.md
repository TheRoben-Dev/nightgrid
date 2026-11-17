# Install NVIDIA drivers on a server

Get an NVIDIA GPU working for containers (Ollama, Jellyfin/Plex transcoding, Frigate, Immich machine learning) on Debian, Ubuntu or Fedora, and keep it working after kernel updates.

![From the GPU up to containers.](fig:lx-nvidia)

## What you end up with

- The driver loaded, with `nvidia-smi` showing the card
- Docker containers that can use the GPU
- A setup that survives kernel updates

## Before you start

- Secure Boot: either off, or be ready to enroll a MOK key at the next boot (DKMS modules must be signed)
- On a Proxmox host passing the GPU to a VM, install the driver inside the VM, not on the host

## Part 1 · The driver

- [ ] Ubuntu: `sudo ubuntu-drivers install --gpgpu` (server, no desktop bits), then reboot.
- [ ] Debian: enable `contrib non-free non-free-firmware` in your sources, then:

```sh
sudo apt install linux-headers-amd64 nvidia-driver firmware-misc-nonfree
```

- [ ] Fedora: enable RPM Fusion, then `sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda` and wait a few minutes for the module to build before rebooting.
- [ ] After reboot: `nvidia-smi` lists the card, driver version and CUDA version.

## Part 2 · Containers

- [ ] Install the NVIDIA Container Toolkit from NVIDIA's repository (follow their install page for your distro), then:

```sh
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
docker run --rm --gpus all ubuntu nvidia-smi
```

- [ ] In compose, give a service the GPU:

```yaml
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
```

## Part 3 · Kernel updates

- [ ] DKMS (Debian/Ubuntu) and akmods (Fedora) rebuild the module for each new kernel. After an update, check `dkms status` before rebooting a remote server.
- [ ] If `nvidia-smi` says "couldn't communicate with the driver" after a reboot, the module didn't build for the new kernel: boot the previous kernel from GRUB and reinstall the headers for the new one.

#### Check

- [ ] `docker run --rm --gpus all ubuntu nvidia-smi` shows the card from inside a container.

## In Nightgrid

- The host's monitor on [Overview](view:overview) shows GPU busy %, VRAM, temperature and power once `nvidia-smi` works, with history.
- [Fleet](view:fleet) → **Updates** shows a pending kernel; update GPU hosts when you can reach the console in case the module fails.

## Limits and gotchas

- Consumer cards limit simultaneous NVENC sessions; the limit has been raised over driver versions, check current numbers before building a transcode box.
- Mixing the distro's driver with NVIDIA's `.run` installer breaks updates; pick one.

## Related

- [Ollama and Open WebUI](kb:apps/ollama-openwebui)
- [Local AI GPU server](kb:guides/local-ai-gpu-server)
- [GPU passthrough to a VM](kb:apps/gpu-passthrough)
