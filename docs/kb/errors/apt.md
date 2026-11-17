# Errors: apt and dpkg

Messages from apt, apt-get and dpkg on Debian, Ubuntu, Raspberry Pi OS and Proxmox VE. See also [Packages and updates](kb:linux/packages) and [Release upgrades](kb:linux/release-upgrade).

### E: Could not get lock /var/lib/dpkg/lock-frontend. It is held by process <pid> (<name>)
Means: Another apt or dpkg run holds the lock: unattended-upgrades, a desktop updater, or a second terminal.
Fix: Wait for it (`ps -fp <pid>`). Never delete lock files while that process lives. If the process is gone and the lock stays, reboot, then `sudo dpkg --configure -a`.

### E: Could not get lock /var/lib/dpkg/lock
Means: Older wording of the same lock: another package operation is running.
Fix: `ps aux | grep -E 'apt|dpkg'`; wait or let it finish.

### E: Unable to acquire the dpkg frontend lock (/var/lib/dpkg/lock-frontend), is another process using it?
Means: Same as "Could not get lock": another package manager is running.
Fix: Wait for unattended-upgrades to finish (`systemctl status unattended-upgrades`, `journalctl -u apt-daily-upgrade`).

### Waiting for cache lock: Could not get lock /var/lib/dpkg/lock-frontend
Means: Newer apt waits for the lock instead of failing.
Fix: Let it wait, or find the other process with `fuser -v /var/lib/dpkg/lock-frontend`.

### E: dpkg was interrupted, you must manually run 'sudo dpkg --configure -a' to correct the problem.
Means: A package install or upgrade stopped half-way (closed SSH session, power loss, full disk).
Fix: `sudo dpkg --configure -a`, then `sudo apt -f install`. Use tmux for long upgrades so a dropped connection can't interrupt them.

### E: Unable to locate package <pkg>
Means: No enabled repository has a package by that name: a typo, the package lists were never fetched, or it's in a component (universe, contrib, non-free) that isn't enabled.
Fix: `sudo apt update` first. `apt search <part>` to find the right name. On Ubuntu enable universe (`sudo add-apt-repository universe`); on Debian add `contrib non-free non-free-firmware`.

### E: Package '<pkg>' has no installation candidate
Means: The name is known (another package refers to it) but no repository offers a version for your release or architecture.
Fix: `apt-cache policy <pkg>`. It may be renamed, removed in this release, or only in backports.

### Package <pkg> is not available, but is referred to by another package.
Means: The package is obsolete or only reachable from a different source.
Fix: Look for the replacement named in the message, or the package's new name.

### E: Unable to correct problems, you have held broken packages.
Means: apt can't find a set of versions that satisfies every dependency: mixed releases, a PPA with mismatched versions, or held packages.
Fix: `apt-mark showhold`. Try `sudo apt install <pkg>` to see the exact conflict, or `aptitude install <pkg>` which proposes solutions. Remove PPAs meant for another release.

### The following packages have unmet dependencies:
Means: Installing would leave a package without something it depends on.
Fix: Read the lines below it: they name the missing or conflicting version. `sudo apt -f install` often fixes an interrupted state; mixed repositories need cleaning up.

### E: Unmet dependencies. Try 'apt --fix-broken install' with no packages (or specify a solution).
Means: The installed set is already inconsistent, usually after a failed install.
Fix: `sudo apt --fix-broken install`, read what it plans before saying yes.

### dpkg: dependency problems prevent configuration of <pkg>:
Means: dpkg unpacked the package but can't configure it until its dependencies are installed and configured.
Fix: `sudo apt -f install` to pull in what's missing.

### dpkg: error processing package <pkg> (--configure):
Means: The package's configure (postinst) script failed. The real reason is in the lines just above.
Fix: Scroll up. Common: a service that fails to start after the upgrade, or a config file with a syntax error. Fix that, then `sudo dpkg --configure -a`.

### installed <pkg> package post-installation script subprocess returned error exit status <n>
Means: The package's postinst script failed, often because the service it restarts won't start.
Fix: `systemctl status <service>` and its journal. Fix the cause, then `sudo dpkg --configure -a`.

