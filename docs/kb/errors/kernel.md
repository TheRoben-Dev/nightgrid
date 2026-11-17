# Errors: kernel, memory and processes

Kernel log (dmesg) messages, out-of-memory kills, hung tasks, hardware errors and process limits. See also [Slow server](kb:linux/slow-server) and [Memory limits](kb:linux/memory-limits).

### Out of memory: Killed process <pid> (<name>)
Means: The kernel ran out of RAM and swap and killed the process with the worst OOM score to recover.
Fix: `dmesg -T | grep -i -A10 'invoked oom-killer'` shows memory use at that moment. Add RAM or swap/zram, cap the hungry service (MemoryMax, container mem_limit), or fix its leak.

### <name> invoked oom-killer: gfp_mask=<n>, order=<n>, oom_score_adj=<n>
Means: The first line of an OOM report: the named process asked for memory that couldn't be found, which triggered the killer (it's not necessarily the one killed).
Fix: Read on to the "Killed process" line to see the victim and the task list for who used the memory.

### Memory cgroup out of memory: Killed process <pid> (<name>)
Means: The process hit a cgroup memory limit (a container's mem_limit, a unit's MemoryMax), not total system memory.
Fix: Raise the limit for that container or unit, or reduce its memory use. `systemd-cgtop` and `docker stats` show usage against limits.

### oom-kill:constraint=CONSTRAINT_MEMCG
Means: The OOM kill was caused by a cgroup limit (container or systemd unit), not by the whole machine running out.
Fix: Find which cgroup in the same log block (`oom_memcg=`), then raise that limit.

### oom_reaper: reaped process <pid> (<name>)
Means: The kernel reclaimed the memory of a process it had just OOM-killed.
Fix: Nothing extra: look at the "Killed process" line before it.

### page allocation failure: order:<n>
Means: The kernel couldn't find a large enough block of contiguous memory (fragmentation), even with free memory overall. Network drivers and ZFS often hit it.
Fix: Usually transient. If frequent, raise `vm.min_free_kbytes` a little, or limit ZFS ARC.

### INFO: task <name>:<pid> blocked for more than 120 seconds.
Means: A process waited over 2 minutes in uninterruptible sleep, almost always on disk or network storage (a slow or hung disk, NFS server gone, overloaded I/O).
Fix: `dmesg -T` for disk or NFS errors around it, `iostat -x 1` for saturated devices. Hung NFS mounts and failing disks are the usual cause.

### watchdog: BUG: soft lockup - CPU#<n> stuck for <n>s!
Means: A CPU ran kernel code without yielding for many seconds. In VMs this is often the host overcommitting CPU; on bare metal a driver bug or hardware problem.
Fix: In a VM check host load and CPU steal (`top`, `st` column). On metal update the kernel and firmware; note which module appears in the stack trace.

### rcu: INFO: rcu_sched self-detected stall on CPU
Means: The kernel's RCU mechanism noticed a CPU not making progress, usually for the same reasons as soft lockups (VM steal, driver bug, hardware).
Fix: As for soft lockups: check host overcommit, update kernel/firmware, look at the module in the trace.

### NMI watchdog: Watchdog detected hard LOCKUP on cpu <n>
Means: A CPU stopped responding to interrupts: a serious hardware, firmware or driver fault.
Fix: Update BIOS and kernel, check temperatures and power, test RAM. Repeated hard lockups often mean failing hardware.

### Kernel panic - not syncing: VFS: Unable to mount root fs on unknown-block(0,0)
Means: The kernel couldn't find the root filesystem: a missing or broken initramfs for this kernel, or a wrong root= device.
Fix: Boot an older kernel from the GRUB menu, then rebuild the initramfs (`update-initramfs -u -k all` or `dracut -f --regenerate-all`) and `update-grub`.

### Kernel panic - not syncing: Attempted to kill init!
Means: PID 1 (systemd) crashed or exited, often because of a broken root filesystem, a bad /sbin/init or a missing library after an interrupted upgrade.
Fix: Boot a rescue system, check the root filesystem, and reinstall systemd/libc packages from a chroot. See [Boot rescue](kb:linux/boot-rescue).

