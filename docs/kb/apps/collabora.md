# Collabora Online for Nextcloud

Edit Word, Excel and PowerPoint files in the browser inside Nextcloud, with several people at once, on your own server.

![Nextcloud and Collabora working together.](fig:app-collabora)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `collabora`:

```yaml
services:
  collabora:
    image: collabora/code:latest
    restart: unless-stopped
    cap_add: [MKNOD]
    environment:
      aliasgroup1: https://cloud.example.com:443
      extra_params: --o:ssl.enable=false --o:ssl.termination=true
      username: admin
      password: ${ADMIN_PASSWORD}
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `office.example.com` to `collabora:9980` with websockets (Caddy passes them by default).

## Part 2 · Connect Nextcloud

- [ ] In Nextcloud, install the **Nextcloud Office** app. Administration → Office → use your own server: `https://office.example.com`.
- [ ] If Nextcloud and Collabora can't reach each other by public name from inside Docker, add the WOPI allow list in the Office settings and check DNS from inside the containers.

#### Check

- [ ] A `.docx` opens in the browser and two people can edit it at once.

## Limits and gotchas

- CODE (the free edition) shows a notice above a number of concurrent documents; fine for a household.
- It needs 2+ GB RAM under use.

## Related

- [Nextcloud](kb:apps/nextcloud)
- [Caddy reverse proxy](kb:apps/caddy)
