# Errors: systemd, journald and boot

Messages from systemctl, journalctl, unit files, timers and the boot process. See also [Write your own systemd service](kb:linux/systemd-units), [Journald](kb:linux/journald) and [Boot rescue](kb:linux/boot-rescue).

### Failed to start <unit>.
Means: The unit's start failed. The reason is in the lines just before this one in the journal.
Fix: `systemctl status <unit>` and `journalctl -u <unit> -b -n 50`. Look for the program's own error or a status=2xx code.

### Job for <unit> failed because the control process exited with error code.
Means: The ExecStart (or ExecStartPre) command exited with a non-zero code.
Fix: `journalctl -xeu <unit>` shows the program's output. Run the ExecStart command by hand as the unit's User= to see the error directly.

### Job for <unit> failed because a timeout was exceeded.
Means: The service didn't signal it had started (or stopped) within TimeoutStartSec.
Fix: For Type=forking the PIDFile may be wrong; for Type=notify the program must send READY=1. Check what it waits on (network, database). Raise TimeoutStartSec only once you know why.

### Job for <unit> failed because the service did not take the steps required by its unit configuration.
Means: The unit type doesn't match the program: for example Type=forking but the program stays in the foreground, or a PIDFile that never appears.
Fix: Most modern programs run in the foreground: use Type=simple or Type=exec, and drop PIDFile.

### Job for <unit> failed because start of the service was attempted too often.
Means: The unit crashed repeatedly and hit StartLimitBurst within StartLimitIntervalSec, so systemd stopped retrying.
Fix: Fix the underlying error first (`journalctl -u <unit> -b`), then `systemctl reset-failed <unit>` and start again.

### Start request repeated too quickly.
Means: Same start-limit condition: too many restarts in a short time.
Fix: Find the first failure in the journal, fix it, then `systemctl reset-failed <unit>`.

### <unit>: Failed with result 'exit-code'.
Means: The main process exited with a non-zero code.
Fix: `journalctl -u <unit> -n 50` for the program's own error message; the "status=" line shows the code.

### <unit>: Failed with result 'signal'.
Means: The main process was killed by a signal (often SIGKILL from the OOM killer, or SIGSEGV from a crash).
Fix: The status line shows which signal. For SIGKILL check `dmesg | grep -i oom`; for SIGSEGV `coredumpctl info`.

### <unit>: Failed with result 'core-dump'.
Means: The process crashed and dumped core.
Fix: `coredumpctl info <unit-or-pid>`. Try the previous version of the program.

### <unit>: Failed with result 'oom-kill'.
Means: The unit (or a process in it) was killed by the OOM killer, possibly due to its own MemoryMax.
Fix: `systemctl show -p MemoryMax,MemoryPeak <unit>`. Raise the limit or reduce memory use.

### <unit>: Failed with result 'timeout'.
Means: Start, stop or watchdog timeout exceeded.
Fix: See the "a timeout was exceeded" entry. For WatchdogSec, the program stopped pinging systemd (hung).

### <unit>: Failed with result 'watchdog'.
Means: The service has WatchdogSec set and stopped sending keep-alive pings, so systemd killed it as hung.
Fix: Look at what it was doing before (journal). It may be overloaded or deadlocked.

### <unit>: Failed with result 'resources'.
Means: systemd couldn't set up resources for the unit (a missing PIDFile, a failed mount namespace, a missing directory).
Fix: The lines before it name the resource. Check paths in ReadWritePaths=, WorkingDirectory= and PIDFile=.

### <unit>: Main process exited, code=exited, status=<n>/<NAME>
Means: The program exited with that code. Codes 200-245 are systemd's own setup failures (the program never ran); others come from the program.
Fix: Look up the code: 203/EXEC = can't execute ExecStart, 217/USER = user missing, 226/NAMESPACE = sandbox path missing, 1 = program error (read its log).

### status=203/EXEC
Means: systemd couldn't execute the ExecStart program: wrong path, not executable, a script without #!, a missing interpreter, or SELinux blocking it.
Fix: ExecStart needs an absolute path. `ls -l` it, check the #! line, `chmod +x`. On SELinux systems files in /home are not allowed: move the binary to /usr/local/bin and `restorecon` it.

