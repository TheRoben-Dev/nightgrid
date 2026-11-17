# Kubernetes-lite with k3s

Run a small Kubernetes cluster on one or a few machines with k3s: a single binary with storage, ingress and load balancing built in. Useful for learning Kubernetes or running Helm charts.

![A k3s server with two agents.](fig:app-k3s)

## What you end up with

- A k3s server (and optional agents) with Traefik ingress and local-path storage
- `kubectl` and Helm working from the server or your computer
- Nodes monitored in Nightgrid like any host

## Before you start

- 2 GB RAM per node minimum, Linux with a static IP.
- Docker and k3s on the same host work but compete for ports 80/443 (k3s ships Traefik). Use separate hosts or disable one ingress.
- Nightgrid has no Kubernetes view. You manage k3s from its terminal; Nightgrid covers the hosts.

## Part 1 · Install the server

- [ ] In a [Terminal](view:terminal) on the first node (read the script first if you like, it's on k3s's site):

```sh
curl -sfL https://get.k3s.io | sh -
sudo k3s kubectl get nodes
```

- [ ] DRIFT's command check will flag `curl | sh`; that's expected here.
- [ ] Use kubectl without sudo:

```sh
mkdir -p ~/.kube && sudo cp /etc/rancher/k3s/k3s.yaml ~/.kube/config && sudo chown $USER: ~/.kube/config
```

## Part 2 · Add agents (optional)

- [ ] On the server: `sudo cat /var/lib/rancher/k3s/server/node-token`.
- [ ] On each agent:

```sh
curl -sfL https://get.k3s.io | K3S_URL=https://<server-ip>:6443 K3S_TOKEN=<token> sh -
```

## Part 3 · Deploy something

- [ ] Install Helm on the server, then for example:

```sh
helm repo add podinfo https://stefanprodan.github.io/podinfo
helm install demo podinfo/podinfo
kubectl get pods
```

## Part 4 · In Nightgrid

- [ ] Command widgets (read-only, no sudo) on the server: pods not running

```sh
kubectl get pods -A --no-headers | grep -vcE 'Running|Completed'
```

with **Low is bad** off and warn at 1.
- [ ] Snippets in [Automation](view:automation) for `kubectl get pods -A`, `kubectl top nodes`, `kubectl rollout restart deploy/{{name}} -n {{ns=default}}`.
- [ ] `k9s` in a Nightgrid terminal gives a full-screen cluster UI.

#### Check

- [ ] `kubectl get nodes` shows every node Ready.

## Limits and gotchas

- Back up `/var/lib/rancher/k3s/server` (the embedded database and token).
- Upgrades: rerun the install script with `INSTALL_K3S_CHANNEL=stable`, one minor version at a time, server first.

## Related

- [Build dashboards with command widgets](kb:howto/command-widgets)
- [Ansible for your homelab](kb:apps/ansible)
- [Dev and git deploy lab](kb:guides/dev-git-deploy-lab)