### E: Sub-process /usr/bin/dpkg returned an error code (1)
Means: dpkg failed for one or more packages; apt just reports it.
Fix: Look for the "dpkg: error processing" lines above for the package and reason.

### dpkg: error processing archive <file> (--unpack):
Means: dpkg couldn't unpack the .deb: a file conflict with another package, a corrupt download, or a full disk.
Fix: Read the next line ("trying to overwrite …" or "No space left"). `sudo apt clean` and retry for corrupt downloads.

### trying to overwrite '<path>', which is also in package <pkg>
Means: Two packages ship the same file, usually a PPA or third-party repo clashing with the distro.
Fix: Remove one of the packages, or use packages built for your release. Forcing with `dpkg -i --force-overwrite` is a last resort.

### W: GPG error: <url> <release> InRelease: The following signatures couldn't be verified because the public key is not available: NO_PUBKEY <key>
Means: apt doesn't have the signing key for that repository, so it won't trust it.
Fix: Install the repo's key from its official instructions into /etc/apt/keyrings and reference it with `signed-by=` in the .list or .sources file. `apt-key adv` is deprecated.

### E: The repository '<url> <release> Release' is not signed.
Means: The repository has no valid signature, or apt couldn't download the Release file (wrong URL looks like this too).
Fix: Check the URL in /etc/apt/sources.list.d/ in a browser. Remove repos that don't exist for your release.

### E: The repository '<url> <release> Release' does not have a Release file.
Means: The repository has no files for your distro release: a PPA without builds for it, an EOL release moved to an archive, or a wrong codename.
Fix: Check `lsb_release -cs` against what the repo supports. For an end-of-life release, point at old-releases.ubuntu.com or archive.debian.org, or upgrade.

### W: An error occurred during the signature verification. The repository is not updated and the previous index files will be used.
Means: A signature check failed, often an expired key (EXPKEYSIG) or a captive portal or proxy returning HTML.
Fix: Read the line naming the key; refresh it from the vendor. Check `curl -I <url>` returns the repo, not a login page.

### EXPKEYSIG <key>
Means: The repository's signing key expired.
Fix: Download the current key from the vendor into /etc/apt/keyrings and run `sudo apt update`.

### W: Key is stored in legacy trusted.gpg keyring (/etc/apt/trusted.gpg), see the DEPRECATION section in apt-key(8) for details.
Means: A warning: the key works but lives in the old global keyring that trusts it for every repo.
Fix: Export it to /etc/apt/keyrings/<name>.gpg and add `signed-by=` to that repo's line. Then delete it from the old keyring.

### E: Release file for <url> is not valid yet (invalid for another <time>). Updates for this repository will not be applied.
Means: Your system clock is behind, so the repository's Release file looks like it's from the future.
Fix: Fix the clock: `timedatectl`, enable NTP (`timedatectl set-ntp true`). Raspberry Pis without an RTC often boot with an old date.

### E: Release file for <url> is expired (invalid since <time>). Updates for this repository will not be applied.
Means: The Release file's validity date passed: a stale mirror, or a snapshot/archive repository.
Fix: Use another mirror. For archive repos only, `-o Acquire::Check-Valid-Until=false`.

### W: Failed to fetch <url> Hash Sum mismatch
Means: A downloaded index didn't match its checksum: a mirror mid-sync or a caching proxy serving stale files.
Fix: `sudo rm -rf /var/lib/apt/lists/* && sudo apt update`. Switch mirrors if it persists.

### E: Failed to fetch <url> 404 Not Found
Means: The file isn't on the mirror: stale package lists, or a release that moved to the archive.
Fix: `sudo apt update` and try again. If it persists, your release may be end-of-life.

### Temporary failure resolving '<host>'
Means: apt couldn't resolve the mirror's name: DNS broken or no network.
Fix: `resolvectl status`, `ping -c1 1.1.1.1`, `getent hosts <host>`. See the DNS errors shelf.

### Could not connect to <host>:80 (<ip>). - connect (113: No route to host)
Means: No network path to the mirror: no default route, a firewall, or IPv6 without connectivity.
Fix: `ip route`; try forcing IPv4 with `-o Acquire::ForceIPv4=true`.

