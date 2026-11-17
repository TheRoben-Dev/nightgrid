# GPU passthrough to a VM

Give a whole GPU to a virtual machine on Proxmox (or libvirt) for gaming, AI or transcoding, with IOMMU set up correctly.

![The host hands the whole GPU to one VM through VFIO and the IOMMU.](fig:gpu-passthrough)

## Before you start

- CPU and board with IOMMU (Intel VT-d or AMD-Vi) enabled in the firmware.
- A GPU the host doesn't need for its own console (or a CPU with integrated graphics for the host).

## Part 1 · Enable IOMMU

- [ ] Intel: add `intel_iommu=on iommu=pt` to the kernel command line. AMD: IOMMU is on by default on recent kernels; add `iommu=pt`.
- [ ] Proxmox with GRUB: edit `GRUB_CMDLINE_LINUX_DEFAULT` in `/etc/default/grub`, then `update-grub`. With systemd-boot (ZFS root): edit `/etc/kernel/cmdline`, then `proxmox-boot-tool refresh`.
- [ ] Load VFIO modules: add `vfio`, `vfio_iommu_type1`, `vfio_pci` to `/etc/modules`, then `update-initramfs -u -k all` and reboot.
- [ ] Check: `dmesg | grep -e DMAR -e IOMMU` and that the GPU is alone in its group:

```sh
for d in /sys/kernel/iommu_groups/*/devices/*; do n=${d#*/iommu_groups/*}; n=${n%%/*}; printf 'group %s ' "$n"; lspci -nns "${d##*/}"; done | grep -i -e vga -e audio
```

## Part 2 · Keep the host off the GPU

- [ ] Find its ids (`lspci -nn`, e.g. `10de:2684` and its audio function) and bind them to vfio-pci:

```sh
echo "options vfio-pci ids=10de:2684,10de:22ba" > /etc/modprobe.d/vfio.conf
update-initramfs -u -k all
```

## Part 3 · Attach it

- [ ] The VM should use machine type q35 and OVMF (UEFI).
- [ ] [Proxmox](view:proxmox) → the VM → **HARDWARE** → PCI passthrough: pick the GPU (devices in the same IOMMU group go along), tick all functions and PCI-Express.
- [ ] Install the GPU driver inside the guest.

#### Check

- [ ] `nvidia-smi` (or the vendor tool) works inside the guest, and Nightgrid's monitor on the guest shows the GPU.

## Limits and gotchas

- A passed-through GPU can't be shared with the host or other guests at the same time.
- Some consumer AMD cards don't reset cleanly between VM restarts ("reset bug"); a host reboot fixes it.
- For containers (LXC) you don't need passthrough; share `/dev/dri` instead (see [LXC containers on Proxmox](kb:apps/proxmox-lxc)).

## Related

- [Local AI GPU server](kb:guides/local-ai-gpu-server)
- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Open VM screens and RDP](kb:howto/vm-desktops)
