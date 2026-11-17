# Penpot design and prototyping

An open-source design tool in the browser for UI mockups, diagrams and click-through prototypes, built on SVG and open standards. Self-hosted, your files stay on your server and you can invite as many people as you like.

## What you end up with

- The official Penpot stack (frontend, backend, exporter, PostgreSQL, Valkey) running from its own compose file
- It behind your reverse proxy on HTTPS
- Sign-ups closed after your own account exists

## Before you start

- A Docker host with at least 4 GB RAM free; the backend is a JVM and the exporter runs a headless browser.
- HTTPS: browsers only allow some of Penpot's features (clipboard, fonts) on a secure origin.

## Part 1 · The stack

- [ ] In a [Terminal](view:terminal) tab on the host: `mkdir -p ~/stacks/penpot && cd ~/stacks/penpot`, then fetch the official compose file: `curl -o docker-compose.yaml https://raw.githubusercontent.com/penpot/penpot/main/docker/images/docker-compose.yaml`.
- [ ] Open it in [Files](view:files). Set `PENPOT_PUBLIC_URI` to `https://design.example.com`, and change the database password in both places it appears.
- [ ] The file publishes the frontend on port `9001`; change that line to `127.0.0.1:9001:8080` (keep the container port the file uses) so only the proxy reaches it.
- [ ] `docker compose up -d`. The stack shows up in [Docker](view:docker) like any other.
- [ ] Point the reverse proxy for `design.example.com` at `127.0.0.1:9001`, with WebSocket support on (Penpot uses it for live collaboration).

## Part 2 · Accounts and flags

- [ ] The flags live in one shared `PENPOT_FLAGS` line near the top of the file. The default includes `disable-email-verification`, so you can register right away without mail. Create your account.
- [ ] Then edit the flags: add `disable-registration` so strangers can't sign up, and remove `disable-secure-session-cookies` now that you're on HTTPS. Redeploy and invite others from a team's **Invitations**.
- [ ] The default file sends mail to a local test catcher. For real invitation emails, set the `PENPOT_SMTP_*` variables to your relay and remove the catcher service.

#### Check

- [ ] Two browsers editing the same file see each other's cursors and changes live; export a board as PNG to confirm the exporter works.

## In Nightgrid

- Add the site to [Services](view:services).
- Back up a PostgreSQL dump plus the `penpot_assets` volume (uploaded images and fonts); see [PostgreSQL and MariaDB in Docker](kb:apps/databases).
- [Docker](view:docker) → **Updates** shows new image versions; read Penpot's release notes before major upgrades, they sometimes change the compose file.

## Limits and gotchas

- The upstream compose file changes between releases. When upgrading, compare your copy with the new one rather than only bumping tags.
- Penpot is not Figma: Figma files don't import directly. Plugins and community converters exist, with mixed results.
- The exporter needs outbound access to the frontend's public URL; if exports hang, check that the server can resolve and reach `design.example.com` itself.

## Related

- [Excalidraw whiteboard](kb:apps/excalidraw)
- [Forgejo git server](kb:apps/forgejo)
- [Caddy reverse proxy](kb:apps/caddy)
