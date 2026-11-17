# Backrest: a web UI for restic

Manage restic repositories, schedules, retention and restores from a web UI, with hooks for notifications. Handy for people who prefer clicking to systemd units.

![Backrest schedules restic to local and offsite repositories.](fig:app-backup-repo)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `backrest`:

```yaml
services:
  backrest:
    image: garethgeorge/backrest:latest
    restart: unless-stopped
    hostname: myhost
    environment:
      BACKREST_DATA: /data
      BACKREST_CONFIG: /config/config.json
      XDG_CACHE_HOME: /cache
      TZ: Etc/UTC
    volumes:
      - ./data:/data
      - ./config:/config
      - ./cache:/cache
      - /etc:/userdata/etc:ro
      - /opt/stacks:/userdata/stacks:ro
      - /srv/backups:/repos
    ports:
      - "127.0.0.1:9898:9898"
```

- [ ] Open the UI through a saved tunnel in [Tunnels](view:tunnels); set a login.
- [ ] Add a repository (local `/repos/myhost`, SFTP or S3) and a plan with the `/userdata/...` paths, a schedule and retention.

## Part 2 · With Nightgrid

- [ ] Add the repository folder as a path job in [Fleet](view:fleet) → **Backups** so a stale repository alerts.
- [ ] Backrest's hooks can call ntfy or Healthchecks on success and failure.

#### Check

- [ ] A plan's first run succeeds, and a restore of one file works from the UI.

## Limits and gotchas

- Read-only mounts protect sources; add each folder you back up as its own mount.
- Keep the repository password somewhere other than this host.

## Related

- [restic backups with systemd timers](kb:apps/restic)
- [Healthchecks dead man's switch](kb:apps/healthchecks)
