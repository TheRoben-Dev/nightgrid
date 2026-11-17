# Errors: dnf, rpm, rpm-ostree and other package managers

Messages from dnf/yum (Fedora, RHEL, Rocky, Alma), rpm, rpm-ostree (Fedora Atomic, Bazzite), pacman, zypper and apk. See also [rpm-ostree](kb:linux/rpm-ostree).

### Error: Unable to find a match: <pkg>
Means: dnf found no package by that name in the enabled repositories: a typo, or it's in EPEL, RPM Fusion or CRB.
Fix: `dnf search <part>`, `dnf provides '*/<file>'`. On RHEL clones enable EPEL (`dnf install epel-release`) and CRB (`dnf config-manager --set-enabled crb`).

### No match for argument: <pkg>
Means: Same: the package isn't in any enabled repo.
Fix: Check the name and the repos (`dnf repolist`).

### Error: Failed to download metadata for repo '<repo>': Cannot download repomd.xml
Means: dnf couldn't fetch the repository index: no network, DNS, a dead mirror, or a repo URL for another release.
Fix: `curl -I` the baseurl from /etc/yum.repos.d/<repo>.repo. Disable the repo temporarily with `--disablerepo=<repo>` to get on.

### Curl error (6): Couldn't resolve host name for <url>
Means: DNS lookup for the mirror failed.
Fix: Check `/etc/resolv.conf` / `resolvectl` and the network.

### Curl error (28): Timeout was reached for <url>
Means: The mirror didn't answer in time.
Fix: Retry; set `fastestmirror=True` in /etc/dnf/dnf.conf, or check firewall/proxy.

### Error: GPG check FAILED
Means: A package's signature didn't verify against the imported keys.
Fix: Import the repo's key (`rpm --import <key-url>`) from the vendor; don't disable gpgcheck for real use.

### Public key for <file> is not installed
Means: The repo's GPG key isn't imported yet.
Fix: Confirm the prompt if you trust the repo, or `rpm --import` the key first.

### The GPG keys listed for the "<repo>" repository are already installed but they are not correct for this package.
Means: The package was signed with a different key than the one configured, often after a vendor rotated keys.
Fix: Import the vendor's new key; check the repo file's `gpgkey=` URL.

### Error: Transaction test error: file <path> from install of <pkg> conflicts with file from package <pkg>
Means: Two packages ship the same file.
Fix: Remove one, or use packages from a single matching repo. `--allowerasing` lets dnf swap them.

### Error: Problem: package <pkg> requires <dep>, but none of the providers can be installed
Means: A dependency can't be satisfied: mixed releases, a missing repo, or a version excluded.
Fix: Read the rest of the "Problem:" block. Try `dnf install <pkg> --allowerasing` or `--skip-broken` to see alternatives.

### Error: Problem: conflicting requests
Means: The requested packages can't all be installed together.
Fix: The following lines list which. Install them separately or pick one.

### (try to add '--skip-broken' to skip uninstallable packages or '--nobest' to use not only best candidate packages)
Means: dnf's hint after a dependency problem.
Fix: Read the Problem lines first; `--nobest` accepts an older version that resolves.

### Error: Cannot open: <file>. Skipping.
Means: A local rpm path doesn't exist or isn't readable.
Fix: Check the path.

### Waiting for process with pid <pid> to finish.
Means: Another dnf or PackageKit holds the lock.
Fix: Wait, or check what it is: `ps -fp <pid>`.

### Error: This command has to be run with superuser privileges (under the root user on most systems).
Means: dnf install/remove needs root.
Fix: Use sudo.

### error: rpmdb: BDB0113 Thread/process <n> failed: BDB1507 Thread died in Berkeley DB library
Means: The RPM database is corrupt, usually after a crash during a transaction (older Berkeley DB rpmdb).
Fix: `sudo rpm --rebuilddb`. Back up /var/lib/rpm first.

### error: rpmdb open failed
Means: rpm can't open its database (corruption or wrong permissions).
Fix: `sudo rpm --rebuilddb`.

### warning: <file>: Header V4 RSA/SHA256 Signature, key ID <key>: NOKEY
Means: rpm installed or inspected a package signed by a key that isn't imported.
Fix: Import the vendor's key with `rpm --import`.

### error: Failed dependencies:
Means: `rpm -i` doesn't resolve dependencies; it lists what's missing below.
Fix: Install local rpms with `dnf install ./<file>.rpm`, which pulls dependencies.

### package <pkg> is already installed
Means: rpm refused because the same version is installed.
Fix: Use `dnf reinstall <pkg>` if you meant to repair it.

### Error: Transaction check error: installing package <pkg> needs <n>MB more space on the / filesystem
Means: Not enough disk space for the transaction.
Fix: Free space (`dnf clean all`, old kernels: `dnf remove --oldinstallonly`), check `df -h /`.

