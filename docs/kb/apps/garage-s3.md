# Garage S3 storage

A light S3-compatible object store for backups and apps that want S3 (restic, Kopia, rclone, Loki, Immich alternatives), with optional replication across several machines.

## Part 1 · Config and stack

- [ ] `/opt/stacks/garage/garage.toml`:

```toml
metadata_dir = "/var/lib/garage/meta"
data_dir = "/var/lib/garage/data"
db_engine = "sqlite"
replication_factor = 1
rpc_bind_addr = "[::]:3901"
rpc_secret = "<openssl rand -hex 32>"

[s3_api]
s3_region = "garage"
api_bind_addr = "[::]:3900"
root_domain = ".s3.example.lan"
```

- [ ] [Docker](view:docker) → **+ New stack** named `garage`:

```yaml
services:
  garage:
    image: dxflrs/garage:v1.1.0
    restart: unless-stopped
    ports:
      - "3900:3900"
    volumes:
      - ./garage.toml:/etc/garage.toml
      - ./meta:/var/lib/garage/meta
      - /srv/s3:/var/lib/garage/data
```

- [ ] Use the newest version tag from Garage's releases.

## Part 2 · Layout, bucket and key

```sh
alias garage='docker exec -it garage-garage-1 /garage'
garage status
garage layout assign -z home -c 1T <node-id>
garage layout apply --version 1
garage bucket create backups
garage key create backup-key
garage bucket allow --read --write backups --key backup-key
```

- [ ] Use the key id and secret in restic (`s3:http://s3.example.lan:3900/backups`) or rclone.

#### Check

- [ ] `rclone ls` against the bucket works with the new key.

## Limits and gotchas

- Put TLS in front (a reverse proxy) if anything outside the host uses it.
- One node means no redundancy; that's fine for a backup target that's itself a second copy.

## Related

- [restic backups with systemd timers](kb:apps/restic)
- [Offsite copies with rclone](kb:apps/rclone)
