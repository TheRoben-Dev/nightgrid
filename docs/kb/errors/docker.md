# Errors: Docker and Podman

Messages from the docker CLI, dockerd, containerd, image pulls and builds, and Podman. See also [Docker networks](kb:linux/docker-networks), [Docker cleanup](kb:linux/docker-cleanup) and Nightgrid's [Docker](view:docker) view.

### Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?
Means: The docker CLI can't reach dockerd: the service is stopped or failed, or DOCKER_HOST/context points elsewhere.
Fix: `sudo systemctl status docker` and `journalctl -u docker -n 50`. Check `docker context ls` and `echo $DOCKER_HOST`.

### permission denied while trying to connect to the Docker daemon socket at unix:///var/run/docker.sock
Means: Your user may not use the Docker socket.
Fix: Use sudo, or `sudo usermod -aG docker $USER` and log in again. The docker group is root-equivalent; rootless Docker or Podman avoids that.

### Got permission denied while trying to connect to the Docker daemon socket
Means: Same as above, older wording.
Fix: sudo or the docker group (then a new login).

### Error response from daemon: pull access denied for <image>, repository does not exist or may require 'docker login'
Means: The image name is wrong, the repository is private, or you're not logged in to that registry.
Fix: Check the exact name and namespace (ghcr.io/…, lscr.io/…); `docker login <registry>` for private ones.

### Error response from daemon: manifest for <image> not found: manifest unknown: manifest unknown
Means: That tag doesn't exist in the repository.
Fix: Check available tags on the registry page; many images have no `latest`.

### no matching manifest for linux/arm64/v8 in the manifest list entries
Means: The image has no build for your CPU architecture (common on Raspberry Pi and Apple Silicon).
Fix: Find an image that publishes arm64, or run amd64 under emulation (`platform: linux/amd64` plus qemu-user-static), which is slow.

### toomanyrequests: You have reached your pull rate limit. You may increase the limit by authenticating and upgrading
Means: Docker Hub's anonymous pull limit per IP (per 6 hours) was hit.
Fix: `docker login` (free accounts get more), use a pull-through cache, or images from ghcr.io/lscr.io.

### Error response from daemon: Get "https://registry-1.docker.io/v2/": net/http: request canceled while waiting for connection (Client.Timeout exceeded while awaiting headers)
Means: dockerd couldn't reach Docker Hub: no network, DNS, a proxy, or IPv6 issues.
Fix: Test `curl -I https://registry-1.docker.io/v2/` on the host; configure dockerd's proxy in a systemd drop-in if you use one.

### Error response from daemon: Get "<url>": dial tcp: lookup <host> on <ip>:53: no such host
Means: The registry name didn't resolve via the host's DNS.
Fix: Check the host's resolver; a DNS container on the same host that's down can cause this.

### Error response from daemon: Get "<url>": x509: certificate signed by unknown authority
Means: A private registry uses a certificate Docker doesn't trust.
Fix: Put its CA in /etc/docker/certs.d/<registry:port>/ca.crt, or list it under insecure-registries only for plain HTTP labs.

### Error response from daemon: Get "<url>": http: server gave HTTP response to HTTPS client
Means: The registry speaks plain HTTP but Docker expects HTTPS.
Fix: Add it to `insecure-registries` in /etc/docker/daemon.json (LAN only), or put TLS in front of it.

### Error response from daemon: Conflict. The container name "<name>" is already in use by container "<id>". You have to remove (or rename) that container to be able to reuse that name.
Means: A container (often a stopped one) already has that name.
Fix: `docker ps -a --filter name=<name>`; remove it if unneeded (`docker rm <name>`), or rename the new one.

### Error response from daemon: driver failed programming external connectivity on endpoint <name>: Bind for 0.0.0.0:<port> failed: port is already allocated
Means: Another container (or the same stack under an old project name) already publishes that host port.
Fix: `docker ps --format '{{.Names}} {{.Ports}}' | grep <port>`. Stop it or change the host side of the mapping.

