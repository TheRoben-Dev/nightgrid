# Dev and git deploy lab

![Git, CI and deploys on your own hardware.](fig:guide-dev-git-deploy-lab)

A self-hosted git server with CI, plus staging and production Docker hosts cloned from one Proxmox template. Staging deploys itself on every push through a CI runner. Production deploys only when you review the diff in Nightgrid and press the button, with a snapshot to roll back to.

## What you end up with

- A Debian 12 LXC template with Docker, and three containers made from it: `git` (Forgejo plus a runner), `staging` (a linked clone) and `prod` (a full clone).
- Forgejo (or Gitea) hosting your repositories, with Actions doing build and test.
- Compose stacks on `staging` and `prod` tracked with Docker → **From git…**: **Check** for new commits, **Review & deploy…** the diff, **Roll back to …** when it goes wrong.
- Secrets in Nightgrid's encrypted **Vault**, written to `.env` (mode 600) per host, never committed.
- A config baseline per host, so you can see how `prod` drifted from `staging`.
- Caddy or Traefik routes and a service check for each environment.

## Before you start

- A Proxmox VE 8.x node with 4 cores and 8 GB of RAM to spare for three small containers.
- Settings → **FEATURES**: **Standard** leaves out Proxmox. Turn it on, or pick **Everything**.
- An SSH key in Settings → **CONNECTIONS** → **SSH KEYS**. **+ Container** needs at least one public key, because new containers have no root password.

## Part 1 · One template, three containers

- [ ] [Proxmox](view:proxmox) → **STORAGE** → **Get CT template…** → Debian 12 standard.
- [ ] **GUESTS** → **+ Container**: name it `docker-tpl`, 2 cores, 2 GB, 16 GB disk, your SSH key.
- [ ] Docker in an unprivileged LXC needs nesting and keyctl. Set them while the container is stopped:

```sh
pct set <ctid> --features nesting=1,keyctl=1
```

- [ ] Start it, add it as a host and install Docker Engine with the Compose plugin from Docker's repository (Debian's `docker-compose` package is the old v1 tool, and this guide uses `docker compose`):

```sh
apt update && apt install -y curl git && curl -fsSL https://get.docker.com | sh
```

- [ ] Shut it down, then **⋯** → **Convert to template…**.
- [ ] **⋯** → **Clone…** three times: `git` (full), `prod` (full) and `staging` (linked). Linked clones only come from templates. They're fast and small, but they depend on the template, so don't destroy it while `staging` exists.
- [ ] Each clone keeps the template's SSH host keys and machine-id. Regenerate both on each clone before adding it as a host:

```sh
rm -f /etc/ssh/ssh_host_* && dpkg-reconfigure openssh-server && rm -f /etc/machine-id && systemd-machine-id-setup
```

- [ ] Add `git`, `staging` and `prod` as hosts, in a group `lab`.

#### Check

- [ ] Docker → pick each host. The header shows the engine version.

## Part 2 · Forgejo and a runner

- [ ] On `git`: Docker → **+ New stack** → paste YAML. Paste this into the editor; don't send it to a terminal. Pin `<version>` to a current release:

```yaml
services:
  forgejo:
    image: codeberg.org/forgejo/forgejo:<version>
    environment:
      - USER_UID=1000
      - USER_GID=1000
    ports:
      - "3000:3000"
      - "2222:22"
    volumes:
      - ./data:/data
    restart: unless-stopped
  runner:
    image: data.forgejo.org/forgejo/runner:<version>
    user: "0:0"
    command: forgejo-runner daemon --config /data/config.yml
    volumes:
      - ./runner:/data
      - /var/run/docker.sock:/var/run/docker.sock
    restart: unless-stopped
    depends_on:
      - forgejo
```

- [ ] **up** the stack, open `http://<git-ip>:3000` and finish the installer. Turn off open registration.
- [ ] Get a runner token in Forgejo (Site administration → Actions → Runners → Create new runner) and register the runner once. The token is a secret: run this in a Nightgrid terminal and clear the line from history afterwards, or paste the token at an interactive `forgejo-runner register` prompt instead:

```sh
docker compose run --rm runner forgejo-runner register --no-interactive --instance http://forgejo:3000 --token <token> --name lab-runner
```

- [ ] Generate a default runner config into `./runner/config.yml`, then restart the stack:

```sh
docker compose run --rm runner forgejo-runner generate-config > runner/config.yml
```

- [ ] The runner gets the host's Docker socket, which is root on `git`. Keep untrusted repositories off this runner.

#### Check

- [ ] The runner shows as idle in Forgejo. Push a repository with a trivial `.forgejo/workflows/ci.yml` and watch it run.

## Part 3 · Deploys with review

