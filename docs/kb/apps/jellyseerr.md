# Jellyseerr media requests

Let family request films and shows from a friendly UI; requests go to Sonarr and Radarr automatically, and requesters get told when it's ready. Works with Jellyfin (and Plex/Emby).

![From a family request to a ready title.](fig:app-requests)

## Part 1 · The stack

- [ ] Add to your `arr` stack or a new one:

```yaml
services:
  jellyseerr:
    image: fallenbagel/jellyseerr:latest
    restart: unless-stopped
    environment:
      TZ: Etc/UTC
    volumes:
      - ./jellyseerr:/app/config
    networks: [default, proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `requests.example.com` to `jellyseerr:5055`.

## Part 2 · Setup wizard

- [ ] Sign in with a Jellyfin admin account and pick the libraries.
- [ ] Add Radarr and Sonarr with their API keys and container names (`radarr:7878`, `sonarr:8989`) when they share a Docker network, with root folders and quality profiles.
- [ ] Users sign in with their Jellyfin accounts; set request limits per user.

#### Check

- [ ] A request appears as monitored in Radarr and downloads.

## Limits and gotchas

- If the *arr apps are in another stack, attach Jellyseerr to that stack's network too, or use the host's LAN IP and ports.
- The Overseerr and Jellyseerr projects are merging; check which image is current before installing.

## Related

- [The *arr stack](kb:apps/arr-stack)
- [Jellyfin media server](kb:apps/jellyfin)
