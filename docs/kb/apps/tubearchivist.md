# Tube Archivist video archive

Download and index YouTube channels and playlists you want to keep, with a web player, search, and watched status. Uses yt-dlp under the hood, with Elasticsearch for search.

![Tube Archivist archiving channels.](fig:app-video-archive)

## What you end up with

- Subscribed channels downloaded on a schedule
- A searchable library with a player and watch progress
- Files in a folder Jellyfin or Plex can also read (with a plugin)

## Before you start

- 4 GB of RAM or more (Elasticsearch is the hungry part)
- Plenty of disk: video adds up quickly

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `tubearchivist`, passwords in `.env`:

```yaml
services:
  tubearchivist:
    image: bbilly1/tubearchivist:latest
    restart: unless-stopped
    depends_on: [archivist-es, archivist-redis]
    ports:
      - "8000:8000"
    environment:
      ES_URL: http://archivist-es:9200
      REDIS_CON: redis://archivist-redis:6379
      HOST_UID: "1000"
      HOST_GID: "1000"
      TA_HOST: http://tube.lan:8000
      TA_USERNAME: admin
      TA_PASSWORD: ${TA_PASSWORD}
      ELASTIC_PASSWORD: ${ES_PASSWORD}
      TZ: Etc/UTC
    volumes:
      - /srv/youtube:/youtube
      - ./cache:/cache
  archivist-redis:
    image: redis:7
    restart: unless-stopped
    volumes:
      - ./redis:/data
  archivist-es:
    image: bbilly1/tubearchivist-es:latest
    restart: unless-stopped
    environment:
      ELASTIC_PASSWORD: ${ES_PASSWORD}
      ES_JAVA_OPTS: "-Xms1g -Xmx1g"
      xpack.security.enabled: "true"
      discovery.type: single-node
      path.repo: /usr/share/elasticsearch/data/snapshot
    volumes:
      - ./es:/usr/share/elasticsearch/data
```

- [ ] Elasticsearch needs `vm.max_map_count` raised on the host: `echo vm.max_map_count=262144 | sudo tee /etc/sysctl.d/99-es.conf && sudo sysctl --system`.
- [ ] `./es` must be writable by uid 1000: `sudo chown -R 1000:1000 es`.

## Part 2 · Subscribe and download

- [ ] Channels → subscribe by URL; Settings → Scheduling: how often to look for new videos and when to download.
- [ ] Downloads → add single videos or playlists; the queue shows progress.

#### Check

- [ ] A downloaded video plays in the web player, and search finds it by a word from its title.

## In Nightgrid

- Watch `/srv/youtube` grow in [Fleet](view:fleet) → **Disks** with the full-in-N-days forecast.
- Elasticsearch failing to start is almost always the `max_map_count` or folder permissions; its log in [Docker](view:docker) says which.

## Limits and gotchas

- Archive only what you're allowed to keep under the site's terms and your local law.
- YouTube changes break downloads from time to time; updating the image (newer yt-dlp) usually fixes it.

## Related

- [Jellyfin media server](kb:apps/jellyfin)
- [Keep an eye on disks and pools](kb:howto/disks)
- [The *arr stack](kb:apps/arr-stack)
