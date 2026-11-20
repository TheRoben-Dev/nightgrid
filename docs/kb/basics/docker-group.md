# The docker group

On Linux, the Docker daemon listens on a socket (`/var/run/docker.sock`) owned by root and the `docker` group. Adding a user to that group lets them run `docker` without sudo. It also gives them root on that host, which is worth knowing before you do it.

![Why the docker group equals root.](fig:bs-docker-group)

## Why it's root

Anyone who can talk to the Docker socket can start a container that mounts the host's root filesystem:

```sh
docker run --rm -it -v /:/host alpine chroot /host
```

That's a root shell on the host, no password asked. So membership in `docker` is equivalent to passwordless sudo.

## When it's fine

On a single-purpose homelab server where your user already has sudo, adding yourself to `docker` changes nothing about who can become root, and it makes Docker tools (including Nightgrid) work without sudo prompts:

```sh
sudo usermod -aG docker "$USER"
```

Log out and back in (or reconnect the SSH session) for the group to apply. Check with `id`.

## Synology NAS (DSM)

Synology does things its own way, so the commands above fail there (`usermod: command not found`):

- DSM manages users and groups with `synogroup`, not `usermod`/`groupadd`.
- Container Manager (formerly the Docker package) creates `/var/run/docker.sock` owned by `root:root`, so even a docker group can't use it until the socket's group is changed.
- The `docker` binary lives in `/usr/local/bin`, which non-interactive SSH sessions don't have on `PATH`. Nightgrid adds it for you.

Your user must be in DSM's **administrators** group to use `sudo`. Then, over SSH:

```sh
sudo synogroup --add docker "$USER"            # creates the group with you in it
# or, if a docker group already exists:
sudo synogroup --memberadd docker "$USER"
sudo chown root:docker /var/run/docker.sock
```

Reconnect for the group to apply. Nightgrid's **NO ACCESS TO DOCKER** card detects DSM, shows exactly these commands for your user, and its **Fix it for me** button runs them (asking for your sudo password) and reconnects.

If the socket goes back to `root` after Container Manager restarts (a package update or a reboot), the card comes back; one more click fixes it. To make it stick, add a **Triggered Task** in DSM's Control Panel → Task Scheduler (user root, event Boot-up) that runs `sleep 60; chown root:docker /var/run/docker.sock`.

Portainer and similar tools get the same access by mounting the socket into their container: it's root-equivalent either way.

## When it isn't

- A shared machine where other users shouldn't have root.
- A service account that only needs to read container status. Give it a narrower tool instead.
- Never expose the Docker API over TCP without TLS (port 2375). Nightgrid's network scan flags it in red for that reason.

## Alternatives

- **sudo for docker only**: keep the user out of the group and give it passwordless sudo for docker. When the socket is off limits but `sudo -n` works, Nightgrid reaches Docker through `sudo docker system dial-stdio` on its own.
- **Rootless Docker** runs the daemon as your user. Containers can't become root on the host, at the cost of some features (ports below 1024, some network modes).
- **Podman** is daemonless and rootless by default, with a mostly compatible CLI.

Nightgrid reads which way a host is set up when it connects to Docker and tells you if the socket isn't reachable.

## Related

- [Docker and compose basics](kb:basics/docker-compose)
- [Hardening sshd](kb:basics/sshd-hardening): who can log in at all matters more once they can reach Docker.