### status=200/CHDIR
Means: WorkingDirectory= doesn't exist or the unit's user can't enter it.
Fix: Create the directory or fix permissions; a `-` prefix (`WorkingDirectory=-/path`) makes it optional.

### status=217/USER
Means: The User= in the unit doesn't exist.
Fix: Create the user (`useradd --system <name>`) or fix the name; DynamicUser=yes avoids needing one.

### status=216/GROUP
Means: The Group= in the unit doesn't exist.
Fix: Create the group or correct the name.

### status=226/NAMESPACE
Means: A sandbox setting failed: a path in ReadWritePaths=, ReadOnlyPaths=, BindPaths= or similar doesn't exist.
Fix: Create the paths or prefix optional ones with `-`. `systemd-analyze security <unit>` shows the sandbox settings.

### status=209/STDOUT
Means: The StandardOutput= target couldn't be set up (for example a file in a directory that doesn't exist).
Fix: Create the directory, or use `StandardOutput=journal`.

### status=210/CHROOT
Means: RootDirectory= or RootImage= couldn't be used.
Fix: Check the path exists and holds a usable root tree.

### status=218/CAPABILITIES
Means: Capability settings (CapabilityBoundingSet=, AmbientCapabilities=) couldn't be applied, often in a container.
Fix: Remove unsupported capabilities; in LXC check the container's allowed capabilities.

### status=219/CGROUP
Means: systemd couldn't create or configure the unit's cgroup (often a delegation or cgroup v1/v2 mismatch in containers).
Fix: Check `mount | grep cgroup`; in containers, enable nesting/delegation.

### status=243/CREDENTIALS
Means: LoadCredential= or SetCredential= failed: the credential file doesn't exist or can't be read.
Fix: Check the path and its permissions (root must read it).

### status=1/FAILURE
Means: The program exited with code 1, a generic error it chose itself.
Fix: Its own log message just before tells you why: `journalctl -u <unit> -n 50`.

### code=killed, status=9/KILL
Means: The process got SIGKILL: the OOM killer, a memory limit, a stop timeout (systemd escalates from SIGTERM), or `kill -9`.
Fix: `dmesg -T | grep -i oom`; also check "State 'stop-sigterm' timed out" lines for stop timeouts.

### code=killed, status=6/ABRT
Means: The program aborted itself (an assertion or a detected corruption).
Fix: Read the last lines it logged; `coredumpctl info`.

### code=killed, status=11/SEGV
Means: The program crashed on an invalid memory access.
Fix: `coredumpctl info`; try another version; test RAM if many programs crash.

### code=dumped, status=<n>/<NAME>
Means: The process was killed by a signal and a core dump was saved.
Fix: `coredumpctl list` then `coredumpctl info <pid>`.

### State 'stop-sigterm' timed out. Killing.
Means: The service didn't exit within TimeoutStopSec after SIGTERM, so systemd sent SIGKILL.
Fix: The program ignores SIGTERM or shuts down slowly. Set KillSignal= to what it expects (SIGINT, SIGQUIT), or raise TimeoutStopSec.

### Unit <unit> not found.
Means: No unit file with that name is loaded.
Fix: Check the exact name with `systemctl list-unit-files | grep <part>`. After adding a unit file, run `systemctl daemon-reload`.

### Unit <unit> could not be found.
Means: Same as "not found": no such unit loaded.
Fix: `systemctl list-unit-files`; mind the suffix (.service, .timer, .mount).

### Failed to enable unit: Unit file <unit> does not exist.
Means: There's no unit file with that name in the unit search paths.
Fix: Put it in /etc/systemd/system/ (or link it with `systemctl link`), then daemon-reload.

### Unit <unit> is masked.
Means: The unit is linked to /dev/null so nothing can start it.
Fix: `systemctl unmask <unit>` if you want it.

### Failed to start <unit>: Unit <unit> has a bad unit file setting.
Means: systemd couldn't parse a required setting.
Fix: `systemd-analyze verify <path-to-unit>` lists the problems. Also check `journalctl -b | grep <unit>` for "Unknown key" or "Invalid" lines.

### <file>:<n>: Unknown key name '<key>' in section '<section>', ignoring.
Means: A setting is misspelled, in the wrong section, or too new for this systemd version.
Fix: Check spelling and section ([Unit], [Service], [Install]). `systemctl --version` and `man systemd.directives` show what's supported.

