# Prepare a Linux Docker host

Turn a fresh Debian or Ubuntu server into a tidy Docker host: the official engine and compose plugin, log rotation, a stacks folder and ports that don't slip past the firewall.

![The layers of a Docker host.](fig:app-docker-host)

## What you end up with

- Docker Engine and the compose plugin from Docker's own repository
- Container logs that can't fill the disk
- One folder per stack under `/opt/stacks`
- The host in Nightgrid with Docker, monitoring and updates working

## Before you start

- Debian 12/13 or Ubuntu 22.04/24.04 with a sudo user and SSH key login.
- The host added in Nightgrid (see [Add your first host](kb:howto/first-host)).

## Part 1 · Install Docker

- [ ] In a [Terminal](view:terminal) on the host, remove distro packages that conflict:

```sh
for p in docker.io docker-compose podman-docker containerd runc; do sudo apt-get remove -y $p; done
```

- [ ] Add Docker's repository and install (Debian shown; for Ubuntu replace `debian` with `ubuntu` in both URLs):

```sh
sudo apt-get update && sudo apt-get install -y ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian $(. /etc/os-release && echo $VERSION_CODENAME) stable" | sudo tee /etc/apt/sources.list.d/docker.list
sudo apt-get update && sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
```

- [ ] Let your user run Docker without sudo, then log out and back in (read [The docker group](kb:basics/docker-group) first: it's root-equivalent):

```sh
sudo usermod -aG docker $USER
```

## Part 2 · Sensible defaults

- [ ] Rotate container logs. Put this in `/etc/docker/daemon.json` (edit it with Nightgrid's editor via `sudoedit`):

```json
{
  "log-driver": "local",
  "log-opts": { "max-size": "20m", "max-file": "3" }
}
```

- [ ] `sudo systemctl restart docker`. Existing containers keep their old log settings until they're recreated.
- [ ] Make the stacks folder: `sudo mkdir -p /opt/stacks && sudo chown $USER: /opt/stacks`.
- [ ] Add `/opt/stacks` as a bookmark in [Files](view:files).

## Part 3 · Ports and the firewall

- [ ] Docker writes its own iptables rules, so a published port ignores ufw. Publish only what must be public, and bind everything else to localhost: `127.0.0.1:8080:8080`.
- [ ] Put apps behind one reverse proxy on 80/443 (see [Caddy reverse proxy](kb:apps/caddy)).

#### Check

- [ ] `docker run --rm hello-world` works without sudo.
- [ ] [Docker](view:docker) in Nightgrid lists the host's containers, and [System](view:system) → **security** shows no Docker API on all interfaces.

## Part 4 · Keep it healthy

- [ ] Prune unused images monthly: a [timer](kb:howto/schedules) running `docker image prune -af --filter "until=720h"`.
- [ ] Engine updates come with normal OS updates ([Patch every host from one place](kb:howto/fleet-updates)).

## Limits and gotchas

- Never expose the Docker API (port 2375/2376) to the network. Nightgrid doesn't need it; it uses SSH.
- `docker.sock` mounted into a container gives that container root on the host. Only do it for tools you trust.
- Rootless Docker and Podman work for many apps but not for ones that need privileged ports or device access.

## Related

- [Docker and compose basics](kb:basics/docker-compose)
- [Deploy a new compose stack](kb:howto/deploy-stack)
- [Caddy reverse proxy](kb:apps/caddy)
- [The docker group](kb:basics/docker-group)
