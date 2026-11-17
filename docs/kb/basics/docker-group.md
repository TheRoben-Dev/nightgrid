# The docker group

On Linux, the Docker daemon listens on a socket (`/var/run/docker.sock`) owned by root and the `docker` group. Adding a user to that group lets them run `docker` without sudo. It also gives them root on that host, which is worth knowing before you do it.

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

## When it isn't

- A shared machine where other users shouldn't have root.
- A service account that only needs to read container status. Give it a narrower tool instead.
- Never expose the Docker API over TCP without TLS (port 2375). Nightgrid's network scan flags it in red for that reason.

## Alternatives

- **sudo for docker only**: keep the user out of the group and let Nightgrid ask for the sudo password (it's fed over the SSH channel, never on a command line). Slower, but explicit.
- **Rootless Docker** runs the daemon as your user. Containers can't become root on the host, at the cost of some features (ports below 1024, some network modes).
- **Podman** is daemonless and rootless by default, with a mostly compatible CLI.

Nightgrid reads which way a host is set up when it connects to Docker and tells you if the socket isn't reachable.

## Related

- [Docker and compose basics](kb:basics/docker-compose)
- [Hardening sshd](kb:basics/sshd-hardening): who can log in at all matters more once they can reach Docker.