### Certificate verification failed: The certificate is NOT trusted.
Means: apt over HTTPS couldn't verify the mirror's certificate: missing ca-certificates, a wrong clock, or a TLS-intercepting proxy.
Fix: `sudo apt install ca-certificates` (via an http mirror if needed), check the clock, and add your proxy's CA to /usr/local/share/ca-certificates then `update-ca-certificates`.

### N: Skipping acquire of configured file '<component>' as repository '<url>' doesn't support architecture '<arch>'
Means: A notice: the repo doesn't publish packages for one of your configured architectures (often i386).
Fix: Add `[arch=amd64]` to that repo's line, or remove the foreign architecture with `dpkg --remove-architecture i386` if unused.

### W: Target <target> is configured multiple times in <file> and <file>
Means: The same repository is listed twice.
Fix: Remove the duplicate line or file in /etc/apt/sources.list.d/.

### N: Ignoring file '<file>' in directory '/etc/apt/sources.list.d/' as it has an invalid filename extension
Means: Files there must end in .list or .sources; others (like .save or .bak) are ignored.
Fix: Rename or delete the file.

### E: Malformed entry <n> in list file <file> (Component)
Means: A line in a sources file is broken.
Fix: Fix that line: `deb [signed-by=…] <url> <suite> <component>…`.

### E: Type '<word>' is not known on line <n> in source list <file>
Means: A sources line doesn't start with `deb` or `deb-src` (often HTML pasted in, or a typo).
Fix: Correct or remove that line.

### W: Some index files failed to download. They have been ignored, or old ones used instead.
Means: Summary: at least one repository failed during update.
Fix: The "Err:" or "W:" lines above say which and why.

### E: You don't have enough free space in /var/cache/apt/archives/.
Means: Not enough room to download the packages.
Fix: `sudo apt clean`, `sudo apt autoremove --purge`, clear journald and Docker; check `df -h /var`.

### gzip: stdout: No space left on device
Means: The disk (usually /boot) filled while building an initramfs during a kernel update.
Fix: `df -h /boot`; remove old kernels with `sudo apt autoremove --purge`, then `sudo apt -f install`.

### update-initramfs: failed for <path> with 1.
Means: Building the initramfs failed, very often because /boot is full.
Fix: Free /boot (remove old kernels), then `sudo update-initramfs -u -k all`. Don't reboot until it succeeds.

### E: Sub-process /usr/bin/dpkg returned an error code (2)
Means: dpkg itself failed hard, often a corrupted dpkg database or a full disk.
Fix: Check `df -h`. Look for the first error above; `sudo dpkg --audit` lists inconsistent packages.

### dpkg: warning: files list file for package '<pkg>' missing; assuming package has no files currently installed
Means: dpkg's record of a package's files is missing, usually after disk corruption or a hard power-off.
Fix: Reinstall the package: `sudo apt install --reinstall <pkg>`.

### dpkg: unrecoverable fatal error, aborting: files list file for package '<pkg>' is missing final newline
Means: A file in /var/lib/dpkg/info is corrupt (zero bytes after a crash).
Fix: Restore it from /var/backups/dpkg.status.* or recreate it by reinstalling: move the broken `<pkg>.list` away, then `sudo apt install --reinstall <pkg>`.

### E: Problem executing scripts APT::Update::Post-Invoke-Success
Means: A hook that runs after `apt update` failed (often command-not-found's database or a broken appstream).
Fix: The line above names the script. Reinstalling that package usually fixes it.

### E: The package <pkg> needs to be reinstalled, but I can't find an archive for it.
Means: dpkg marks a package as half-installed but no repository has that version any more.
Fix: `sudo dpkg --remove --force-remove-reinstreq <pkg>`, then install a current version.

### The following packages have been kept back:
Means: Not an error: apt upgrade won't install packages that need new dependencies or are being phased (Ubuntu's phased updates).
Fix: Usually wait (phased updates arrive later). To take them now: `sudo apt install <pkg>` or `sudo apt full-upgrade`.

