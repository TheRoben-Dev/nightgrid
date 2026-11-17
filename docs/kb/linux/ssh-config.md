# Organise SSH with ~/.ssh/config

One file turns `ssh -i ~/.ssh/lab_ed25519 -p 2222 -J admin@gateway admin@10.0.0.15` into `ssh nas`. Nightgrid imports the same file, so a tidy config pays off twice.

![A short name expands to the full connection.](fig:lx-ssh-config)

## What you end up with

- Short names for every server, with user, port, key and jump host
- Shared defaults for whole groups of hosts
- Connections that reuse one TCP session and survive flaky Wi-Fi

## Part 1 · The basics

```
Host gateway
  HostName gateway.example.lan
  User admin

Host nas
  HostName 10.0.0.15
  User admin
  Port 2222
  IdentityFile ~/.ssh/lab_ed25519
  IdentitiesOnly yes
  ProxyJump gateway
```

- [ ] `IdentitiesOnly yes` stops SSH from offering every key in your agent (servers drop you after too many tries).
- [ ] The first matching value wins, so put specific hosts above general patterns.

## Part 2 · Defaults for groups

```
Host lab-*
  User admin
  IdentityFile ~/.ssh/lab_ed25519

Host *
  ServerAliveInterval 30
  ServerAliveCountMax 4
  AddKeysToAgent yes
```

- [ ] `ServerAliveInterval` keeps connections alive through NAT and notices dead ones within two minutes.

## Part 3 · Faster repeated connections

```
Host *
  ControlMaster auto
  ControlPath ~/.ssh/cm-%r@%h:%p
  ControlPersist 10m
```

- [ ] The second `ssh nas` within ten minutes reuses the first connection and opens instantly. Good for scripts and Ansible.

#### Check

- [ ] `ssh -G nas | grep -E "^(user|port|hostname|proxyjump) "` prints the values SSH will actually use.

## In Nightgrid

- **IMP** in the sidebar imports hosts from this file, `ProxyJump` included; the file is only read, never written (see [Import hosts and use jump hosts](kb:howto/jump-hosts)).
- **Settings → Connections → SSH keys** shows which key works where, if you'd rather keep keys in Nightgrid.

## Limits and gotchas

- Windows' OpenSSH uses `%USERPROFILE%\.ssh\config` and doesn't support `ControlMaster`.
- `Include ~/.ssh/config.d/*` keeps big configs tidy; it must come before any `Host` line to apply globally.

## Related

- [SSH keys and agents](kb:basics/ssh-keys)
- [Import hosts and use jump hosts](kb:howto/jump-hosts)
- [Forward ports through SSH tunnels](kb:linux/ssh-tunnels)
