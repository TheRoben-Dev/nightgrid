# Kernel modules that survive updates (DKMS)

ZFS, NVIDIA, some network cards, VirtualBox and many hardware drivers live outside the kernel and must be rebuilt for each new kernel. DKMS does that automatically on kernel updates. When it fails, the next reboot comes up without the module: no pool, no GPU, no network. Here's how to keep it working and fix it when it doesn't.

## What you end up with

- Matching kernel headers installed so DKMS can build
- A quick check before every reboot after a kernel update
- A fix for the usual failures, including Secure Boot signing

## Part 1 · How it works

- [ ] `dkms status` lists modules, their versions and which kernels they're built for:

```text
zfs/2.2.6, 6.8.0-49-generic, x86_64: installed
nvidia/550.120, 6.8.0-49-generic, x86_64: installed
```

- [ ] When a new kernel is installed, a package hook runs `dkms autoinstall` for it. That needs the **headers** for the new kernel.

## Part 2 · Headers that follow the kernel

- [ ] Install the meta package so headers update together with the kernel:

```bash
sudo apt install linux-headers-generic        # Ubuntu
sudo apt install linux-headers-amd64          # Debian
sudo dnf install kernel-devel                 # Fedora/RHEL
```

- [ ] Proxmox uses its own kernel: `apt install proxmox-default-headers`.

## Part 3 · Before you reboot

- [ ] After a kernel update, check the new kernel has every module:

```bash
ls /lib/modules/
dkms status
```

- [ ] Every module should say `installed` for the newest kernel version. If one is missing, build it now (Part 4) instead of discovering it at boot.

## Part 4 · Fix a failed build

```bash
sudo dkms autoinstall -k 6.8.0-50-generic
```

- [ ] The error log path is printed (usually `/var/lib/dkms/<module>/<version>/build/make.log`). Typical causes:
  - headers missing for that kernel: install `linux-headers-<version>`
  - module too old for the new kernel: update the module package (newer ZFS or NVIDIA driver) or stay on the older kernel
  - compiler mismatch after a release upgrade: install the `gcc` version the kernel was built with
- [ ] Still broken? Boot the previous kernel from the GRUB menu (**Advanced options**) and hold kernel updates until the module catches up.

## Part 5 · Secure Boot

- [ ] With Secure Boot on, unsigned modules don't load (`Key was rejected by service` in `dmesg`). On Debian/Ubuntu, DKMS signs with a Machine Owner Key: run `sudo mokutil --import /var/lib/dkms/mok.pub`, set a one-time password, reboot and choose **Enroll MOK** on the blue screen.

#### Check

- [ ] After a reboot into the new kernel, `lsmod | grep -E 'zfs|nvidia'` shows the modules and the pool or GPU works.

## In Nightgrid

- [Fleet](view:fleet) → **Updates** shows which hosts need a reboot after updates. Before rebooting a ZFS or NVIDIA host, check `dkms status` from the [Terminal](view:terminal).
- [System](view:system) → **timeline** after the reboot shows failed units (a ZFS import, a GPU container) if something went wrong.

## Limits and gotchas

- Kernels from backports or HWE stacks can be newer than what your ZFS or NVIDIA package supports. Pin the kernel series on hosts that depend on these modules.
- Removing old kernels with `apt autoremove` keeps the running one and one fallback; make sure the fallback still has working modules.
- Distros that ship ZFS as a prebuilt kmod (Proxmox, some Fedora repos) don't use DKMS for it.

## Related

- [Install NVIDIA drivers on a server](kb:linux/nvidia-drivers)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Fix a server that won't boot](kb:linux/boot-rescue)
