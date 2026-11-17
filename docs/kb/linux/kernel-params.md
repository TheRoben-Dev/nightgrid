# Change kernel boot parameters

Add or remove kernel command-line options such as `intel_iommu=on` for passthrough, `nomodeset` for a GPU that won't show a console, or `pcie_aspm=off` for a flaky NIC. Where the setting lives depends on the bootloader, so find yours first.

![Changing kernel parameters, step by step.](fig:lx-kernel-params)

## What you end up with

- The parameter tested once from the boot menu, then made permanent
- The right file and update command for your distribution and bootloader
- A way back if the machine doesn't boot with it

## Part 1 · Look first

- [ ] `cat /proc/cmdline` shows the parameters the running kernel booted with.
- [ ] Which bootloader: `ls /boot/grub /boot/grub2 2>/dev/null` (GRUB), `bootctl status` (systemd-boot), `proxmox-boot-tool status` on Proxmox.
- [ ] Test a risky parameter for one boot only: at the GRUB menu press **e**, add it to the end of the line starting with `linux`, then **Ctrl+X**. A reboot without editing drops it again. You need the server's screen for this, or IPMI/PiKVM.

## Part 2 · Make it permanent

- [ ] **Debian, Ubuntu, Proxmox with GRUB**: edit `GRUB_CMDLINE_LINUX_DEFAULT` in `/etc/default/grub`, for example `"quiet intel_iommu=on iommu=pt"`, then `sudo update-grub`.
- [ ] **Fedora, RHEL, Rocky, Alma**: `sudo grubby --update-kernel=ALL --args="intel_iommu=on"` (and `--remove-args=` to undo). It updates every installed kernel's entry.
- [ ] **Proxmox on ZFS or UEFI with systemd-boot**: the line is in `/etc/kernel/cmdline` (one line, keep the existing `root=` part), then `sudo proxmox-boot-tool refresh`.
- [ ] **Fedora Atomic, Bazzite, CoreOS**: `sudo rpm-ostree kargs --append=intel_iommu=on`; it stages a new deployment, so reboot to apply (see [Atomic Fedora hosts (rpm-ostree)](kb:linux/rpm-ostree)).
- [ ] Reboot and confirm with `cat /proc/cmdline`.

## Part 3 · Common ones

- [ ] `intel_iommu=on` / `amd_iommu=on` with `iommu=pt`: IOMMU for PCI passthrough (AMD usually has it on by default).
- [ ] `nomodeset`: no kernel graphics driver at boot; a rescue option for a black screen, not something to keep with a GPU you use.
- [ ] `pcie_aspm=off`: disables PCIe power saving; a known fix for some NICs and NVMe drives that drop off, at the cost of idle power.
- [ ] `usbcore.autosuspend=-1`: stops USB devices (Zigbee sticks, UPS cables) from being put to sleep.
- [ ] `consoleblank=0`: keeps the local console from blanking, useful on a rack screen.

#### Check

- [ ] After reboot the parameter appears in `/proc/cmdline`, and its effect shows (for IOMMU: `dmesg | grep -i -e DMAR -e IOMMU` reports it enabled).

## In Nightgrid

- [System](view:system) → **config** → **Save baseline** before the change; **Compare** later shows `/etc/default/grub` or `/etc/kernel/cmdline` as changed, a reminder of what you set and why.
- Add a note in [Knowledge](view:notes) with the parameter and the reason; six months later nobody remembers why `pcie_aspm=off` is there.

## Limits and gotchas

- If the machine won't boot after a change, pick the old entry or edit the line at the GRUB menu to remove the parameter, boot, then fix the file (see [Fix a server that won't boot](kb:linux/boot-rescue)).
- `mitigations=off` gains a little speed by turning off CPU security fixes; don't use it on a machine that runs anything you don't fully trust, including containers from the internet.
- Editing `/boot/grub/grub.cfg` directly is overwritten by the next kernel update; always edit the source file and regenerate.

## Related

- [GPU passthrough to a VM](kb:apps/gpu-passthrough)
- [Fix a server that won't boot](kb:linux/boot-rescue)
- [Cut a home server's power draw](kb:linux/power-saving)
