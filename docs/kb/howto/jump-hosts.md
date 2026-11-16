# Import hosts and use jump hosts

Bring your existing `~/.ssh/config` into Nightgrid and reach machines that are only visible from another server, such as hosts on a home LAN behind one always-on box.

## What you end up with

- Every host from your SSH config in the sidebar, with users, ports, keys and `ProxyJump` kept
- Hosts behind a jump host that connect in one click, through the jump host's SSH connection
- Your SSH config untouched: Nightgrid only reads it

## Before you start

- A working `~/.ssh/config`, or at least one server that can reach the others (the jump host).
- Your public key authorized on the jump host and on each host behind it. The private key stays on this computer; the jump host never sees it.

## Part 1 · Import

- [ ] Press **IMP** in the sidebar. Nightgrid lists the `Host` entries it found, with wildcard entries applied.
- [ ] Untick the ones you don't want and import.
- [ ] Open each imported host once to accept its host key.

## Part 2 · A jump host by hand

A typical layout: this computer reaches `gateway` (over the internet or a tailnet), and `gateway` reaches `nas` and `media` on its LAN.

- [ ] Add the jump host first as a normal host.
- [ ] For each host behind it, save its LAN address and set the jump host in the host editor (it's the same as `ProxyJump gateway` in SSH config).
- [ ] Put your key on the inner hosts: **Settings → SSH keys → CHECK HOSTS** works through the jump host too.

The equivalent SSH config, if you prefer to keep it there and import:

```
Host gateway
    HostName gateway.example.com
    User admin

Host nas
    HostName 192.168.1.20
    User admin
    ProxyJump gateway
```

#### Check

- [ ] The inner host connects and shows on the [Overview](view:overview).
- [ ] [Files](view:files) and [Docker](view:docker) work on it like on any host.

## Part 3 · Or skip the jump host

- [ ] If the jump host runs Tailscale, advertise the LAN as a subnet route and turn on **Use subnet routes** in **Settings → Tailnet**. Hosts saved with LAN addresses then work at home and away (see [Reach your servers away from home](kb:howto/remote-access)).

## Limits and gotchas

- Agent forwarding is not needed and not used; keys never leave this computer.
- If the jump host is down, everything behind it is too. Add a dependency in **Settings → Dependencies** so their alerts fold under the jump host's.
- `Match` blocks and `ProxyCommand` are not imported; rewrite those as `ProxyJump` where you can.

## Related

- [Add your first host](kb:howto/first-host)
- [Reach your servers away from home](kb:howto/remote-access)
- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