### Kernel panic - not syncing: Fatal exception
Means: The kernel hit an unrecoverable error (a bug or hardware fault) and halted.
Fix: Capture the trace (serial console, `kdump`, or a phone photo). Update the kernel and firmware; test RAM if it repeats.

### BUG: unable to handle page fault for address: <addr>
Means: The kernel accessed invalid memory: a driver bug or corrupt memory.
Fix: Note the module in the call trace; update or blacklist that driver. Run memtest86+ if traces vary.

### general protection fault, probably for non-canonical address
Means: Kernel code used a corrupted pointer, typically a driver bug or RAM corruption.
Fix: As for page fault BUGs: identify the module, update it, test RAM.

### Oops: <n> [#<n>] SMP
Means: The kernel recovered from a crash in one task but is now in an untrusted state.
Fix: Reboot soon. Report the trace; check whether it involves an out-of-tree module (nvidia, zfs, dkms).

### Tainted: <flags>
Means: Information in a trace: the kernel has loaded something that makes bug reports less trustworthy (P = proprietary module, O = out-of-tree, E = unsigned, W = an earlier warning).
Fix: Not an error itself. Upstream developers often ask you to reproduce without the tainting module.

### mce: [Hardware Error]: Machine check events logged
Means: The CPU reported a hardware error: corrected ECC memory errors, cache errors, or bus problems.
Fix: `rasdaemon` (`ras-mc-ctl --errors`) or `mcelog` decodes them. Repeated errors on one DIMM mean replace that DIMM.

### EDAC MC<n>: <n> CE memory read error on <dimm>
Means: ECC RAM corrected a memory error on that module.
Fix: A few are tolerable; a rising count on the same DIMM means it's failing. Replace it before it starts producing uncorrectable errors.

### EDAC MC<n>: <n> UE <details>
Means: An uncorrectable memory error: data in RAM was damaged and couldn't be fixed.
Fix: Replace the DIMM. Check the processes or files that were active for corruption.

### CPU<n>: Core temperature above threshold, cpu clock throttled
Means: The CPU got too hot and slowed itself down.
Fix: Clean dust, check fans and the heatsink mount, reapply paste. `sensors` shows temperatures. See [Sensors and fans](kb:linux/sensors-fans).

### thermal thermal_zone<n>: critical temperature reached, shutting down
Means: A sensor reached its critical limit and the kernel powered off to protect the hardware.
Fix: Fix cooling before running heavy loads again.

### hrtimer: interrupt took <n> ns
Means: A timer interrupt ran long, usually from VM CPU steal or heavy load. Rarely a real problem.
Fix: Ignore unless frequent; then check host overcommit and power management settings.

### clocksource: timekeeping watchdog on CPU<n>: Marking clocksource 'tsc' as unstable
Means: The CPU timestamp counter drifted against another clock, so the kernel switched to a slower clock source.
Fix: Common in VMs and with deep C-states. Update BIOS; in VMs it's usually harmless.

### TSC found unstable after boot, most likely due to broken BIOS. Use 'tsc=unstable'.
Means: The kernel saw the CPU's timestamp counter misbehave.
Fix: BIOS update; otherwise harmless, the kernel picks another clock.

### systemd-journald[<pid>]: Missed <n> kernel messages
Means: The kernel produced messages faster than journald read them (a flood, often a driver spamming).
Fix: Find the flood in `dmesg`; fix that driver or device.

### systemd-journald[<pid>]: Suppressed <n> messages from <unit>
Means: journald rate-limited a unit that logged too much in a short time.
Fix: Make the program log less, or raise `RateLimitBurst=` / `RateLimitIntervalSec=` in journald.conf.

### printk: <n> messages suppressed.
Means: The kernel itself rate-limited repeated log lines.
Fix: Look at the message repeated before it; that's what to fix.