### Configuration file '<file>' ==> Modified (by you or by a script) since installation.
Means: dpkg found you changed a config file that the new package also changes, and asks what to keep.
Fix: Press D to see the diff. Usually keep yours (N) and merge new options by hand from `<file>.dpkg-dist`.

### debconf: unable to initialize frontend: Dialog
Means: apt has no terminal for interactive prompts (in Docker builds or non-interactive SSH).
Fix: Harmless; set `DEBIAN_FRONTEND=noninteractive` for unattended installs.

### debconf: delaying package configuration, since apt-utils is not installed
Means: A harmless message in minimal images.
Fix: Ignore, or install apt-utils.

### invoke-rc.d: policy-rc.d denied execution of start.
Means: In containers, policy-rc.d stops packages from starting services during install. Expected.
Fix: Start the service yourself if the container should run it.

### E: Unable to fetch some archives, maybe run apt-get update or try with --fix-missing?
Means: Package downloads failed, usually because the package lists are stale.
Fix: `sudo apt update`, then retry.

### W: Download is performed unsandboxed as root as file '<file>' couldn't be accessed by user '_apt'. - pkgAcquire::Run (13: Permission denied)
Means: Installing a local .deb from a folder the _apt user can't read.
Fix: Harmless. Copy the .deb to /tmp first to silence it.

### E: Unsupported file <file> given on commandline
Means: `apt install` was given a .deb path without `./`, so apt read it as a package name.
Fix: Use `sudo apt install ./<file>.deb`.

### dpkg: error: dpkg frontend lock is locked by another process
Means: dpkg itself was run while apt holds the lock.
Fix: Wait for the other process; use apt for installs.

### dpkg-deb: error: '<file>' is not a Debian format archive
Means: The .deb is incomplete or not a .deb at all (an HTML page from a failed download).
Fix: `file <file>`; download it again from the right link.

### package architecture (<arch>) does not match system (<arch>)
Means: The .deb is for another CPU architecture.
Fix: Get the build for `dpkg --print-architecture`.

### E: Conflicting values set for option Signed-By regarding source <url>
Means: The same repository is listed twice with different signed-by keys.
Fix: Keep one entry, in one file.

### Error: Timeout was reached
Means: Software tools (add-apt-repository, snap) couldn't reach Launchpad or the store in time.
Fix: Check network and proxy; retry later.

### unattended-upgrades: Lock could not be acquired (another package manager running?)
Means: unattended-upgrades skipped a run because something else held the lock.
Fix: Harmless once; recurring means a stuck process.

### needrestart: Pending kernel upgrade!
Means: A newer kernel is installed than the one running.
Fix: Reboot when convenient. Nightgrid's Fleet → **Updates** shows reboot-required per host.

### *** System restart required ***
Means: An update (kernel, libc, firmware) needs a reboot; /var/run/reboot-required exists.
Fix: Reboot at a quiet time. `cat /var/run/reboot-required.pkgs` lists why.

### pveversion: command not found
Means: Proxmox tools are missing: not a Proxmox host, or a broken upgrade removed them.
Fix: On Proxmox, check `apt policy proxmox-ve`, the Proxmox repos, and reinstall `proxmox-ve`.

### 401 Unauthorized [IP: <ip> 443]
Means: apt was refused by a repository that needs a subscription (Proxmox enterprise repo, Ubuntu Pro ESM).
Fix: On Proxmox without a subscription, disable the enterprise repo and add the no-subscription one. For Ubuntu ESM attach Pro or disable the esm sources.

### E: Failed to fetch https://enterprise.proxmox.com/<path> 401 Unauthorized
Means: The Proxmox enterprise repository needs a subscription key.
Fix: Comment it out in /etc/apt/sources.list.d/pve-enterprise.list (and ceph.list), add `pve-no-subscription`, then `apt update`.

### E: Repository '<url>' changed its 'Suite' value from '<a>' to '<b>'
Means: The repo moved to a new suite name (often after a release, stable → oldstable).
Fix: `sudo apt update --allow-releaseinfo-change` after confirming it's expected. Plan a release upgrade.
