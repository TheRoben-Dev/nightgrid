# Forward ports through SSH tunnels

Three kinds of SSH forward cover most "I need to reach that port from here" problems, with nothing opened on a firewall. This is plain OpenSSH, so it works from any machine, Nightgrid or not.

![Local, remote and dynamic forwards side by side.](fig:lx-ssh-tunnels)

## What you end up with

- A remote admin page or database on a local port (`-L`)
- A local service reachable from a server (`-R`)
- A SOCKS proxy to browse a remote LAN (`-D`)
- Tunnels that stay up on their own with systemd

## Part 1 · Local forward

- [ ] Reach a database that listens only on the server's localhost:

```sh
ssh -N -L 15432:localhost:5432 user@server
```

- [ ] Then connect to `localhost:15432` here. The middle part is resolved on the server, so `-L 8443:192.168.1.1:443` reaches the router behind it.

## Part 2 · Remote forward

- [ ] Let a server reach a service on this machine:

```sh
ssh -N -R 9000:localhost:3000 user@server
```

- [ ] The server's `localhost:9000` now leads here. Binding it on other addresses needs `GatewayPorts clientspecified` in the server's sshd config; usually you don't want that.

## Part 3 · Dynamic (SOCKS)

```sh
ssh -N -D 1080 user@server
```

- [ ] Set your browser's SOCKS5 proxy to `localhost:1080` (with "proxy DNS" on) and it browses as if it were on the server's network.

## Part 4 · Keep one up

- [ ] Put the options in `~/.ssh/config` so the tunnel is one word:

```
Host db-tunnel
  HostName server.example.lan
  User admin
  LocalForward 15432 localhost:5432
  ServerAliveInterval 30
  ExitOnForwardFailure yes
```

- [ ] On a server that should hold a tunnel permanently, a user unit restarts it on failure (`~/.config/systemd/user/db-tunnel.service`):

```ini
[Service]
ExecStart=/usr/bin/ssh -N db-tunnel
Restart=always
RestartSec=10

[Install]
WantedBy=default.target
```

- [ ] `systemctl --user enable --now db-tunnel` and `sudo loginctl enable-linger $USER` so it runs without a login.

#### Check

- [ ] `ss -ltn | grep 15432` shows the listener, and the app connects through it.

## In Nightgrid

- [Tunnels](view:tunnels) does the same three kinds with saved tunnels that start with the app and come back after reconnects; see [Reach admin web UIs through SSH tunnels](kb:howto/tunnels).
- Use plain `ssh` tunnels for things that must run while Nightgrid is closed, like a server-to-server forward.

## Limits and gotchas

- `-L 0.0.0.0:...` exposes the forward to your whole network; leave the bind address out to keep it on localhost.
- TCP only. UDP (DNS, game servers, WireGuard) needs another approach.
- Tunnels through a jump host: add `-J jump-host`, or `ProxyJump` in the config.

## Related

- [Reach admin web UIs through SSH tunnels](kb:howto/tunnels)
- [Import hosts and use jump hosts](kb:howto/jump-hosts)
- [SSH keys and agents](kb:basics/ssh-keys)