### <file>:<n>: Executable path is not absolute, ignoring: <cmd>
Means: Older systemd versions require ExecStart= to start with an absolute path.
Fix: Use the full path: `ExecStart=/usr/bin/<prog>`.

### <file>:<n>: Neither a valid executable name nor an absolute path: <cmd>
Means: The ExecStart value isn't usable (for example it starts with a shell construct or a variable).
Fix: Use an absolute path. For shell features use `ExecStart=/bin/sh -c '...'`.

### Service has more than one ExecStart= setting, which is only allowed for Type=oneshot services. Refusing.
Means: A drop-in added a second ExecStart= without clearing the first.
Fix: In the drop-in, write an empty `ExecStart=` line first, then the new one.

### Warning: The unit file, source configuration file or drop-ins of <unit> changed on disk. Run 'systemctl daemon-reload' to reload units.
Means: You edited the unit; systemd still uses the old version.
Fix: `sudo systemctl daemon-reload`, then restart the unit.

### Failed to connect to bus: No such file or directory
Means: systemctl couldn't reach systemd's D-Bus: you're in a container without systemd, a chroot, or `systemctl --user` without a user session.
Fix: In a chroot use `systemctl --root`. For `--user` units over SSH/su, log in properly or set `XDG_RUNTIME_DIR=/run/user/$(id -u)`; `loginctl enable-linger <user>` keeps the user manager running.

### System has not been booted with systemd as init system (PID 1). Can't operate.
Means: You're in a container, WSL without systemd, or a chroot, where PID 1 isn't systemd.
Fix: Start the program directly, use the container's own process manager, or enable systemd (WSL: `systemd=true` in /etc/wsl.conf).

### Failed to connect to bus: $DBUS_SESSION_BUS_ADDRESS and $XDG_RUNTIME_DIR not defined
Means: `systemctl --user` was run without a user session environment (often after `sudo -u` or `su`).
Fix: Use `machinectl shell <user>@` or `sudo -u <user> XDG_RUNTIME_DIR=/run/user/<uid> systemctl --user ...`.

### Interactive authentication required.
Means: polkit needs a password for that systemctl action and there's no agent to ask for it (e.g., over SSH without sudo).
Fix: Use `sudo systemctl ...`.

### Access denied
Means: A systemctl or loginctl action was refused by polkit or because you aren't root.
Fix: Use sudo, or add a polkit rule for that user and action.

### Failed to get D-Bus connection: Operation not permitted
Means: Usually systemctl inside an unprivileged container that has no systemd running.
Fix: Run services directly in the container, or use a systemd-enabled container image with the right privileges.

### Transaction for <unit> is destructive (<unit> has 'stop' job queued, but 'start' is included in transaction).
Means: The requested start conflicts with a pending stop (often during shutdown or with Conflicts= in units).
Fix: Wait for the stop to finish, or check Conflicts= between the units.

### Found ordering cycle on <unit>/start
Means: Units' After=/Before= relations form a loop, so systemd removed one job to break it.
Fix: The following lines list the cycle. Remove the After=/Requires= that doesn't belong; often a mount or network-online dependency.

### Job <unit>/start deleted to break ordering cycle starting with <unit>/start
Means: systemd skipped starting a unit to break a dependency cycle.
Fix: Fix the cycle (see "Found ordering cycle"); the skipped unit is listed here.

### A start job is running for <unit> (<n>s / <n>)
Means: Boot is waiting for that unit to finish starting, up to its timeout.
Fix: For disks: wrong fstab UUID (add `nofail`). For network-online.target: a NIC without a link; disable `systemd-networkd-wait-online` or set `--any`.

### A stop job is running for <unit> (<n>s / <n>)
Means: Shutdown waits for that unit to stop, by default up to 90 s.
Fix: Find why it hangs (often a container, an NFS mount whose network went first, or a user session). Lower `DefaultTimeoutStopSec` only as a workaround.

### Failed to start Wait for Network to be Configured.
Means: systemd-networkd-wait-online timed out because some managed interface never came up.
Fix: Mark unused interfaces `optional: true` in netplan, or run wait-online with `--any`. Safe to disable if nothing needs it.

