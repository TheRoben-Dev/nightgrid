# UniFi Network Application in Docker

Run the controller for Ubiquiti access points, switches and gateways on your own server instead of a Cloud Key. Devices keep working without it; you need it for changes, stats and firmware updates.

![The UniFi controller and its adopted devices.](fig:app-unifi)

## What you end up with

- The UniFi Network Application on port 8443 with its own MongoDB
- Adopted access points and switches
- Automatic config backups you can restore anywhere

## Part 1 · The stack

- [ ] An init script for the database user, `./init-mongo.sh` (make it executable):

```sh
#!/bin/bash
mongosh <<EOF2
use admin
db.auth("root", "$MONGO_INITDB_ROOT_PASSWORD")
db.getSiblingDB("unifi").createUser({user: "unifi", pwd: "$MONGO_PASS", roles: [{role: "dbOwner", db: "unifi"}, {role: "dbOwner", db: "unifi_stat"}]})
EOF2
```

- [ ] [Docker](view:docker) → **+ New stack** named `unifi`, passwords in `.env`:

```yaml
services:
  unifi:
    image: lscr.io/linuxserver/unifi-network-application:latest
    restart: unless-stopped
    depends_on: [mongo]
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
      MONGO_USER: unifi
      MONGO_PASS: ${MONGO_PASS}
      MONGO_HOST: mongo
      MONGO_PORT: "27017"
      MONGO_DBNAME: unifi
      MONGO_AUTHSOURCE: admin
    volumes:
      - ./config:/config
    ports:
      - "8443:8443"
      - "3478:3478/udp"
      - "10001:10001/udp"
      - "8080:8080"
  mongo:
    image: mongo:7.0
    restart: unless-stopped
    environment:
      MONGO_INITDB_ROOT_USERNAME: root
      MONGO_INITDB_ROOT_PASSWORD: ${MONGO_ROOT_PASS}
      MONGO_PASS: ${MONGO_PASS}
    volumes:
      - ./mongo:/data/db
      - ./init-mongo.sh:/docker-entrypoint-initdb.d/init-mongo.sh:ro
```

- [ ] Port 8080 is device inform, 3478 STUN, 10001 discovery. Open `https://host:8443` and run the setup.

## Part 2 · Adopt devices

- [ ] Settings → System → Advanced: set **Inform Host** to the server's LAN IP, otherwise devices get the container's internal address.
- [ ] Devices on the same subnet appear as "Ready to adopt". Others: SSH to the device (default `ubnt`/`ubnt` on a factory-reset device) and run `set-inform http://server-ip:8080/inform`.

## Part 3 · Backups

- [ ] Settings → System → Backups: auto backup weekly. The `.unf` files land in `./config/data/backup/autobackup`.

#### Check

- [ ] Every device shows **Online**, and a setting change (Wi-Fi name) reaches the access points.

## In Nightgrid

- Copy the autobackup folder off the host with your backup job; [Fleet](view:fleet) → **Backups** watches its age.
- The [Network](view:network) map's LLDP support shows which switch port a host is on; UniFi switches send LLDP.
- [Services](view:services) → **Add integration** → **UniFi Network** (9.0 or newer) with an API key from Settings → Control Plane → Integrations: devices (offline first), clients, and **Restart** for a device (see [Add app cards](kb:howto/integrations)).

## Limits and gotchas

- MongoDB 5 and newer need a CPU with AVX. Old Celeron and Atom hosts must use `mongo:4.4`.
- The init script only runs on an empty database folder. Changing passwords later means changing them inside MongoDB too.
- Don't run two controllers for the same devices; they fight over adoption.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [OPNsense firewall in a VM](kb:apps/opnsense)
- [Monitor backups and test restores](kb:howto/backups)
