# Vikunja tasks and projects

A to-do and project app with lists, Kanban boards, Gantt and shared projects, synced to your phone via CalDAV.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `vikunja`:

```yaml
services:
  vikunja:
    image: vikunja/vikunja:latest
    restart: unless-stopped
    environment:
      VIKUNJA_SERVICE_PUBLICURL: https://tasks.example.com
      VIKUNJA_SERVICE_JWTSECRET: ${JWT_SECRET}
      VIKUNJA_DATABASE_TYPE: sqlite
      VIKUNJA_DATABASE_PATH: /db/vikunja.db
      VIKUNJA_SERVICE_ENABLEREGISTRATION: "false"
    volumes:
      - ./files:/app/vikunja/files
      - ./db:/db
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] `./files` and `./db` must be writable by uid 1000: `sudo chown -R 1000 files db`.
- [ ] Route `tasks.example.com` to `vikunja:3456`. With registration off, create users from the CLI: `docker exec vikunja-vikunja-1 /app/vikunja/vikunja user create -u alice -e alice@example.com -p <password>`.

## Part 2 · Phone sync

- [ ] CalDAV URL: `/dav/principals/<username>/` on your site, with an app token from Settings → CalDAV. Works with Tasks.org and Apple Reminders.

#### Check

- [ ] A task added on the phone shows in the web UI.

## Limits and gotchas

- Back up `./db` and `./files` together.

## Related

- [Nextcloud](kb:apps/nextcloud)
- [Mealie recipes](kb:apps/mealie)
