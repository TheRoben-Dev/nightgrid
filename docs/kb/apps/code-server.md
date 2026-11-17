# code-server (VS Code in the browser)

Run VS Code on a server and use it from any browser: edit configs and code where they live, with the server's CPU and tools.

## What you end up with

- VS Code at `code.example.com` with a password
- A workspace folder on the server and extensions from Open VSX

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `code-server`:

```yaml
services:
  code-server:
    image: lscr.io/linuxserver/code-server:latest
    restart: unless-stopped
    environment:
      PUID: 1000
      PGID: 1000
      TZ: Etc/UTC
      HASHED_PASSWORD: ${HASHED_PASSWORD}
      DEFAULT_WORKSPACE: /config/workspace
    volumes:
      - ./config:/config
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Make an argon2 hash of your password (for example with `npx argon2-cli -e` on any machine with Node) and put it in the **Vault** (double each `$` in `.env`).
- [ ] Route `code.example.com` to `code-server:8443`.

#### Check

- [ ] The editor loads, and the integrated terminal runs inside the container.

## Limits and gotchas

- Anyone who logs in gets a shell in the container. Keep it behind a tailnet or SSO.
- For editing files on hosts, Nightgrid's own editor (Files view or `nano` with **Edit server files in Nightgrid**) needs nothing installed.

## Related

- [Work faster in the terminal](kb:howto/terminal)
- [Forgejo git server](kb:apps/forgejo)