### nf_conntrack: table full, dropping packet
Means: The connection-tracking table is full, so new connections are dropped. Typical on busy routers, torrent hosts or under attack.
Fix: Raise `net.netfilter.nf_conntrack_max` (and the hash size), shorten timeouts, or find the flood with `conntrack -S`.

### TCP: request_sock_TCP: Possible SYN flooding on port <port>. Sending cookies.
Means: The listen queue for that port overflowed: a real SYN flood or simply a service that accepts connections too slowly.
Fix: Check whether the service is overloaded. Raise its backlog and `net.core.somaxconn` if the traffic is legitimate.

### TCP: out of memory -- consider tuning tcp_mem
Means: TCP socket buffers used up the memory the kernel allows for them.
Fix: Look for many stuck connections (`ss -s`); raise `net.ipv4.tcp_mem` only if the load is real.

### neighbour: arp_cache: neighbor table overflow!
Means: More hosts are in the ARP/neighbour table than allowed, common on large flat networks or Docker hosts with many networks.
Fix: Raise `net.ipv4.neigh.default.gc_thresh1/2/3`.

### <iface>: renamed from eth<n>
Means: Not an error: udev gave the interface a predictable name (enp3s0, eno1).
Fix: Use the new name in network config. `ip link` lists them.

### e1000e <pci>: <iface>: Detected Hardware Unit Hang
Means: A known issue with some Intel NICs (often with TSO/GSO offloads) where the transmit queue stalls.
Fix: Disable offloads: `ethtool -K <iface> tso off gso off`, and make it persistent. Update the kernel.

### r8169 <pci> <iface>: rtl_rxtx_empty_cond == 0 (loop: 42, delay: 100).
Means: The Realtek driver timed out waiting for the NIC, often with ASPM power saving.
Fix: Update the kernel; try `pcie_aspm=off`; or use the r8168 driver from your distro.

### usb <bus>: device descriptor read/64, error -71
Means: A USB device didn't answer properly: a bad cable, insufficient power, or a failing device.
Fix: Try another port, cable or a powered hub. Disks on USB should get their own power.

### usb <bus>: reset high-speed USB device number <n>
Means: The USB device stopped responding and was reset. With USB disks this often precedes I/O errors.
Fix: Use a powered hub, a better cable, or a USB-SATA bridge with better firmware. Avoid UAS on bad bridges (`usb-storage.quirks=<vid>:<pid>:u`).

### uas_eh_abort_handler <n> uas-tag <n> inflight: CMD
Means: The UAS (USB Attached SCSI) driver had to abort commands: many cheap USB-SATA bridges misbehave with UAS.
Fix: Disable UAS for that bridge: `usb-storage.quirks=<vid>:<pid>:u` on the kernel command line (find ids with `lsusb`).

### Under-voltage detected! (0x00050005)
Means: A Raspberry Pi's supply voltage dropped below 4.63 V. This causes random crashes and SD/USB corruption.
Fix: Use the official power supply or a better cable; power USB disks separately. `vcgencmd get_throttled` shows history.

### module verification failed: signature and/or required key missing - tainting kernel
Means: An unsigned out-of-tree module (often from DKMS) loaded on a kernel that checks signatures but doesn't enforce them.
Fix: Harmless when Secure Boot is off. With Secure Boot on, enroll a MOK key so DKMS modules are signed. See [DKMS](kb:linux/dkms).

### modprobe: ERROR: could not insert '<module>': Key was rejected by service
Means: Secure Boot is on and the module isn't signed with a key the firmware trusts.
Fix: Enroll the DKMS signing key with `mokutil --import` and reboot to confirm it, or disable Secure Boot.

### modprobe: FATAL: Module <module> not found in directory /lib/modules/<ver>
Means: The module isn't built for the running kernel: kernel updated but not rebooted, DKMS build failed, or the -extra modules package is missing.
Fix: Reboot into the new kernel, check `dkms status`, or install `linux-modules-extra-$(uname -r)` on Ubuntu.

