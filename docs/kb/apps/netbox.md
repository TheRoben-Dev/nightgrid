# NetBox network documentation

The source of truth for your network: sites, racks, devices, cables, IP ranges, VLANs and which port goes where. Overkill for three machines, excellent once you have a rack, several VLANs or a shop network to document.

## What you end up with

- Every subnet, VLAN and address with what uses it
- Devices in racks with their interfaces and cables
- An API scripts can query ("which IP is free in the server VLAN?")

## Part 1 · Install with netbox-docker

- [ ] On a host with 2 GB or more of free RAM:

```sh
git clone -b release https://github.com/netbox-community/netbox-docker.git /opt/stacks/netbox
cd /opt/stacks/netbox
cat > docker-compose.override.yml <<'YML'
services:
  netbox:
    ports:
      - "8000:8080"
YML
docker compose pull
docker compose up -d
docker compose exec netbox /opt/netbox/netbox/manage.py createsuperuser
```

- [ ] First start runs database migrations and takes a few minutes; follow it in the logs.
- [ ] Change the passwords and secret key in the `env/*.env` files before real use.

## Part 2 · Model your network

- [ ] Order matters: Sites → Racks (optional) → Manufacturers → Device types → Devices.
- [ ] IPAM: add your prefixes (`192.168.1.0/24`, VLAN subnets) and VLANs; then IP addresses assigned to device interfaces.
- [ ] Use the community device type library to import switch and server models with all their ports.

#### Check

- [ ] Open a prefix: its utilisation and the next free address are shown.

## In Nightgrid

- It runs as a normal compose project, so [Docker](view:docker) → **Stacks** lists it with up, down, pull and restart. Keep the override file: it survives `git pull` updates of netbox-docker.
- Nightgrid's [Network](view:network) map shows what's actually on the wire; NetBox records what should be. Compare the two after a scan to find undocumented devices.

## Limits and gotchas

- Major NetBox upgrades can need intermediate versions; follow netbox-docker's release notes rather than jumping several versions.
- Keeping it current is the hard part. Document as you change things, not later.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [Snipe-IT asset inventory](kb:apps/snipe-it)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
