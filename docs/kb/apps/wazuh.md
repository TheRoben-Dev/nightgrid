# Wazuh security monitoring (SIEM)

An open-source security platform: agents on each host report logins, file changes, rootkit checks, known vulnerabilities in installed packages and configuration problems to one manager, with a web dashboard to search and alert on all of it.

![Wazuh agents, manager, indexer and dashboard.](fig:app-wazuh)

## What you end up with

- A single-node Wazuh manager, indexer and dashboard in Docker
- Agents on your Linux hosts reporting security events
- A vulnerability list per host, based on its installed packages

## Before you start

- A host with at least 4 cores, 8 GB RAM and 50 GB free disk. The indexer (OpenSearch) is the hungry part.
- Pick one Wazuh version (for example 4.x latest) and use the same version for manager, agents and the docker repo tag.

## Part 1 · The stack

- [ ] The indexer needs a higher memory map limit on the host. Set it now and make it stick:

```bash
sudo sysctl -w vm.max_map_count=262144
echo 'vm.max_map_count=262144' | sudo tee /etc/sysctl.d/99-wazuh.conf
```

- [ ] Wazuh's compose files and certificate generator live in the official `wazuh-docker` repository. In a terminal on the host:

```bash
git clone https://github.com/wazuh/wazuh-docker.git -b v4.9.2 /opt/stacks/wazuh-src
cd /opt/stacks/wazuh-src/single-node
docker compose -f generate-indexer-certs.yml run --rm generator
docker compose up -d
```

- [ ] Replace `v4.9.2` with the current release tag. The generator writes TLS certificates the three containers use to talk to each other.
- [ ] The dashboard is on `https://<host>` (port 443). Default login is `admin` / `SecretPassword`: change it right away by following the "change the default password" steps in the repo's README (it involves a hash and a restart, not just the web UI).

## Part 2 · Agents

- [ ] Dashboard → **Agents** → **Deploy new agent**: pick the OS, enter the manager's address, and copy the generated install command. It sets `WAZUH_MANAGER` and installs the package from Wazuh's repository.
- [ ] Run it on each host from the Nightgrid [Terminal](view:terminal), then start the agent:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now wazuh-agent
```

- [ ] Agents connect out to the manager on TCP 1514 (events) and 1515 (enrolment). Open those on the manager's firewall for your LAN only.

## Part 3 · What to look at

- [ ] **Threat hunting / Events**: failed SSH logins, sudo use, new users.
- [ ] **Vulnerability detection**: CVEs per host from its package list. Patch from [Fleet](view:fleet) → **Updates** and watch the count fall.
- [ ] **File integrity monitoring**: `/etc`, `/usr/bin` and others by default. Add your compose folders in the agent's `ossec.conf` under `<syscheck>`.
- [ ] **Configuration assessment**: CIS-style checks with a pass/fail score per host.

#### Check

- [ ] Every host shows as **Active** under Agents, and a failed SSH login on one of them appears in events within a minute.

## In Nightgrid

- Add the dashboard to [Services](view:services) as an HTTPS check (it uses a self-signed certificate, so expect the "untrusted" note).
- Nightgrid's own host **security** check and [Fleet](view:fleet) → **Access** cover a lot of the same ground with nothing to install. Wazuh is worth it when you want history, central alert rules and vulnerability matching.

## Limits and gotchas

- The indexer grows quickly. Set index retention (Index management → policies) to 30–90 days or the disk fills.
- Upgrades: manager first, then agents. Agents newer than the manager are refused.
- Containers and short-lived VMs need an agent inside each, or rely on the Docker listener module; one agent on the Docker host doesn't see inside containers.

## Related

- [fail2ban and CrowdSec](kb:apps/fail2ban-crowdsec)
- [Audit who can log in](kb:howto/access-audit)
- [Raise a host's security score](kb:howto/security-score)
