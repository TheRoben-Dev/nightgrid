# Recyclarr quality profiles for Sonarr and Radarr

Keep Sonarr and Radarr's quality profiles, custom formats and naming in sync with the community TRaSH Guides automatically, instead of clicking through dozens of settings by hand. Recyclarr reads a YAML file and applies it through the apps' APIs on a schedule.

![TRaSH profiles synced into Sonarr and Radarr.](fig:app-recyclarr)

## What you end up with

- Recyclarr in the same stack as your *arr apps, syncing daily
- TRaSH-recommended profiles (for example "WEB-1080p" or "Remux + WEB 2160p")
- Settings you can read, diff and keep in git

## Before you start

- Sonarr v4 and Radarr v5 or later running (see [The *arr stack](kb:apps/arr-stack)), with their API keys from **Settings → General**.

## Part 1 · Add it to the stack

- [ ] In the *arr stack's compose file, add:

```yaml
  recyclarr:
    image: ghcr.io/recyclarr/recyclarr:7
    restart: unless-stopped
    user: "1000:1000"
    environment:
      TZ: Etc/UTC
      CRON_SCHEDULE: "@daily"
      SONARR_API_KEY: ${SONARR_API_KEY}
      RADARR_API_KEY: ${RADARR_API_KEY}
    volumes:
      - ./recyclarr:/config
```

- [ ] Put both API keys in the stack's `.env`.

## Part 2 · The config

- [ ] List the ready-made templates and create a config from one:

```bash
docker compose run --rm recyclarr config list templates
docker compose run --rm recyclarr config create -t web-1080p-v4 -t hd-bluray-web
```

- [ ] Edit `recyclarr/recyclarr.yml`: set each `base_url` to the app's address inside the stack (`http://sonarr:8989`, `http://radarr:7878`) and use `!env_var SONARR_API_KEY` for `api_key`.
- [ ] Preview, then apply:

```bash
docker compose run --rm recyclarr sync --preview
docker compose run --rm recyclarr sync
```

#### Check

- [ ] Sonarr's **Settings → Profiles** shows the new profile with its custom formats and scores, and new downloads pick the release the profile prefers.

## In Nightgrid

- The `recyclarr.yml` file lives next to the stack; edit it in [Files](view:files) and run a sync from the [Terminal](view:terminal). With [Deploy stacks from git](kb:howto/stacks-from-git), it's versioned with the rest of the stack.
- Recyclarr's container logs in [Docker](view:docker) show each sync's changes.

## Limits and gotchas

- Recyclarr owns the profiles it manages: edits you make in the app's UI to those profiles get overwritten at the next sync. Make changes in the YAML.
- Assign the new profile to existing series and movies yourself (Mass Editor); Recyclarr doesn't change what items use.
- Major Recyclarr versions change the config format; read the upgrade notes before moving the image tag.

## Related

- [The *arr stack](kb:apps/arr-stack)
- [SABnzbd Usenet downloader](kb:apps/sabnzbd)
- [Jellyseerr media requests](kb:apps/jellyseerr)
