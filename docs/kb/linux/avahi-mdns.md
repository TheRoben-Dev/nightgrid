# Reach servers by name.local (mDNS)

Make `nas.local` or `pi.local` work on your LAN without running a DNS server, and advertise services so phones and computers find them. Uses Avahi, the standard mDNS daemon on Linux; Macs and iPhones speak mDNS out of the box.

![How a .local name is found.](fig:lx-avahi)

## What you end up with

- Each Linux host answering to `<hostname>.local`
- Linux clients able to resolve `.local` names too
- Optional service announcements (SSH, SMB, a web UI) that show up in Finder and discovery apps

## Part 1 · Announce a host

- [ ] On the server: `sudo apt install avahi-daemon` (Fedora: `sudo dnf install avahi`), then `sudo systemctl enable --now avahi-daemon`.
- [ ] The name is the host's own hostname: `hostnamectl hostname` shows it, `sudo hostnamectl set-hostname nas` changes it.
- [ ] Allow mDNS through the firewall: UDP 5353 (ufw: `sudo ufw allow 5353/udp`; firewalld: `sudo firewall-cmd --permanent --add-service=mdns && sudo firewall-cmd --reload`).

## Part 2 · Resolve on Linux clients

- [ ] `sudo apt install libnss-mdns` (Fedora: `nss-mdns`). It adds `mdns4_minimal` to the `hosts:` line in `/etc/nsswitch.conf`.
- [ ] macOS and iOS need nothing; current Windows 10/11 resolves `.local` names in most apps.
- [ ] Test: `getent hosts nas.local` (goes through NSS like real programs) and `avahi-resolve -n nas.local`.

## Part 3 · Advertise services (optional)

- [ ] Drop an XML file in `/etc/avahi/services/`, such as `ssh.service`:

```xml
<?xml version="1.0" standalone='no'?>
<!DOCTYPE service-group SYSTEM "avahi-service.dtd">
<service-group>
  <name replace-wildcards="yes">%h</name>
  <service>
    <type>_ssh._tcp</type>
    <port>22</port>
  </service>
</service-group>
```

- [ ] `_smb._tcp` port 445 makes a Samba share appear in macOS Finder's sidebar; `_http._tcp` with a port advertises a web UI. Avahi picks up new files by itself.
- [ ] `avahi-browse -art` (from `avahi-utils`) lists everything announced on the LAN.

#### Check

- [ ] From another machine, `ping nas.local` resolves to the server's LAN address, and `avahi-browse -art` shows its services.

## In Nightgrid

- [Network](view:network) → **Scan from …** reads mDNS through `avahi-browse` on the scanning host, so announced devices and services show up on the map. Install `avahi-utils` there.
- You can use `nas.local` as a host's address in Nightgrid when the computer running Nightgrid is on the same LAN; over a tailnet use the tailnet name instead.

## Limits and gotchas

- mDNS doesn't cross subnets or VLANs on its own; the router needs an mDNS reflector or repeater (OpenWrt, OPNsense and UniFi all have one).
- Two machines with the same hostname get renamed `nas-2.local` silently. Unique hostnames avoid surprises.
- Don't use `.local` as the zone for a normal DNS server; see [Debug DNS on a Linux server](kb:linux/dns-debug).

## Related

- [Debug DNS on a Linux server](kb:linux/dns-debug)
- [Samba file shares](kb:apps/samba)
- [Map your network and find exposed ports](kb:howto/network-map)
