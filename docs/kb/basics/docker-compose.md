# Docker and compose basics

Docker runs each app in a **container**: its own filesystem and processes, built from an **image**, sharing the host's kernel. **Compose** describes a set of containers in one YAML file, so the whole app comes up with one command and the file is the documentation.

## Words you'll see

- **Image**: a read-only template (`jellyfin/jellyfin:10.9`). The part after `:` is the tag; `latest` is just a tag that moves.
- **Container**: a running (or stopped) instance of an image. Delete it and anything not in a volume is gone.
- **Volume / bind mount**: where data survives. A bind mount maps a host folder (`./config:/config`); a named volume is managed by Docker.
- **Port mapping**: `"8096:8096"` publishes container port 8096 on every host address. `"127.0.0.1:8096:8096"` publishes it to the host only.
- **Network**: containers in one compose project share a network and reach each other by service name (`http://db:5432`).

## A small compose file

```yaml
services:
  app:
    image: ghcr.io/example/app:1.4
    restart: unless-stopped
    ports:
      - "127.0.0.1:8080:8080"
    environment:
      DB_HOST: db
      DB_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./data:/data
    depends_on:
      - db
  db:
    image: postgres:16
    restart: unless-stopped
    environment:
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/postgresql/data
```

- `restart: unless-stopped` brings it back after a reboot.
- `${DB_PASSWORD}` comes from a `.env` file next to the compose file, so the secret isn't in the YAML. In Nightgrid, keep it in the stack's **Vault** and use **Deploy .env…**.

## Everyday commands

```sh
docker compose up -d        # create or update, in the background
docker compose ps           # what's running
docker compose logs -f app  # follow one service's logs
docker compose pull && docker compose up -d   # update images
docker compose down         # stop and remove containers (volumes stay)
```

[Docker](view:docker) in Nightgrid does all of these per stack, with a confirmation that names the stack for anything destructive, and snapshots the compose and `.env` files before an update so you can roll back.

## Habits that save you later

- Pin versions you care about (`postgres:16`, not `postgres:latest`): a major database upgrade should be a decision.
- One folder per stack, with its data folders beside the compose file. Backing up the folder backs up the app.
- Ask DRIFT to review a compose file: it flags missing restart policies, ports on 0.0.0.0, `:latest` and inline secrets.
- `docker system df` shows what images and build cache use; prune deliberately, not on a timer.

## Related

- [The docker group](kb:basics/docker-group): what running Docker without sudo really grants.
- [Reverse proxies](kb:basics/reverse-proxies): put the apps behind one HTTPS entry point.
- [Backups](kb:basics/backups): one folder per stack makes them simple.
