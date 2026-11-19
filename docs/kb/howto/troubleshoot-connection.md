# When a host won't connect

Work out why a saved host shows **OFFLINE**, **needs a login** or an error, from the quickest checks to the rarer causes.

![What each connection message usually means.](fig:ng-ssh-errors)

## Before you start

- Note the exact message on the host's row or in the terminal tab. Most causes can be read straight off it.

## Part 1 · Read the message

- [ ] **needs a login**: the host wants a password, a passphrase or a new host key. Open a terminal on it once and answer. Background features never prompt.
- [ ] **Host key changed**: the server's identity is different. A reinstall or a new IP reused by another machine causes it. Type the host name only once you know why.
- [ ] **Permission denied (publickey)**: the key Nightgrid offers isn't in the server's `authorized_keys`, and the server doesn't take passwords. Check **Settings → Connections → SSH keys → CHECK HOSTS**. When the server does take passwords, a refused key falls back to asking for the password, so you can log in and add the key.
- [ ] **SSH agent** hosts with no agent running (normal on Windows) or an empty one: Nightgrid tries `~/.ssh/id_ed25519`, `id_ecdsa` and `id_rsa`, then the password, like OpenSSH. Load your key into the agent, or pick **Key file** in the host's editor, to skip the password.
- [ ] **Connection refused**: the machine answers but nothing listens on the port. sshd is stopped, or it listens on another port.
- [ ] **Timed out** or **No route**: the address is wrong, the machine is off, a firewall drops the packets, or you're away from home without a tailnet route.
- [ ] **ASLEEP**, **SUSPENDED**, **SHUT DOWN**: Nightgrid knows it's off on purpose; no alert fires.

## Part 2 · Quick checks from another host

- [ ] [Network](view:network) → **Tools** with **Run from** set to another host on the same LAN: **Ping**, then **Ports** with `22`. Or in a [Terminal](view:terminal) on that host:

```sh
ping -c3 <address>
nc -vz <address> 22
```

- [ ] Ping works but port 22 doesn't: sshd or the firewall. Neither works: power, cabling, IP change (DHCP), or the wrong VLAN.
- [ ] The [Network](view:network) map's **Scan from …** shows whether the machine still answers on the LAN and with which address.

## Part 3 · On the console

When you can get to the machine's screen (or Proxmox **Console**, or a BMC):

- [ ] `systemctl status ssh` (Debian/Ubuntu) or `systemctl status sshd` (Fedora/Arch).
- [ ] `ip -br a` for the address it actually has.
- [ ] `sudo ufw status` or `sudo nft list ruleset` for a rule that blocks port 22.
- [ ] `journalctl -u ssh -n 50` for why logins are refused (`AllowUsers`, bad permissions on `~/.ssh`).

## Part 4 · Windows hosts

- [ ] OpenSSH for Windows logs administrators in with keys from `C:\ProgramData\ssh\administrators_authorized_keys`, not `~\.ssh\authorized_keys`, and ignores that file if other users can read it. See [Add a Windows Server host](kb:howto/windows-hosts).
- [ ] The terminal opens cmd or PowerShell. Empty Overview numbers, Docker and System panels for that host are expected: those read Linux tools.

## Part 5 · Away from home

- [ ] Is **Built-in Tailscale** connected, and is the subnet route approved?
- [ ] Does the host have a **Tailnet address** fallback in its editor?

## Limits and gotchas

- `~/.ssh` must be `700` and `authorized_keys` `600`, owned by the user, or sshd ignores the key.
- A firewall change can lock you out. Use [Change a firewall safely](kb:howto/firewall) which rolls back on its own.
- **Logs**: Settings → General → Diagnostics shows the log folder. Failed connections are written there with the reason, so an exported diagnostics file says why a host won't connect. `NIGHTGRID_LOG=debug` adds detail.

## Related

- [Add your first host](kb:howto/first-host)
- [Add a Windows Server host](kb:howto/windows-hosts)
- [Reach your servers away from home](kb:howto/remote-access)
- [Change a firewall safely](kb:howto/firewall)
- [Hardening sshd](kb:basics/sshd-hardening)
