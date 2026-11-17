# Open VM screens and RDP

See and use a virtual machine's graphical console inside Nightgrid, and connect to Windows guests over RDP, without opening any ports.

## What you end up with

- A tab per VM screen, streamed over the host's SSH connection
- Ctrl+Alt+Del, typed text and clipboard sharing
- RDP to Windows guests through an automatic tunnel

## Before you start

- libvirt/KVM guests with a VNC display on localhost (virt-manager's default), or Proxmox VMs (through `qm vncproxy`, which needs root).
- Settings → **FEATURES**: **Desktops** on.

## Part 1 · Open a screen

- [ ] [System](view:system) → **VMs**, or [Proxmox](view:proxmox) → GUESTS: **Screen** on a running VM. It opens in [Desktops](view:desktops).
- [ ] Click the screen to send keys (magenta outline). App shortcuts pause until you click elsewhere.
- [ ] **Ctrl+Alt+Del** and **Type text…** for installers and login prompts.
- [ ] **Fit**, **1:1**, **Full** and **Redraw** in the toolbar.

## Part 2 · Clipboard

- [ ] **Send clipboard** and **Copy guest clipboard** need a clipboard agent in the guest (`spice-vdagent` on Linux guests). Without one, use **Type text…**.

## Part 3 · RDP

- [ ] **RDP** on a VM looks up its address (DHCP leases or the QEMU guest agent).
- [ ] Nightgrid tunnels a free local port through the host and opens your RDP client. Sign in with the guest's account.
- [ ] Running tunnels are listed at the top of Desktops with **Stop**.

#### Check

- [ ] A VM's screen opens, and typing in it works.

## Limits and gotchas

- SPICE-only libvirt guests say so; add a VNC display to use them here.
- Typing assumes the guest uses the same keyboard layout as this computer.
- Install the QEMU guest agent in VMs (`qemu-guest-agent`) for addresses and clean shutdowns.

## Related

- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Remote family IT](kb:guides/remote-family-it)
