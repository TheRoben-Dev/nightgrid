# Family cloud and photos

Host Immich (photo backup from phones) and optionally Nextcloud (files, calendars, contacts) on your Proxmox node, behind Caddy with real certificates. Nightgrid handles the stacks, secrets, certificates and backups, and test-restores the backups so you know the family photos can actually be recovered.

## What you end up with

- A ZFS dataset holding the photos and files, separate from the app containers
- A Docker guest running Immich and/or Nextcloud as compose stacks
- Caddy in front, with HTTPS routes you edit from Docker → **Routes**
- Database passwords in the stack **Vault**, never in compose files
- Nightly restic backups, one copy off-site, shown in Fleet → **Backups** with regular **Test restore**
- Alerts when a certificate, a backup or the disk pool goes wrong

## Before you start

- Hardware: storage is the main cost. Plan for at least 2x the current photo library on a mirrored ZFS pool, plus an off-site target (a second box at a relative's house, or a cloud restic target such as S3-compatible storage).
- Immich's machine learning (face and object search) runs on CPU but is much faster with a GPU. An Intel iGPU works for its video transcoding, but Nightgrid monitoring doesn't read Intel GPUs yet.
- A domain name you control, if you want phones to sync away from home without a VPN. Otherwise use Tailscale and skip public exposure entirely, which is the safer choice.
- Settings → **FEATURES**: turn on **Proxmox** (the **Standard** preset leaves it out).

## Part 1 · Storage

- [ ] On the node, create datasets for the data (the node's Terminal). Use your pool's name instead of `tank`:

```sh
zfs create -o compression=lz4 -o atime=off tank/family
```

```sh
zfs create tank/family/photos
```

```sh
zfs create tank/family/cloud
```

- [ ] Decide on encryption before the first command: it can't be added to an existing dataset. If the box could be stolen, create `tank/family` with `-o encryption=on -o keyformat=passphrase` added. You then have to unlock it with `zfs load-key` after every reboot before the stacks start, which is a manual step after power loss.
- [ ] Check [Fleet](view:fleet) → **Disks**: the pool shows its health, and the mount gets a **full in about N days** forecast after a day of monitoring.

#### Check

- [ ] `zfs list -r tank/family` shows the three datasets.

## Part 2 · The Docker guest

- [ ] [Proxmox](view:proxmox) → **GUESTS** → **+ Container**, Debian 12, unprivileged, with your SSH key. 4 cores and 8 GB RAM suits Immich with a few users.
- [ ] Turn on nesting for Docker, and bind-mount the datasets into the container:

```sh
pct set <ctid> --features nesting=1,keyctl=1
```

```sh
pct set <ctid> --mp0 /tank/family/photos,mp=/srv/photos --mp1 /tank/family/cloud,mp=/srv/cloud
```

- [ ] Unprivileged containers shift user ids by 100000. Give the in-container user (uid 1000) ownership of the data:

```sh
chown -R 101000:101000 /tank/family/photos /tank/family/cloud
```

- [ ] Add the container as a host in Nightgrid, install Docker on it from Docker's Debian repository, and turn on **start at boot** in its **HARDWARE** boot options.

#### Check

- [ ] In a Terminal on the container, `ls -ln /srv/photos` shows uid 1000.

## Part 3 · Immich and Nextcloud stacks

- [ ] In [Docker](view:docker), create a stack folder for Immich and paste Immich's official `docker-compose.yml` and `.env` example into it with **+ New stack** (from pasted YAML). Use their files for the version you install; the layout changes between releases.
- [ ] Point `UPLOAD_LOCATION` at `/srv/photos`, and keep the database folder on the container's own disk (fast and snapshotted with the guest).
- [ ] Move `DB_PASSWORD` and any other secrets into the stack's **Vault**, then **Deploy .env…** and **Up now**.
- [ ] Nextcloud (optional): the same pattern, with its data folder on `/srv/cloud` and its database password in the Vault.
- [ ] Bind app ports to localhost only, so they're reachable only through Caddy (`127.0.0.1:2283:2283` for Immich).
- [ ] DRIFT can review a compose file: it flags missing restart policies, `:latest` tags, ports on 0.0.0.0 and inline secrets.

#### Check

- [ ] Both stacks are **up**, and `curl -s http://127.0.0.1:2283/api/server/ping` on the container answers `pong`.

## Part 4 · Caddy and certificates

- [ ] Run Caddy as its own stack on the same container (official `caddy` image, ports 80 and 443, a volume for `/data`).
- [ ] Docker → **Routes** → **Edit routes…** → **Add site…**: `photos.example.com` → `immich-server:2283` (or `127.0.0.1:2283` if Caddy uses host networking). Nightgrid shows the change as a diff, then **Save and reload Caddy**. Caddy checks the file and keeps the old one if it can't load it.
- [ ] Forward only 80 and 443 to this container on your router, or skip public exposure and use Tailscale.
- [ ] [Services](view:services): add a check for each site. Certificate expiry shows automatically for HTTPS services, and **Expiry watch** lists them soonest first. Add your domain's renewal date there too.

#### Check

- [ ] The phone app connects to `https://photos.example.com` and starts backing up.
- [ ] Routes shows the certificate's days left.

## Part 5 · Backups that prove themselves

- [ ] The rule is 3-2-1: the live data, a local restic repository on a different disk, and one copy off-site.
- [ ] Install restic on the node and create a systemd timer that backs up `/tank/family` plus a database dump. [System](view:system) → Schedules → **+ Timer** creates the timer from a command and an `OnCalendar` schedule.
- [ ] Dump the database first, since copying a live Postgres folder isn't a reliable backup. Run this on the container (the container name comes from Immich's compose file):

```sh
docker exec -t immich_postgres pg_dumpall --clean --if-exists --username=postgres > /srv/photos/db-dump.sql
```

- [ ] Keep the restic password in a root-only file (`/root/.restic-pass`, mode 600) referenced with `RESTIC_PASSWORD_FILE`, never on the command line.
- [ ] [Fleet](view:fleet) → **Backups**: pick the node and **Find jobs**. The timer appears with last run, size and result.
- [ ] **Test restore** restores one random file from the latest snapshot into `/tmp`, checks it, shows its SHA-256 and deletes it. Run it monthly.
- [ ] Off-site: a second restic repository on a remote target, run by a second timer. Nightgrid shows it as another job.

#### Check

- [ ] Both jobs show a recent successful run, and the last **Test restore** passed.

## Part 6 · Alerts and dependencies

- [ ] Settings → **Dependencies**: link the photo and cloud service checks to the container host and the container to the node, so one outage raises one alert.
- [ ] Settings → **Alerts**: set **FILLING UP** to a window long enough to buy disks (for example 60 days).
- [ ] For the backup jobs, a stale or failed run raises an alert while Nightgrid runs.

#### Check

- [ ] Stopping the Caddy stack raises one **service down** alert per site, folded under the cause.

## When Nightgrid is closed

- Keeps running: the stacks and their restart policies, Caddy's certificate renewal, the restic timers, ZFS scrubs from the node's cron, the phones' background uploads.
- Needs Nightgrid open (or in the tray): service checks, certificate and backup alerts, Test restore, uptime history.

## Limits and gotchas

- An encrypted dataset needs `zfs load-key` after every reboot. Without it the stacks start with empty folders. Document that as a runbook note pinned to the node.
- Immich releases sometimes need manual migration steps. Read the release notes and take a guest snapshot in [Proxmox](view:proxmox) before updating.
- Nightgrid never runs a backup itself. The timers do, and Nightgrid only reads them.
- A restic password lost is a backup lost. Keep it somewhere outside the server, such as a password manager.
- Exposing Nextcloud publicly makes it a target. Keep it updated, or reach it only over Tailscale.

## Related

- Proxmox media server
- Game server host
- Repair bench server
- Local AI GPU server
- Smart home hub
- Remote family IT
- Dev and git deploy lab
