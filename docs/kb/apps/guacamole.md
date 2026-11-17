# Apache Guacamole remote desktop gateway

Reach RDP, VNC and SSH machines from any browser through one login, with no client software. Useful for a locked-down work laptop or a shared family PC.

## What you end up with

- Guacamole at `remote.example.com` with your connections organised in groups
- TOTP two-factor on the login
- Nothing but HTTPS exposed

## Part 1 · The stack

- [ ] Guacamole needs `guacd`, the web app and a database. Initialise the PostgreSQL schema first:

```sh
mkdir -p /opt/stacks/guacamole/init
docker run --rm guacamole/guacamole /opt/guacamole/bin/initdb.sh --postgresql > /opt/stacks/guacamole/init/initdb.sql
```

- [ ] [Docker](view:docker) → **+ New stack** named `guacamole`:

```yaml
services:
  guacd:
    image: guacamole/guacd
    restart: unless-stopped
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_DB: guacamole
      POSTGRES_USER: guacamole
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./init:/docker-entrypoint-initdb.d:ro
      - ./db:/var/lib/postgresql/data
  guacamole:
    image: guacamole/guacamole
    restart: unless-stopped
    depends_on: [guacd, db]
    environment:
      GUACD_HOSTNAME: guacd
      POSTGRESQL_HOSTNAME: db
      POSTGRESQL_DATABASE: guacamole
      POSTGRESQL_USER: guacamole
      POSTGRESQL_PASSWORD: ${DB_PASSWORD}
      TOTP_ENABLED: "true"
    networks: [default, proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `remote.example.com` to `guacamole:8080` with the path `/guacamole/`. Log in with `guacadmin` / `guacadmin`, create your own admin, then delete `guacadmin`.

## Part 2 · Connections

- [ ] Settings → Connections → New: RDP to a Windows machine (enable NLA settings to match), VNC to a Linux desktop, SSH to a server.

#### Check

- [ ] A connection opens in a browser tab on a machine with nothing installed.

## Limits and gotchas

- A Guacamole login is a key to every connection in it. Keep TOTP on, and put it behind SSO or a tailnet if you can.
- For your own servers, Nightgrid's terminal and [Desktops](view:desktops) need no gateway.

## Related

- [RustDesk relay for remote support](kb:apps/rustdesk)
- [Open VM screens and RDP](kb:howto/vm-desktops)
