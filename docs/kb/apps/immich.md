# Immich photo backup

Back up phone photos and videos automatically to your own server, with face recognition, search and shared albums. A self-hosted alternative to Google Photos.

## What you end up with

- Phones uploading in the background with the Immich app
- Machine-learning search ("dog on the beach") running locally
- A library folder and database you back up nightly

## Before you start

- 4 GB RAM minimum (6+ comfortable), and an x86-64 or arm64 host.
- Storage for the library; it grows quickly with video.
- The full family setup with Nextcloud, sharing and remote access is in [Family cloud and photos](kb:guides/family-cloud-photos).

## Part 1 · Install

- [ ] Immich publishes a `docker-compose.yml` and `example.env` with each release. Download both into `/opt/stacks/immich` with the commands in Immich's install docs (they come from its GitHub release page).
- [ ] Rename `example.env` to `.env` and set:

```sh
UPLOAD_LOCATION=/tank/photos/library
DB_DATA_LOCATION=/opt/stacks/immich/postgres
IMMICH_VERSION=release
DB_PASSWORD=<random letters and digits>
```

- [ ] Use **Import from host .env** in the stack's **Vault** so the password is kept encrypted in Nightgrid too.
- [ ] Bring the stack up in [Docker](view:docker). The web UI is on port 2283.
- [ ] Route `photos.example.com` to `immich-server:2283`, and raise the proxy's body size limit for large videos (Caddy has none by default).

## Part 2 · Phones

- [ ] Create the admin account in the web UI, then users for the family.
- [ ] Immich app → server URL → log in → enable background backup for the camera roll.
- [ ] Hardware acceleration for transcoding and ML: Immich ships `hwaccel` compose extensions for Intel QSV, NVIDIA and others.

## Part 3 · Backups

- [ ] Immich writes nightly database dumps to `UPLOAD_LOCATION/backups` by itself. Back up the whole `UPLOAD_LOCATION` with restic or borg, and add the job in [Fleet](view:fleet) → **Backups**.
- [ ] Don't back up `DB_DATA_LOCATION` directly while running; the dumps are what you restore.

#### Check

- [ ] A photo from the phone shows up in the web UI within minutes, and Fleet → Backups is green.

## Limits and gotchas

- Immich moves fast. Read the release notes before every update; some need compose changes. Pin `IMMICH_VERSION` to a version like `v1.x.y`.
- The database must be on a local SSD, not a network share.
- Keep originals elsewhere too until you trust it. Immich is a backup target, not your only copy.

## Related

- [Family cloud and photos](kb:guides/family-cloud-photos)
- [restic backups with systemd timers](kb:apps/restic)
- [Update containers without fear](kb:howto/container-updates)