### Error starting userland proxy: listen tcp4 0.0.0.0:<port>: bind: address already in use
Means: A host process (not a container) already listens on that port.
Fix: `sudo ss -ltnp 'sport = :<port>'` names it. Move one of them.

### Error response from daemon: failed to create task for container: failed to create shim task: OCI runtime create failed: runc create failed: unable to start container process: exec: "<cmd>": executable file not found in $PATH: unknown
Means: The image doesn't contain the command you (or the image's CMD) asked for.
Fix: Check the command and entrypoint; `docker run --rm -it --entrypoint sh <image>` to look inside (some minimal images have no shell).

### OCI runtime create failed: runc create failed: unable to start container process: error during container init: error mounting "<src>" to rootfs at "<dst>": <details>
Means: A volume or bind mount failed: the host path doesn't exist where a file was expected, a file is mounted over a directory (or the reverse), or SELinux blocked it.
Fix: Create the host path with the right type before starting (Docker creates a directory if it's missing). On SELinux hosts add `:Z`.

### not a directory: Are you trying to mount a directory onto a file (or vice-versa)?
Means: The bind mount types don't match: usually the host file didn't exist so Docker created a directory with that name.
Fix: Remove the accidental directory on the host, create the file, and start again.

### Error response from daemon: invalid mount config for type "bind": bind source path does not exist: <path>
Means: `--mount type=bind` doesn't create missing paths (unlike `-v`).
Fix: Create the path on the host, or fix a typo.

### Error response from daemon: No such container: <name>
Means: No container by that name or id (it was removed, or a compose project prefixes the name).
Fix: `docker ps -a` for the real name.

### Error response from daemon: No such image: <image>
Means: The image isn't on this host.
Fix: `docker pull <image>` or check the tag.

### Error: No such object: <name>
Means: `docker inspect` found nothing by that name.
Fix: Check `docker ps -a`, `docker volume ls`, `docker network ls`.

### Error response from daemon: Container <id> is not running
Means: `docker exec` needs a running container.
Fix: `docker start <name>` first, or look at why it exited: `docker logs <name>`.

### Error response from daemon: Container <id> is restarting, wait until the container is running
Means: The container is crash-looping (restart policy keeps restarting it).
Fix: `docker logs --tail 50 <name>` for the crash; `docker inspect -f '{{.State.ExitCode}}' <name>`.

### Error response from daemon: conflict: unable to delete <id> (cannot be forced) - image is being used by running container <id>
Means: A running container uses that image.
Fix: Stop and remove the container first.

### Error response from daemon: conflict: unable to remove repository reference "<image>" (must force) - container <id> is using its referenced image <id>
Means: A stopped container still references the image.
Fix: `docker container prune` (after checking) or remove that container.

### Error response from daemon: remove <volume>: volume is in use - [<id>]
Means: A container (running or stopped) uses the volume.
Fix: Remove the container first; check with `docker ps -a --filter volume=<volume>`.

### Error response from daemon: network <name> not found
Means: The network a container or stack refers to doesn't exist.
Fix: `docker network create <name>`, or for compose remove `external: true` so compose creates it.

### Error response from daemon: error while removing network: network <name> id <id> has active endpoints
Means: Containers are still attached to the network.
Fix: `docker network inspect <name>` lists them; disconnect or stop them.

### Error response from daemon: Pool overlaps with other one on this address space
Means: The new network's subnet overlaps an existing Docker network (or a host route).
Fix: Set a free `subnet:` for it, or configure `default-address-pools` in daemon.json. Overlap with your LAN or VPN causes unreachable devices too.

### Error response from daemon: could not find an available, non-overlapping IPv4 address pool among the defaults to assign to the network
Means: Docker ran out of default subnets (many compose projects each make a network).
Fix: `docker network prune`, share networks between stacks, or add larger `default-address-pools` in daemon.json.

### Error response from daemon: OCI runtime exec failed: exec failed: unable to start container process: exec: "bash": executable file not found in $PATH: unknown
Means: The image has no bash (Alpine and distroless images).
Fix: Try `sh`; distroless images have no shell at all, use `docker debug` or a sidecar.

### exec /usr/local/bin/docker-entrypoint.sh: exec format error
Means: The image is for another architecture, or the entrypoint script lacks a #! line or has CRLF endings.
Fix: Compare `docker image inspect -f '{{.Architecture}}' <image>` with `uname -m`; fix line endings when building your own.

### exec /<file>: no such file or directory
Means: The entrypoint exists but its interpreter doesn't (a glibc binary in an Alpine image, or `#!/bin/bash` in an image without bash, or CRLF line endings).
Fix: Use a matching base image, `#!/bin/sh`, or convert line endings.

### standard_init_linux.go:<n>: exec user process caused: exec format error
Means: Older runc wording of an architecture or #! problem.
Fix: As for "exec format error".

### Error response from daemon: unable to find user <user>: no matching entries in passwd file
Means: `--user <name>` names a user that doesn't exist inside the image.
Fix: Use a numeric uid:gid (`--user 1000:1000`) or a user the image defines.

### The container name "<name>" is already in use
Means: Short form of the name conflict above.
Fix: Remove or rename the old container.

### docker: invalid reference format.
Means: The image name has an illegal character: uppercase letters, spaces, an empty variable, or a stray `:`.
Fix: Image names are lowercase; quote and print variables before use.

### docker: invalid reference format: repository name must be lowercase.
Means: The image name contains uppercase letters (often `$PWD` with capitals in a volume flag that shifted arguments).
Fix: Lowercase the name; quote `-v "$PWD":/app` so spaces don't split it.

### docker: Error response from daemon: Unknown runtime specified nvidia.
Means: The NVIDIA container runtime isn't installed or registered.
Fix: Install nvidia-container-toolkit, run `sudo nvidia-ctk runtime configure --runtime=docker`, restart Docker. Prefer `--gpus all`.

### could not select device driver "" with capabilities: [[gpu]]
Means: `--gpus` was used but the NVIDIA toolkit isn't set up.
Fix: Install nvidia-container-toolkit, configure the runtime, restart Docker. See [NVIDIA drivers](kb:linux/nvidia-drivers).

### Error response from daemon: error gathering device information while adding custom device "<dev>": no such file or directory
Means: A `--device` path doesn't exist on the host (for example /dev/dri for Intel Quick Sync, or a USB stick that moved).
Fix: `ls -l /dev/dri`; use stable `/dev/serial/by-id/` paths for USB devices.

### WARNING: The requested image's platform (linux/amd64) does not match the detected host platform (linux/arm64/v8) and no specific platform was requested
Means: The image will run under emulation (or fail without it).
Fix: Use an arm64 image if one exists.

### WARNING: No swap limit support
Means: The kernel isn't booted with memory-swap accounting, so swap limits for containers aren't enforced.
Fix: Harmless for most setups. On Debian/Raspberry Pi, `cgroup_enable=memory swapaccount=1` on the kernel command line.

### failed to register layer: <details>: no space left on device
Means: Docker's storage (usually /var/lib/docker) is full.
Fix: `docker system df`; `docker image prune -a` and `docker builder prune` after checking. Move data-root to a bigger disk if needed.

### write /var/lib/docker/tmp/<file>: no space left on device
Means: Same: the disk under Docker's data root filled up during a pull or build.
Fix: Free space as above; container logs without rotation are a common cause (`log-opts max-size`).

### Error response from daemon: error creating overlay mount to <path>: invalid argument
Means: The overlay2 driver can't work on the backing filesystem (for example on top of ZFS older than 2.2, or XFS without d_type).
Fix: Use the `zfs` storage driver on ZFS (or ZFS 2.2+), or format XFS with `ftype=1`.

### failed to start daemon: error initializing graphdriver: driver not supported
Means: The configured storage driver doesn't work on this filesystem or kernel.
Fix: Remove `storage-driver` from daemon.json to let Docker pick, or move data-root.

### failed to start daemon: Error initializing network controller: <details>
Means: dockerd couldn't set up networking: iptables problems, a conflicting bridge, or a broken daemon.json.
Fix: `journalctl -u docker -b` for details; check `iptables -L` works and daemon.json is valid JSON.

### unable to configure the Docker daemon with file /etc/docker/daemon.json: <details>
Means: daemon.json is invalid JSON or has an unknown key, so dockerd won't start.
Fix: Validate it (`python3 -m json.tool /etc/docker/daemon.json`); remove the bad key.

### the following directives are specified both as a flag and in the configuration file: <directive>
Means: The same setting is in daemon.json and in dockerd's command line (systemd unit).
Fix: Keep it in one place; on Ubuntu the `-H fd://` flag conflicts with `hosts` in daemon.json.

### Error response from daemon: client version <n> is too new. Maximum supported API version is <n>
Means: The docker CLI is newer than the daemon (or a tool bundles a newer client).
Fix: Update Docker Engine, or set `DOCKER_API_VERSION=<n>` for that client.

### Error response from daemon: OCI runtime create failed: <details>: permission denied: unknown
Means: The runtime couldn't access something: SELinux/AppArmor, a noexec mount, or a cgroup restriction in nested containers (Docker in LXC).
Fix: Check `dmesg`/audit logs. For Docker inside Proxmox LXC, enable nesting (and keyctl for unprivileged containers).

### Error response from daemon: failed to create shim task: OCI runtime create failed: runc create failed: unable to start container process: error during container init: open sysctl net.ipv4.ip_unprivileged_port_start file: reopen fd 8: permission denied: unknown
Means: A known issue running Docker inside unprivileged LXC containers with certain runc/AppArmor versions.
Fix: Update Proxmox and the container's packages; enable nesting; as a fallback run Docker in a VM.

### Error response from daemon: cgroups: cgroup mountpoint does not exist: unknown
Means: The host uses a cgroup layout the runtime doesn't expect (old Docker on a cgroup v2 host, or inside a container).
Fix: Update Docker/containerd; avoid mixing cgroup v1-only tools with v2 hosts.

### Error response from daemon: unable to remove filesystem for <id>: <details>: device or resource busy
Means: Something still holds files of that container's layer (another process with the mount open).
Fix: Retry after a moment; find holders with `grep <id> /proc/*/mountinfo`; restart Docker if stuck.

### Error response from daemon: Container <id> is already in progress
Means: Another operation (stop, remove) is running on that container.
Fix: Wait, then retry.

### Error response from daemon: cannot stop container: <name>: tried to kill container, but did not receive an exit event
Means: The container's process didn't die, often stuck in uninterruptible I/O (an NFS mount gone away).
Fix: Check `dmesg` for hung tasks and NFS; restart the Docker daemon or reboot if it's stuck in D state.

### OOMKilled: true
Means: The container was killed for exceeding its memory limit (or the host ran out of memory).
Fix: Raise `mem_limit`/`deploy.resources.limits.memory`, or fix the app's memory use. `docker stats` shows usage.

### Exited (137)
Means: Killed with SIGKILL: out of memory, a memory limit, or `docker kill`/a stop timeout.
Fix: `docker inspect -f '{{.State.OOMKilled}}' <name>`. If false, it didn't stop within the stop timeout: raise `stop_grace_period`.

### Exited (143)
Means: Stopped with SIGTERM, a normal `docker stop`.
Fix: Nothing, unless no one stopped it: check for a healthcheck-based restarter or watchtower.

### Exited (139)
Means: The process crashed with a segmentation fault.
Fix: Try the previous image tag; check the architecture; memory faults on the host.

### Exited (126)
Means: The command in the container couldn't be executed (not executable, wrong format).
Fix: Check the entrypoint's permissions and #! line.

### Exited (127)
Means: The command in the container wasn't found.
Fix: Check the command and the image's PATH.

### Exited (1)
Means: The app exited with a generic error.
Fix: `docker logs <name>` shows why; usually config, a missing env var, or a dependency not reachable.

### health: starting
Means: The healthcheck hasn't passed yet; containers that depend on it wait.
Fix: Normal at start. If it lasts, the check may be wrong: `docker inspect -f '{{json .State.Health}}' <name>`.

### (unhealthy)
Means: The container's healthcheck keeps failing.
Fix: Read the last check outputs in `.State.Health.Log`; the check command may need curl/wget the image doesn't have.

### Error response from daemon: the container is not running, can't attach
Means: `docker attach` needs a running container.
Fix: Start it, or read past output with `docker logs`.

### error during connect: <details>: open //./pipe/docker_engine: The system cannot find the file specified.
Means: On Windows, Docker Desktop's engine isn't running.
Fix: Start Docker Desktop and wait for it to say it's running.

### ERROR: failed to solve: <image>: failed to resolve source metadata for <image>: <details>
Means: BuildKit couldn't fetch the base image's metadata (name, tag, auth or network).
Fix: Check the FROM line's image and tag; `docker pull` it to see the real error.

### ERROR: failed to solve: process "/bin/sh -c <cmd>" did not complete successfully: exit code: <n>
Means: A RUN step in the Dockerfile failed.
Fix: Scroll up for the command's own output. Build with `--progress=plain --no-cache` to see everything.

### ERROR: failed to solve: failed to compute cache key: failed to calculate checksum of ref <id>::<id>: "/<path>": not found
Means: A COPY or ADD source path isn't in the build context (or is excluded by .dockerignore).
Fix: Paths are relative to the context directory; check .dockerignore.

### COPY failed: file not found in build context or excluded by .dockerignore: stat <path>: file does not exist
Means: Legacy builder's version of the missing COPY source.
Fix: Same: context path and .dockerignore.

### ERROR: failed to solve: dockerfile parse error on line <n>: unknown instruction: <word>
Means: The Dockerfile has a typo or a line continuation (\) problem.
Fix: Check the line; a trailing space after `\` breaks continuations.

### ERROR: Multi-platform build is not supported for the docker driver.
Means: `--platform` with several platforms needs a buildx builder with the container driver.
Fix: `docker buildx create --use`, and install qemu emulators (`docker run --privileged --rm tonistiigi/binfmt --install all`).

### Error: short-name "<name>" did not resolve to an alias and no unqualified-search registries are defined in "/etc/containers/registries.conf"
Means: Podman needs a full image name (with registry) unless search registries are configured.
Fix: Use `docker.io/library/<name>` or add `unqualified-search-registries = ["docker.io"]`.

### Error: rootlessport cannot expose privileged port <port>, you can add 'net.ipv4.ip_unprivileged_port_start=<port>' to /etc/sysctl.conf
Means: Rootless Podman (or rootless Docker) can't bind ports below 1024.
Fix: Use a high port and a reverse proxy, or lower `net.ipv4.ip_unprivileged_port_start`.

### Error: cannot setup namespace using newuidmap: exit status 1
Means: Rootless containers need subordinate uid/gid ranges for your user.
Fix: Add `<user>:100000:65536` to /etc/subuid and /etc/subgid, then `podman system migrate`.

### ERRO[0000] cannot find UID/GID for user <user>: no subuid ranges found for user "<user>" in /etc/subuid
Means: Same: no subuid/subgid entries for rootless use.
Fix: `sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 <user>`.

### Error: crun: <details>: Permission denied: OCI permission denied
Means: Podman's runtime was denied access (SELinux labels on a bind mount, most often).
Fix: Add `:Z` (private) or `:z` (shared) to the volume.

### WARN[0000] The cgroupv2 manager is set to systemd but there is no systemd user session available
Means: Rootless Podman run over SSH or su without a user session.
Fix: `loginctl enable-linger <user>` and log in properly, or use `--cgroup-manager=cgroupfs`.

### Error: no container with name or ID "<name>" found: no such container
Means: Podman found no such container (rootless and root containers are separate lists).
Fix: Check `podman ps -a` as the same user that created it (and `sudo podman ps -a`).