### Timed out waiting for device <dev>.
Means: A device named in fstab, crypttab or a unit didn't appear.
Fix: Correct the UUID (`blkid`) or add `nofail` for optional devices.

### Reached target Emergency Mode.
Means: Boot couldn't reach normal mode, usually a failed fstab mount or fsck. You're in a root shell.
Fix: `journalctl -xb` shows what failed. Fix /etc/fstab (comment out the bad line), then `systemctl default` or reboot.

### You are in emergency mode. After logging in, type "journalctl -xb" to view system logs
Means: Same: boot fell into emergency mode.
Fix: Log in as root, `journalctl -xb | grep -i fail`, fix the mount or filesystem, reboot.

### Cannot open access to console, the root account is locked.
Means: Emergency mode needs the root password, but root has no password (Ubuntu default).
Fix: Reboot, edit the GRUB entry, add `init=/bin/bash` or `systemd.unit=rescue.target rd.break` to get a shell. See [Reset root password](kb:linux/reset-root-password).

### Failed to mount <mount>.
Means: A mount unit (from fstab or a .mount file) failed.
Fix: `systemctl status <name>.mount` and `journalctl -b -u <name>.mount`. Test with `mount -a` after fixing fstab.

### Failed to activate swap <dev>.
Means: A swap entry in fstab couldn't be enabled (missing device, unformatted, or a file with holes).
Fix: Check the UUID with `blkid`, run `mkswap`, or remove the stale entry.

### Failed to set up mount unit: Invalid argument
Means: A mount unit name doesn't match its Where= path.
Fix: Name it with `systemd-escape -p --suffix=mount /the/path`.

### <unit>: Scheduled restart job, restart counter is at <n>.
Means: The service exited and Restart= is restarting it: a crash loop if the counter keeps climbing.
Fix: `journalctl -u <unit> -n 100` shows why each start fails.

### <timer>: Timer unit lacks value setting. Refusing.
Means: A .timer unit has no OnCalendar=, OnBootSec= or similar trigger.
Fix: Add a trigger line under [Timer]. Nightgrid's System → JOBS editor builds timers for you.

### Failed to parse calendar specification, ignoring: <spec>
Means: OnCalendar= has a value systemd can't parse.
Fix: Test it with `systemd-analyze calendar '<spec>'`. Example: `Mon..Fri *-*-* 03:00:00`.

### <unit>: Refusing to start, unit <unit> to trigger not loaded.
Means: The timer or path unit points at a service that doesn't exist (a name mismatch).
Fix: Name them the same (foo.timer → foo.service) or set `Unit=` in the timer.

### journalctl: No journal files were found.
Means: The journal is empty: journald stores in memory only (/run) and nothing has logged, or the journal directory was deleted.
Fix: Create /var/log/journal and restart journald for persistence. See [Journald](kb:linux/journald).

### Hint: You are currently not seeing messages from other users and the system.
Means: Your user isn't in a group allowed to read the whole journal, so you only see your own entries.
Fix: Use sudo, or add yourself to `systemd-journal` (or `adm` on Debian/Ubuntu) and log in again.

### Journal file <file> is truncated, ignoring file.
Means: A journal file was cut short (crash or full disk).
Fix: Harmless; journald starts a new file. Clean up with `journalctl --vacuum-time=...` if space is short.

### File <file> corrupted or uncleanly shut down, renaming and replacing.
Means: journald found a journal file not closed properly (power loss or crash) and set it aside.
Fix: Nothing to do; the old file stays readable. Check why the system went down uncleanly.

### Failed to rotate <file>: Read-only file system
Means: journald can't write its files because /var is read-only.
Fix: Find why the filesystem is read-only (disk errors in `dmesg`).

### systemd-logind: Failed to start user slice user-<uid>.slice
Means: logind couldn't create the user's session slice, often a cgroup problem or a full /run.
Fix: `df -h /run`; check `journalctl -b -u systemd-logind`.

### pam_systemd(sshd:session): Failed to create session: <error>
Means: SSH login worked but systemd couldn't register the session (logind not running, or D-Bus problems).
Fix: `systemctl status systemd-logind dbus`. Restarting dbus on a live system can break things; reboot if they're wedged.

