# Errors: Proxmox, KVM, libvirt and LXC

Messages from Proxmox VE (qm, pct, vzdump, pvesm), QEMU/KVM, libvirt and LXC containers. Nightgrid's [Proxmox](view:proxmox) view shows the task log with these lines.

### TASK ERROR: start failed: QEMU exited with code 1
Means: The VM couldn't start; the real reason is in the lines just above (memory, a missing disk, a PCI device).
Fix: Read the task log's earlier lines; `qm start <vmid>` on the node prints them too.

### kvm: failed to initialize kvm: Permission denied
Means: QEMU can't use /dev/kvm (permissions, or nested virtualization not enabled in a VM).
Fix: Check `ls -l /dev/kvm` and the kvm group. Inside a VM, enable nested virtualization on the host.

### KVM virtualisation configured, but not available. Either disable in VM configuration or enable in BIOS.
Means: Proxmox can't use hardware virtualization: VT-x/AMD-V is off in the BIOS.
Fix: Enable it in the firmware setup and reboot.

### TASK ERROR: VM <vmid> not running
Means: The action needs a running VM.
Fix: Start it first.

### TASK ERROR: can't lock file '/var/lock/qemu-server/lock-<vmid>.conf' - got timeout
Means: Another task holds the VM's lock (a backup, a migration, or a stuck task).
Fix: Wait for the other task; if nothing runs, `qm unlock <vmid>`.

### VM is locked (backup)
Means: A backup left the VM locked (it finished uncleanly or is still running).
Fix: If no backup runs, `qm unlock <vmid>` (or `pct unlock <ctid>`).

### TASK ERROR: VM is locked (snapshot-delete)
Means: A snapshot operation was interrupted.
Fix: `qm unlock <vmid>`, then check `qm listsnapshot <vmid>` and clean up.

### TASK ERROR: unable to create VM <vmid> - VM <vmid> already exists on node '<node>'
Means: That VMID is in use (maybe on another cluster node).
Fix: Pick a free VMID (`pvesh get /cluster/nextid`).

### kvm: -device vfio-pci,host=<pci>: vfio <pci>: failed to setup container for group <n>: Failed to set iommu for container: Operation not permitted
Means: PCI passthrough failed: IOMMU not enabled, or the device shares an IOMMU group with others.
Fix: Enable VT-d/AMD-Vi in BIOS, add `intel_iommu=on` (Intel) to the kernel command line, check groups (`find /sys/kernel/iommu_groups -type l`).

### TASK ERROR: cannot prepare PCI pci device <pci>, device not found
Means: The PCI address in the VM config doesn't exist (hardware moved or removed).
Fix: Find the new address with `lspci` and update the VM's hostpci line.

### vfio-pci <pci>: BAR <n>: can't reserve [mem <range>]
Means: The host's framebuffer (efifb/simplefb) still uses the GPU's memory.
Fix: Add `video=efifb:off` (or `initcall_blacklist=sysfb_init`) to the kernel command line for GPU passthrough.

### TASK ERROR: storage '<storage>' does not exist
Means: The VM or container references a storage ID that isn't defined (renamed or removed).
Fix: `pvesm status`; fix the disk line in the VM config, or recreate the storage entry.

### TASK ERROR: no such logical volume <vg>/<lv>
Means: A VM disk on LVM is missing.
Fix: `lvs` to look for it; restore from backup if it's gone.

### TASK ERROR: activating LV '<vg>/<lv>' failed: Check of pool <vg>/data failed (status:1). Manual repair required!
Means: The thin pool's metadata is damaged (often after power loss).
Fix: `lvconvert --repair <vg>/data` with the pool inactive. Take backups first.

### command '<cmd>' failed: exit code <n>
Means: A Proxmox task ran a helper command that failed; the output above says why.
Fix: Run the same command by hand on the node to see the full message.

### got timeout
Means: A Proxmox operation (often a QMP command or storage call) didn't answer in time.
Fix: Check node load and storage latency; a hung NFS or iSCSI storage often causes this.

### VM <vmid> qmp command 'query-proxmox-support' failed - unable to connect to VM <vmid> qmp socket - timeout after <n> retries
Means: Proxmox can't talk to the VM's QEMU process (hung, or the node is overloaded).
Fix: Check node load; `qm stop <vmid>` if the VM is frozen.

### TASK ERROR: VM quit/powerdown failed - got timeout
Means: The guest didn't shut down in time (no guest agent or ACPI response).
Fix: Install qemu-guest-agent in the guest and enable it in Options; or stop it hard.

### QEMU guest agent is not running
Means: Proxmox can't reach the agent inside the VM.
Fix: Install and start `qemu-guest-agent` in the guest, enable "QEMU Guest Agent" in the VM's options, and reboot the VM once.

### TASK ERROR: migration aborted
Means: Live migration failed; reasons are above (local disks, CPU type differences, a passthrough device).
Fix: Use a CPU type both nodes support (x86-64-v2-AES or a common model); shared or replicated storage for disks.