### modprobe: ERROR: could not insert '<module>': Operation not permitted
Means: Loading kernel modules is blocked: you're in a container, kernel lockdown is active, or `kernel.modules_disabled=1`.
Fix: Load modules on the host, not inside containers. Check `cat /sys/kernel/security/lockdown`.

### modprobe: ERROR: could not insert '<module>': Exec format error
Means: The module was built for a different kernel version or configuration.
Fix: Rebuild it for the running kernel (`dkms autoinstall`), or boot the kernel it was built for.

### Direct firmware load for <file> failed with error -2
Means: A driver asked for a firmware file that isn't installed. Often harmless (it tries several names), sometimes the device won't work.
Fix: Install `linux-firmware` (or `firmware-misc-nonfree`/`firmware-realtek` on Debian with non-free-firmware enabled).

### possible missing firmware /lib/firmware/<file> for module <module>
Means: update-initramfs warns that the initramfs may lack firmware for a driver. Usually only matters for hardware you have.
Fix: Ignore if the device works; otherwise install the firmware package and rebuild the initramfs.

### ACPI BIOS Error (bug): Could not resolve symbol <symbol>, AE_NOT_FOUND
Means: The firmware's ACPI tables reference something that doesn't exist. Very common and nearly always harmless.
Fix: Ignore unless a device misbehaves; a BIOS update sometimes removes it.

### ACPI Error: AE_NOT_FOUND, While resolving a named reference package element
Means: Another flavor of buggy ACPI tables in the firmware.
Fix: Usually harmless; update BIOS if suspend, fans or buttons misbehave.

### irq <n>: nobody cared (try booting with the "irqpoll" option)
Means: An interrupt fired repeatedly and no driver handled it, so the kernel disabled that line. A device sharing it may stop working.
Fix: Update BIOS and kernel; try moving the card to another slot; `irqpoll` is a last resort.

### DMAR: DRHD: handling fault status reg <n>
Means: The IOMMU blocked a device's DMA access, often a buggy device or driver, or passthrough misconfiguration.
Fix: Update firmware. For passthrough, check IOMMU groups. `iommu=pt` reduces the impact on the host.

### AMD-Vi: Event logged [IO_PAGE_FAULT domain=<n> address=<addr>]
Means: The AMD IOMMU blocked a device's memory access, often with certain NVMe or GPU firmware.
Fix: Update BIOS and device firmware; try `iommu=pt`.

### kvm: disabled by bios
Means: Hardware virtualization (VT-x/AMD-V) is off in the firmware, so VMs can't use KVM.
Fix: Enable Intel VT-x / AMD SVM in the BIOS setup, then reboot.

### perf: interrupt took too long (<n> > <n>), lowering kernel.perf_event_max_sample_rate to <n>
Means: Informational: the kernel automatically lowered profiling sample rate because handling took long.
Fix: Nothing to do.

### audit: backlog limit exceeded
Means: The audit subsystem produced events faster than auditd could store them, so some were dropped.
Fix: Raise `-b` in audit rules, reduce noisy rules, or check that auditd is running. See [auditd](kb:linux/auditd).

### audit: type=1400 audit(<n>): apparmor="DENIED" operation="<op>" profile="<profile>" name="<path>"
Means: AppArmor blocked a program from accessing that path or doing that operation.
Fix: `sudo aa-status` shows profiles. Adjust the profile with `aa-logprof`, or set it to complain mode (`aa-complain <profile>`) to test.

### avc: denied { <perm> } for pid=<pid> comm="<name>"
Means: SELinux blocked a process (see the scontext and tcontext fields for who and what).
Fix: `sudo ausearch -m avc -ts recent | audit2why`. Usually a wrong file label: `restorecon -Rv <path>`, or for containers use `:Z` on volumes. See [SELinux](kb:linux/selinux).