### systemd-tmpfiles: <file>:<n>: Duplicate line for path "<path>", ignoring.
Means: Two tmpfiles.d snippets configure the same path. Harmless.
Fix: Remove the duplicate if it's yours; otherwise ignore.

### systemd-resolved: Using degraded feature set UDP instead of UDP+EDNS0 for DNS server <ip>.
Means: The DNS server didn't handle EDNS0 properly, so resolved fell back to plain UDP.
Fix: Usually harmless. If lookups fail, try another DNS server or set `DNSSEC=no` in resolved.conf.

### systemd-resolved: DNSSEC validation failed for question <name>: no-signature
Means: DNSSEC validation is on and the answer wasn't signed as expected (often a router or local DNS stripping signatures).
Fix: Set `DNSSEC=allow-downgrade` or `no` in /etc/systemd/resolved.conf, then restart systemd-resolved.

### systemd-timesyncd: Timed out waiting for reply from <server>.
Means: The NTP server didn't answer (UDP 123 blocked, no network yet, or a dead server).
Fix: `timedatectl timesync-status`; check outbound UDP 123; set other servers in timesyncd.conf. See [Time sync](kb:linux/time-sync).

### systemd-udevd: <dev>: Worker [<pid>] processing SEQNUM=<n> is taking a long time
Means: A udev rule or helper is slow or hung, delaying device setup.
Fix: Find the rule (`udevadm test`). Often a RUN+= script waiting on network or a slow USB device.

### systemd[1]: <unit>: Consumed <n> CPU time.
Means: Informational: the CPU used by the unit during its run.
Fix: Nothing to do.

### systemd-coredump[<pid>]: Process <pid> (<name>) of user <uid> dumped core.
Means: A process crashed; its core dump is stored.
Fix: `coredumpctl info <pid>`; journald also has the backtrace.

### Startup finished in <n>s (kernel) + <n>s (userspace)
Means: Informational: boot time.
Fix: To speed up, see `systemd-analyze blame` and `systemd-analyze critical-chain`.

### Failed to execute <cmd>: No such file or directory
Means: An ExecStart*/ExecStop* program doesn't exist (often after uninstalling or moving it).
Fix: Fix the path in the unit; `systemctl cat <unit>` shows all drop-ins.

### Failed to determine user credentials: No such process
Means: The User= in the unit can't be resolved (deleted, or an LDAP/SSSD user unavailable at boot).
Fix: Create the user locally, or order the unit after `nss-user-lookup.target`.

### Failed to set up special execution directory in /var/lib: Permission denied
Means: StateDirectory= couldn't be created or chowned, often because of SELinux labels or a read-only /var/lib.
Fix: Check `ls -ldZ /var/lib/<dir>`; `restorecon -Rv /var/lib/<dir>`.

### Failed to drop capabilities: Operation not permitted
Means: The service manager (often inside a container) couldn't change the process's capabilities.
Fix: Relax CapabilityBoundingSet= or run with enough privileges in the container.

### Failed to load environment files: No such file or directory
Means: EnvironmentFile= points at a file that doesn't exist.
Fix: Create it, or prefix with `-` to make it optional: `EnvironmentFile=-/etc/default/<name>`.

### Ignoring invalid environment assignment '<line>'
Means: A line in an EnvironmentFile= isn't `KEY=value` (spaces around `=`, `export`, or shell syntax).
Fix: Use plain `KEY=value` lines; no `export`, no command substitution.

### <unit>.service: Supervising process <pid> which is not our child. We'll most likely not notice when it exits.
Means: Type=forking with a PIDFile that points at a process systemd didn't start (or the wrong PID).
Fix: Run the program in the foreground with Type=simple if it supports that.

### New main PID <pid> does not belong to service, and PID file is not owned by root. Refusing.
Means: systemd distrusts a PID file writable by a non-root user, to avoid being tricked into killing other processes.
Fix: Prefer Type=simple/notify without a PIDFile; or keep the PID file in a root-owned directory (RuntimeDirectory=).

### <unit>: Got notification message from PID <pid>, but reception only permitted for main PID <pid>
Means: A child process sent sd_notify messages but the unit only accepts them from the main process.
Fix: Set `NotifyAccess=all` (or `exec`) in the unit.
