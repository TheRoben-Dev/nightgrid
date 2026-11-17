# LibreNMS network monitoring

Auto-discovering SNMP monitoring for switches, routers, access points, firewalls, printers and UPSes: port traffic graphs for every interface, device health (CPU, temperature, PSU, fans), neighbours via LLDP/CDP and alerts when a port flaps or a device stops answering.

## What you end up with

- LibreNMS with MariaDB, Redis and its poller in Docker
- SNMP enabled on your network gear and discovered automatically
- Traffic graphs per switch port and alerts by email or webhook

## Part 1 · The stack

- [ ] LibreNMS publishes a maintained compose example. In a terminal on the host:

```bash
git clone https://github.com/librenms/docker.git /opt/stacks/librenms-src
cd /opt/stacks/librenms-src/examples/compose
```

- [ ] Edit `.env` (`TZ`, `MYSQL_PASSWORD` to a long random value) and `librenms.env` if you need to (base URL, memory limits). Then:

```bash
docker compose up -d
```

- [ ] Open `http://<host>:8000` and create the admin user in the installer.

## Part 2 · SNMP on your devices

- [ ] On each switch, router, AP or UPS card: enable **SNMP v2c** with a read-only community that isn't `public`, limited to the LibreNMS host's IP, or **SNMP v3** (authPriv) if the device supports it.
- [ ] Linux servers can be added too: `sudo apt install snmpd`, set `rocommunity <community> <librenms-ip>` and `agentaddress udp:161` in `/etc/snmp/snmpd.conf`, restart `snmpd`.

## Part 3 · Add and discover

- [ ] **Devices → Add Device**: hostname or IP, SNMP version and community or v3 credentials.
- [ ] **Settings → Discovery**: add your LAN ranges under **Autodiscovery networks** so devices found via LLDP/CDP/ARP are added automatically.
- [ ] Discovery runs every 6 hours and polling every 5 minutes; the first graphs appear after two polls.

## Part 4 · Alerts

- [ ] **Alerts → Alert Transports**: email, a webhook, ntfy, Matrix, Mattermost and many more.
- [ ] **Alerts → Alert Rules → Create rule from collection**: start with *Devices up/down*, *Port utilisation over threshold* and *State sensor critical*.

#### Check

- [ ] A switch shows every port with traffic graphs, and unplugging a cable from a monitored port raises a port-down event.

## In Nightgrid

- Nightgrid's [Network](view:network) map shows which switch port a host is plugged into (via `lldpd` on the scanning host); LibreNMS covers devices that only speak SNMP, with long-term port history.
- Add `http://<host>:8000` to [Services](view:services), and the switches LibreNMS polls as dependencies of the hosts behind them in **Settings → Alerts → Dependencies**.

## Limits and gotchas

- SNMP v2c sends the community in clear text. Keep it on a management VLAN or use v3.
- RRD files grow per port: a 48-port switch is a few hundred MB over a year. Plan disk accordingly.
- Some consumer "smart" switches have very limited SNMP; check that interface counters are exposed before buying for this.

## Related

- [Zabbix monitoring](kb:apps/zabbix)
- [ntopng traffic analysis](kb:apps/ntopng)
- [NetBox network documentation](kb:apps/netbox)
