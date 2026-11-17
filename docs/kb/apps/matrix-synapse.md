# Matrix chat with Synapse

Your own end-to-end encrypted chat server for the family or a group of friends. Matrix is federated: your server talks to every other Matrix server, so you can join public rooms and chat with people elsewhere, while your messages and accounts live at home.

## What you end up with

- Synapse with PostgreSQL behind your reverse proxy
- Accounts on `@you:example.com`, created by you, no open sign-up
- Federation working, and Element (web, desktop, phone) as the app

## Before you start

- A domain you control and HTTPS on a subdomain such as `matrix.example.com`.
- Decide the **server name** now: it's the part after the colon in user IDs and can never change. Use the bare domain (`example.com`) and delegate to the subdomain with `.well-known` files (Part 3).

## Part 1 · Generate the config

- [ ] In a terminal on the host, in the stack folder (`/opt/stacks/matrix`):

```bash
docker run --rm -v ./data:/data \
  -e SYNAPSE_SERVER_NAME=example.com \
  -e SYNAPSE_REPORT_STATS=no \
  matrixdotorg/synapse:latest generate
```

- [ ] This writes `data/homeserver.yaml` and the server's signing key. Back up the key: losing it breaks federation for your server name.

## Part 2 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `matrix` in the same folder, with `DB_PASSWORD` in `.env`:

```yaml
services:
  synapse:
    image: matrixdotorg/synapse:latest
    restart: unless-stopped
    depends_on: [db]
    volumes:
      - ./data:/data
    ports:
      - "127.0.0.1:8008:8008"

  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: synapse
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: synapse
      POSTGRES_INITDB_ARGS: "--encoding=UTF8 --lc-collate=C --lc-ctype=C"
    volumes:
      - ./db:/var/lib/postgresql/data
```

- [ ] In `data/homeserver.yaml`, replace the `database:` section (Synapse refuses a database without C collation, hence the init args):

```yaml
database:
  name: psycopg2
  args:
    user: synapse
    password: "the DB_PASSWORD value"
    dbname: synapse
    host: db
    cp_min: 5
    cp_max: 10
```

- [ ] Point the reverse proxy for `matrix.example.com` at `127.0.0.1:8008`.

## Part 3 · Delegation and users

- [ ] Serve two files from `https://example.com/.well-known/matrix/`:

```text
server  ->  {"m.server": "matrix.example.com:443"}
client  ->  {"m.homeserver": {"base_url": "https://matrix.example.com"}}
```

- [ ] Create users from the container (answer the prompts, **yes** for admin on your own account):

```bash
docker compose exec synapse register_new_matrix_user -c /data/homeserver.yaml http://localhost:8008
```

- [ ] Sign in with Element on any device using `@you:example.com`. Set up **Secure Backup** in Element so encrypted history survives a lost phone.

#### Check

- [ ] The public Matrix federation tester reports success for `example.com`, and you can join `#matrix:matrix.org` from your account.

## In Nightgrid

- Add `https://matrix.example.com/_matrix/client/versions` to [Services](view:services) as the health check.
- Back up `data` (signing key, media) and a PostgreSQL dump together; see [PostgreSQL and MariaDB in Docker](kb:apps/databases).

## Limits and gotchas

- Joining big public rooms makes Synapse download their history and state: RAM and database size jump. Stick to small rooms on a small server.
- Media from other servers is cached forever by default; set `media_retention` in `homeserver.yaml`.
- Voice and video calls need a TURN server (coturn) to work across NATs.

## Related

- [Caddy reverse proxy](kb:apps/caddy)
- [Mattermost team chat](kb:apps/mattermost)
- [Mumble voice chat](kb:apps/mumble)