- [ ] Put each app's `compose.yaml` in a repository, one folder per stack if the repo holds several.
- [ ] On `prod`, make a deploy key and add the public half to the repository in Forgejo (Settings → Deploy keys, read-only):

```sh
ssh-keygen -t ed25519 -N '' -f ~/.ssh/id_ed25519 -C prod-deploy
```

- [ ] Docker → **From git…** on `prod`: repository address (`ssh://git@<git-ip>:2222/<user>/<repo>.git`), branch `main`, a folder on the host and the stack's folder inside the repo. **Clone**.
- [ ] After a push: **Check** shows the new commits. **Review & deploy…** shows the diff; **Deploy and up** saves a snapshot, moves the checkout to that commit and brings the stack up.
- [ ] Something broke: **Roll back to …** goes back to the previous deploy.
- [ ] Files edited directly on the host block a deploy until you commit or undo them there. That's on purpose: fix it in git.
- [ ] For `staging`, let CI do it: a workflow job that SSHes to `staging` and runs `git pull && docker compose up -d`. Keep `prod` on manual review in Nightgrid. Nightgrid doesn't poll or auto-deploy, so production only changes when you press the button.

#### Check

- [ ] Push a harmless change, deploy it to `prod` from Nightgrid, then **Roll back to …**. The stack should be on the old commit again, and System → **timeline** on `prod` lists both the deploy and the rollback.

## Part 4 · Secrets in the Vault

- [ ] Docker → the stack → **Vault** → **Import from host .env** to pull in what's there, or paste `NAME=value` lines.
- [ ] **Deploy .env…** shows which names are added, changed or removed; **Write .env to <host>** saves a snapshot and writes the file with mode 600. **Up now** restarts the stack with the new values.
- [ ] Keep separate values per host: the Vault is per stack per host, so `staging` and `prod` can't leak into each other.
- [ ] Add `.env` to the repository's `.gitignore`. DRIFT's compose review flags secrets written inline in `compose.yaml`.

## Part 5 · Drift, routes and checks

- [ ] System → **config** → **Save baseline** on `staging` right after a known-good deploy, and on `prod`.
- [ ] Later, **Compare** shows changed files under `/etc`, the stack folders and the package list. **Compare with** `staging` shows how the two environments differ right now. `.env` files are compared by checksum only.
- [ ] Routes: on a host running Caddy, Docker → **Routes** → **Edit routes…** → **Add site…** (`app.lab.example` → `app:8080`), check the diff and **Save and reload Caddy**. For Traefik, routes go to `nightgrid.yml` in its file provider directory.
- [ ] [Services](view:services): one check per environment (`https://app.staging.lab.example`, `https://app.lab.example`). Certificate expiry shows up in Services → **EXPIRY WATCH** automatically.
- [ ] Settings → **Dependencies**: "prod app depends on prod", "staging app depends on staging", and both on the Proxmox node.

## Part 6 · A promote playbook

- [ ] [Automation](view:automation) → **Playbooks**: a "smoke test" that runs on `staging` after CI deploys:
- [ ] Step 1: wait for port 443 on the host.
- [ ] Step 2: a command that hits the health endpoint and fails if it doesn't answer:

```sh
curl -fsS https://app.staging.lab.example/healthz
```

- [ ] Step 3: Docker restart of a worker container, if your app needs one after a migration.
- [ ] A snippet for one-off image tags across `lab`, with `{{tag=latest}}` asked for when it runs. Values are quoted for you.
- [ ] Destructive steps (`compose down`, `prune`, `rm`) are confirmed with the targets named, and on several hosts you type the group name.

## When Nightgrid is closed

- Runs without the app: Forgejo, the runner and every CI job, CI deploys to `staging`, Docker restart policies, Caddy and Traefik, timers and cron on the hosts.
- Needs Nightgrid: production deploys and rollbacks (by design), Vault writes, drift comparisons, service checks and alerts, playbook runs.
- If you want `prod` to be deployable from a phone, that's a CI job with manual approval in Forgejo, not Nightgrid.

## Limits and gotchas

- **From git…** uses the host's own git access. Each host needs its own deploy key, and Nightgrid never sees it.
- Docker in unprivileged LXC works on ZFS with ZFS 2.2+ (Proxmox VE 8.1+). On older ZFS, use a VM for Docker or an ext4 volume for `/var/lib/docker`.
- Linked clones share the template's base disk. Back up `staging` like any guest, but restore it as a full guest.
- Cloned containers get the template's SSH host keys unless you regenerate them (Part 1). Nightgrid will refuse a host key that changes later until you confirm it, which is what you want.
- The runner with the Docker socket is effectively root on `git`. Don't run pull requests from forks on it.

## Related

Proxmox media server · Game server host · Family cloud and photos · Repair bench server · Local AI GPU server · Smart home hub · Remote family IT