### inotify_add_watch: No space left on device
Means: The user hit the inotify watch limit (not disk space). File sync tools, IDEs and Plex/Jellyfin watching large libraries hit it.
Fix: `sudo sysctl fs.inotify.max_user_watches=524288` and persist it in /etc/sysctl.d/.

### Too many open files
Means: EMFILE/ENFILE: the process (or the whole system) reached its open-file limit.
Fix: For services raise `LimitNOFILE=` in the unit; for containers `ulimits: nofile:`. Check for leaks: `ls /proc/<pid>/fd | wc -l` over time.

### VFS: file-max limit <n> reached
Means: The whole system reached `fs.file-max` open files.
Fix: Find the process holding most (`lsof | awk '{print $1}' | sort | uniq -c | sort -rn | head`), and raise `fs.file-max` if the load is legitimate.

### kernel: zram: Can't change algorithm for initialized device
Means: You tried to reconfigure a zram device that's already in use as swap.
Fix: `swapoff /dev/zram0`, reset it (`echo 1 > /sys/block/zram0/reset`), then configure. zram-generator handles this at boot.

### kernel: Unable to handle kernel paging request at virtual address <addr>
Means: The ARM equivalent of a kernel page fault: a driver bug or memory corruption (on Raspberry Pi often power or SD trouble).
Fix: Check power (under-voltage), update the firmware/kernel, try another SD card or boot from SSD.

### Process <name> (<pid>) of user <uid> dumped core.
Means: systemd-coredump recorded a crash of that process.
Fix: `coredumpctl info <pid>` shows the signal and backtrace. Look at the program's log just before the crash.

### PAM: pam_limits(<service>): Could not set limit for '<item>'
Means: A limit in /etc/security/limits.conf is above the system's hard maximum, so it couldn't be applied.
Fix: Lower the value or raise the system-wide maximum (for nofile: `fs.nr_open`).

### setrlimit(RLIMIT_CORE): Operation not permitted
Means: The program tried to raise its core dump limit without permission, common in containers. Usually harmless (sudo prints it in some containers).
Fix: Ignore in containers; or set the limit from outside (systemd `LimitCORE=`, docker `ulimits`).

### sysctl: cannot stat /proc/sys/<key>: No such file or directory
Means: That sysctl key doesn't exist on this kernel (a typo, a module not loaded, or a key from another kernel version).
Fix: `sysctl -a | grep <part>` to find the real name. For `net.bridge.*` keys load `br_netfilter` first.

### sysctl: permission denied on key '<key>'
Means: You need root, or (in containers) that key isn't namespaced and can't be changed from inside.
Fix: Use sudo on the host. In Docker, only namespaced keys can be set with `sysctls:` in compose.

### kill: (<pid>): No such process
Means: That process already exited (or the PID is wrong).
Fix: `pgrep -a <name>` to find the current PID.

### kill: (<pid>): Operation not permitted
Means: You can only signal your own processes unless you're root.
Fix: Use `sudo kill <pid>`. If even root fails, it's in another PID namespace or a zombie.

### <defunct>
Means: A zombie process: it has exited but its parent hasn't collected its status. It uses no resources besides a PID.
Fix: Restart or fix the parent (`ps -o ppid= -p <pid>`). Killing the zombie itself does nothing. In containers, use an init (`init: true` in compose).

### load average: <n>, <n>, <n>
Means: Not an error: the number of tasks running or waiting (including waiting on disk I/O) averaged over 1, 5 and 15 minutes.
Fix: Compare with the CPU count (`nproc`). High load with idle CPU means processes stuck on I/O: check `iostat -x 1` and the `wa` column in top.

### CPU steal time is high
Means: In a VM, the hypervisor is giving this VM's CPU time to others (the `st` column in top).
Fix: Reduce overcommit on the host, give the VM fewer but dedicated cores, or move it.

### sched: RT throttling activated
Means: Real-time priority tasks used up their CPU budget, and the kernel throttled them to keep the system responsive.
Fix: A real-time task is spinning: find it with `ps -eo pid,cls,rtprio,comm | grep -v TS`.
