# Manage a server with IPMI

Server boards from Supermicro, Dell (iDRAC), HPE (iLO), Lenovo (XCC) and ASRock Rack have a small management controller (BMC) on its own network port. It gives you power control, sensors and a console even when the OS is off or hung.

![The BMC on its own management network.](fig:lx-ipmi)

## What you end up with

- Power on, off and reset from the command line
- Sensor readings and the event log (SEL) for fans, PSUs and ECC errors
- A serial console over the network (Serial over LAN)

## Before you start

- The BMC's network port on a management VLAN or at least a trusted LAN. Never on the internet.
- `sudo apt install ipmitool` on the machine you manage from
- Change the default BMC password before anything else

## Part 1 · Find and secure the BMC

- [ ] From the server's own OS, read and set the BMC's network:

```sh
sudo ipmitool lan print 1
sudo ipmitool lan set 1 ipsrc static
sudo ipmitool lan set 1 ipaddr 192.168.10.20
sudo ipmitool lan set 1 netmask 255.255.255.0
```

- [ ] List users and set a strong password on the admin user (ID from the list):

```sh
sudo ipmitool user list 1
sudo ipmitool user set password 2
```

## Part 2 · Remote control

```sh
ipmitool -I lanplus -H 192.168.10.20 -U admin -E chassis power status
ipmitool -I lanplus -H 192.168.10.20 -U admin -E chassis power on
ipmitool -I lanplus -H 192.168.10.20 -U admin -E sdr elist
ipmitool -I lanplus -H 192.168.10.20 -U admin -E sel elist | tail
```

- [ ] `-E` reads the password from the `IPMI_PASSWORD` environment variable, so it isn't in your shell history.
- [ ] `chassis power soft` asks the OS to shut down cleanly; `off` pulls the plug.

## Part 3 · Serial over LAN

- [ ] Point the kernel console at the serial port the BMC redirects (often `ttyS1`): add `console=tty0 console=ttyS1,115200n8` to `GRUB_CMDLINE_LINUX` in `/etc/default/grub`, then `sudo update-grub`.
- [ ] Connect: `ipmitool -I lanplus -H 192.168.10.20 -U admin -E sol activate` (leave with `~.`).

#### Check

- [ ] `chassis power status` answers from another machine, and the SEL has no unexpected ECC or PSU events.

## In Nightgrid

- Nightgrid's own Wake-on-LAN and power buttons work through the OS; use IPMI for the cases they can't reach (a hung or powered-off server).
- Keep the ipmitool commands in a [Knowledge](view:notes) runbook with **Send to terminal** from a host on the management network.
- Reach a BMC's web UI from away through [Tunnels](view:tunnels) via a host on that network, instead of exposing it.

## Limits and gotchas

- Old BMC firmware has serious security holes. Update it, keep it off untrusted networks, and turn off IPMI over LAN if you only use the web UI.
- The HTML5 or Java KVM console is in the web UI; ipmitool doesn't do video.
- Some boards share the BMC with a normal NIC port ("shared LAN"); traffic then mixes with the host's.

## Related

- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [Forward ports through SSH tunnels](kb:linux/ssh-tunnels)
- [Burn in a server: CPU, memory and stability](kb:linux/burn-in)
