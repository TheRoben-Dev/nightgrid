# Change a firewall safely

Add and remove ufw or firewalld rules from Nightgrid with an automatic rollback, so a mistake that cuts off SSH undoes itself.

## What you end up with

- The host's firewall rules readable in one place, whatever tool it uses
- Changes applied with a 1, 2 or 5 minute safety timer that you confirm with **Keep**
- A sensible default: deny incoming, allow SSH from where you connect

## Part 1 · Look first

- [ ] [System](view:system) → **FIREWALL** shows whether the host uses ufw, firewalld or plain nftables/iptables, and its rules. The raw ruleset is under **nft list ruleset**.
- [ ] [System](view:system) → **security** lists every listening port and who can reach it.

## Part 2 · ufw from scratch

- [ ] If ufw is installed but off, add the SSH rule **before** turning it on: **Add…** → allow, port 22, tcp, from your LAN or tailnet range, comment "ssh".
- [ ] Add rules for what the host serves (80 and 443 for a reverse proxy, 53 for DNS).
- [ ] Set the incoming default to deny and turn ufw on.
- [ ] Each change shows what will happen and asks how long to wait (1, 2 or 5 minutes) before **Apply with rollback**.

## Part 3 · Keep or undo

- [ ] Right after a change Nightgrid opens a fresh SSH connection to prove you still get in. **Test again** repeats it.
- [ ] Press **Keep**. If you don't, the host restores the old rules by itself when the timer runs out. **Undo now** restores them at once.
- [ ] A change that could close Nightgrid's own SSH port asks you to type the host name.

#### Check

- [ ] From another machine, `nc -vz <host> <port>` connects to allowed ports and times out on others.

## Part 4 · firewalld

- [ ] Add or remove services, ports and sources per zone. The same rollback applies.

## Limits and gotchas

- Docker bypasses ufw for published ports. Bind containers to `127.0.0.1` or a LAN IP, or use `DOCKER-USER` rules (see [Prepare a Linux Docker host](kb:apps/docker-host)).
- nftables and iptables setups are read-only in Nightgrid.
- Proxmox has its own firewall: Proxmox → **FIREWALL**, also with a 90-second **Keep**.

## Related

- [Raise a host's security score](kb:howto/security-score)
- [fail2ban and CrowdSec](kb:apps/fail2ban-crowdsec)
- [When a host won't connect](kb:howto/troubleshoot-connection)
