---
id: host-unreachable
title: "Runbook: host unreachable"
summary: Work out whether a host is down, off the network, or just refusing SSH.
alertKinds: [hostOffline]
tags: [runbook, starter, network]
library: [howto/troubleshoot-connection, errors/ssh, errors/network]
---
Nightgrid lost its connection to the host and couldn't get it back for two minutes. Work from the outside in: power, network, then SSH.

- [ ] Is it on? Check the power light, or its hypervisor or IPMI console. A UPS alert at the same time points at power.
- [ ] Does it answer on the network? From another host in [Network tools](view:network) or a terminal:

```sh
ping -c 3 HOST
nc -vz -w 3 HOST 22
```

- [ ] Ping works but port 22 doesn't: sshd stopped, or a firewall rule changed. Use the console to check `systemctl status ssh` (or `sshd`).
- [ ] Neither answers: cable, switch port, Wi-Fi, or its address changed (check your router's DHCP list). Over the tailnet, check the device is online in Tailscale.
- [ ] It answers but Nightgrid still can't log in: read the error in the host's connection test (key refused, host key changed, password expired).
- [ ] It came back after a reboot? Check why it rebooted:

```sh
last -x reboot | head -5
journalctl --list-boots | tail -3
```

More in [Troubleshoot a connection](kb:howto/troubleshoot-connection) and [SSH errors](kb:errors/ssh).