### ERROR: Backup of VM <vmid> failed - <details>
Means: vzdump failed for that guest; the detail says why (storage full, snapshot failed, guest agent freeze timeout).
Fix: Read the detail; check backup storage space and `qm guest cmd <vmid> fsfreeze-status`.

### INFO: guest-agent 'fs-freeze' command failed: got timeout
Means: The guest agent couldn't freeze filesystems before snapshot backup in time.
Fix: Usually the backup still works (without consistency); check the guest agent version and heavy I/O.

### unable to parse value of '<key>' - format error
Means: A VM or container config line has an invalid value (hand-edited /etc/pve/qemu-server/<vmid>.conf).
Fix: Fix the line; `qm config <vmid>` shows what Proxmox parses.

### cluster not ready - no quorum?
Means: In a Proxmox cluster, the node can't see a majority of votes, so /etc/pve is read-only.
Fix: Get the other nodes back online. For a planned two-node setup add a QDevice; `pvecm expected 1` only temporarily on a single surviving node.

### pmxcfs: unable to open database '/var/lib/pve-cluster/config.db'
Means: The Proxmox cluster filesystem's database is damaged or missing.
Fix: Restore config.db from a backup; get help before acting on clusters.

### TASK ERROR: CT <ctid> already running
Means: The container is already started.
Fix: Nothing to do.

### lxc-start: <ctid>: lxccontainer.c: wait_on_daemonized_start: <n> No such file or directory - Failed to receive the container state
Means: The LXC container failed early during start; the real error is in the debug log.
Fix: `pct start <ctid> --debug` (or `lxc-start -n <ctid> -F -l DEBUG`) to see the failing step.

### unable to create CT <ctid> - unable to open file '<path>' - No such file or directory
Means: The container template isn't downloaded or the path is wrong.
Fix: Download the template (`pveam update; pveam download local <template>`).

### TASK ERROR: unable to mount <volume>: <details>
Means: Proxmox couldn't mount the container's root or a mount point (missing volume, wrong permissions, quota).
Fix: Check the volume exists (`pvesm list <storage>`); run `pct fsck <ctid>` for filesystem errors.

### newuidmap: write to uid_map failed: Operation not permitted
Means: Unprivileged containers need subuid/subgid ranges for root on the host.
Fix: Check /etc/subuid and /etc/subgid contain `root:100000:65536`.

### Permission denied on bind mount in unprivileged container
Means: In unprivileged LXC, uid 1000 inside is 101000 on the host, so host files owned by 1000 appear as nobody.
Fix: chown the host directory to 101000 (or map ids with lxc.idmap); or use a privileged container only when needed.

### TASK ERROR: missing 'arch' - internal error
Means: A container config is incomplete or corrupt.
Fix: Compare with another container's config in /etc/pve/lxc/; restore from backup.

### error: failed to connect to the hypervisor
Means: virsh can't reach libvirtd (not running, or you lack permission on the socket).
Fix: `sudo systemctl start libvirtd` (or virtqemud); add your user to the libvirt group; use `virsh -c qemu:///system`.

### error: Failed to start domain '<name>'
Means: libvirt couldn't start the VM; the next line gives the reason.
Fix: Read it; `/var/log/libvirt/qemu/<name>.log` has QEMU's own output.

### error: Cannot access storage file '<path>': Permission denied
Means: The QEMU process (user libvirt-qemu or qemu) can't read the disk image, often in a home directory, or SELinux/AppArmor blocks it.
Fix: Move images to /var/lib/libvirt/images, or give the qemu user access to the path.

### error: Requested operation is not valid: network 'default' is not active
Means: The VM uses libvirt's default NAT network, which isn't started.
Fix: `virsh net-start default` and `virsh net-autostart default`.

### error: internal error: process exited while connecting to monitor: <details>
Means: QEMU died right after starting; the details give the cause (bad device, missing firmware file, memory).
Fix: Read the details and the VM's log in /var/log/libvirt/qemu/.

### Could not access KVM kernel module: No such file or directory
Means: The kvm module isn't loaded (no hardware virtualization, or in a VM without nesting).
Fix: Enable virtualization in BIOS; `modprobe kvm_intel` or `kvm_amd`.

### qemu-img: Could not open '<file>': Could not open '<file>': Permission denied
Means: qemu-img can't read the image file.
Fix: Run as root or fix permissions; stop the VM if the image is locked.

### qemu-img: Failed to get shared "write" lock
Means: The image is in use by a running VM.
Fix: Stop the VM, or use `-U` for read-only inspection.

### VNC: Failed to connect to server: Connection refused
Means: The VM's VNC server isn't listening (VM stopped, VNC disabled, or a different display).
Fix: Check the VM runs; Nightgrid's Desktops view connects to the local VNC over SSH.
