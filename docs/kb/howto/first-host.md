# Add your first host

Get one server into Nightgrid with key login, a trusted host key and monitoring, in about five minutes. Everything after this builds on it.

![From an address to a monitored host.](fig:ng-first-host)

## What you end up with

- A saved host that logs in with your SSH key, never a typed password
- Its host key accepted once, so a changed key is caught later
- Live CPU, memory, disk and network on the [Overview](view:overview)
- A terminal, files and Docker on that host from one click

## Before you start

- A Linux server you can already reach with `ssh user@address` from this computer.
- An SSH key on this computer. No key yet? **Settings → Connections → SSH keys → NEW KEY…** makes an ed25519 key in `~/.ssh` (see [Set up and rotate SSH keys](kb:howto/ssh-keys)).
- If you already have hosts in `~/.ssh/config`, **IMP** in the sidebar imports them instead (see [Import hosts and use jump hosts](kb:howto/jump-hosts)).

## Part 1 · Save the host

- [ ] Press **+** in the sidebar.
- [ ] Fill in a name you'll recognise (`media`, `nas`, `server01`), the address (LAN IP, DNS name or tailnet name), port 22 and your user.
- [ ] Pick **SSH agent** if your key is loaded in an agent (1Password and Secretive count), or **Key file** and choose the private key.
- [ ] Laptops, desktops and handhelds you also manage: tick **Client, not a server** so views never open on them by themselves, and so they show as away (no offline alert) when they're off.
- [ ] Add a group (`home`, `lab`) and a tag or two now. Groups drive group power, fleet upgrades and Automation targets later.

## Part 2 · Connect once

- [ ] Click the host. The first connection shows the server's fingerprint. Compare it with the server's own (run `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` on its console) and accept.
- [ ] A terminal opens. Background features (monitoring, checks) never prompt, so this first visit is where passwords and host keys get answered.
- [ ] Still asked for a password? Your key isn't on the server yet. **Settings → Connections → SSH keys → CHECK HOSTS**, then **+** on the host's row adds it.

#### Check

- [ ] The host shows on the [Overview](view:overview) with live numbers.
- [ ] Reconnecting doesn't ask for anything.

## Part 3 · First look around

- [ ] [System](view:system) → **security** gives the host a hygiene score with fixes for each finding.
- [ ] [System](view:system) → **updates** lists pending packages.
- [ ] [Docker](view:docker) shows its containers if Docker runs there.
- [ ] Settings → **FEATURES**: start with **Simple** or **Standard** if the sidebar feels busy. Nothing is deleted when you hide a view.

## When Nightgrid is closed

- Nothing runs on the server for Nightgrid. There is no agent: every feature uses SSH while the app is open (or in the tray).

## Limits and gotchas

- A host that needs a password or a new host key shows **needs a login** until you open a terminal on it once.
- If the fingerprint changes later, the connection is blocked until you type the host name to accept the new key. A reinstall does this; an unexplained change is worth investigating first.
- Hosts on another network: add them with their tailnet address or through a jump host.

## Related

- [Set up and rotate SSH keys](kb:howto/ssh-keys)
- [Import hosts and use jump hosts](kb:howto/jump-hosts)
- [When a host won't connect](kb:howto/troubleshoot-connection)
- [Prepare a Linux Docker host](kb:apps/docker-host)