### Last metadata expiration check: <time> ago
Means: Informational: when repo metadata was last refreshed.
Fix: Nothing; `dnf makecache --refresh` to refresh now.

### Error: Could not resolve host: mirrors.fedoraproject.org
Means: No DNS or network.
Fix: Network/DNS first.

### Status code: 404 for <url>
Means: The repository path doesn't exist, often a third-party repo without builds for your Fedora version (common right after a Fedora release).
Fix: Disable that repo until it publishes, or point it at the previous release.

### error: Could not depsolve transaction; <n> problem detected:
Means: rpm-ostree couldn't resolve the layered packages against the new base image.
Fix: Read the problem; usually a layered package not yet built for the new release. Remove it (`rpm-ostree uninstall <pkg>`) or wait.

### error: Packages not found: <pkg>
Means: rpm-ostree found no such package to layer.
Fix: Check the name with `dnf search` in a toolbox, or that the repo is enabled in /etc/yum.repos.d/.

### error: Transaction in progress: <op>
Means: Another rpm-ostree operation is running (often the automatic update).
Fix: `rpm-ostree status`; wait or `rpm-ostree cancel`.

### error: Running in a container; rpm-ostree only works on hosts
Means: You ran rpm-ostree inside a toolbox or distrobox.
Fix: Exit the container and run it on the host.

### error: Unable to find a mutable lock on this deployment
Means: You tried to change files under /usr on an immutable system.
Fix: Layer packages with rpm-ostree, use Flatpak or a toolbox, or `rpm-ostree usroverlay` for a temporary writable /usr.

### error: Package/capability '<pkg>' is already provided by a package: <pkg>
Means: That package (or something providing it) is already in the base image.
Fix: Nothing to layer; it's there.

### error: failed to commit transaction (conflicting files)
Means: pacman: a file the package wants to install already exists, owned by another package or by none.
Fix: Read which file. If it's unowned leftover, move it away; use `--overwrite <glob>` only when you understand why.

### error: target not found: <pkg>
Means: pacman found no package by that name.
Fix: `pacman -Syu` first, `pacman -Ss <part>`, or it's in the AUR.

### error: <pkg>: signature from "<packager>" is unknown trust
Means: pacman's keyring doesn't trust the packager's key, usually an outdated archlinux-keyring.
Fix: `sudo pacman -Sy archlinux-keyring && sudo pacman -Su`.

### error: failed to synchronize all databases (unable to lock database)
Means: pacman's lock file exists: another pacman is running, or one crashed.
Fix: If no pacman is running, `sudo rm /var/lib/pacman/db.lck`.

### warning: <path> installed as <path>.pacnew
Means: A config file you changed has a new version from the package.
Fix: Merge them (`pacdiff`), then delete the .pacnew.

### ERROR: unable to select packages:
Means: apk (Alpine) can't find or satisfy a package, often because the community repo isn't enabled.
Fix: Enable community in /etc/apk/repositories, run `apk update`.

### WARNING: Ignoring <url>: No such file or directory
Means: apk couldn't fetch an index (no `apk update` yet, or a wrong mirror).
Fix: `apk update`.

### Problem: nothing provides '<dep>' needed by the to be installed <pkg>
Means: zypper (openSUSE) can't satisfy a dependency.
Fix: Add the repo that provides it, or choose a solution zypper offers.

### System management is locked by the application with pid <pid> (<name>).
Means: zypper is locked by another package tool (often PackageKit).
Fix: Wait, or stop PackageKit (`systemctl stop packagekit`).

### error: snap "<name>" not found
Means: No snap by that name in the store.
Fix: `snap find <part>`.

### error: cannot communicate with server: Post "http://localhost/v2/snaps/<name>": dial unix /run/snapd.socket: connect: no such file or directory
Means: snapd isn't running (common in containers and minimal installs).
Fix: `sudo systemctl enable --now snapd.socket`. Snaps don't work inside most containers.

### error: This revision of snap "<name>" was published using classic confinement and thus may perform arbitrary system changes outside of the security sandbox
Means: The snap needs `--classic` to be installed.
Fix: Re-run with `--classic` if you trust it.

### error: Unable to load summary from remote flathub
Means: Flatpak couldn't fetch Flathub's index (network, DNS, or a stale summary).
Fix: Check the network; `flatpak remote-ls flathub` to retry; `flatpak repair` if local data is damaged.

### error: No remote refs found for '<name>'
Means: Flatpak found no app by that name in the configured remotes.
Fix: `flatpak search <part>`; add Flathub with `flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo`.

### error: <app> needs a later flatpak version
Means: The app requires a newer Flatpak than installed.
Fix: Update the system's flatpak package.
