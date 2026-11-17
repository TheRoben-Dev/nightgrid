# Dockge alongside Nightgrid

Dockge is a small web UI for compose stacks: it edits `compose.yaml` files in a folder, runs up/down/pull, and shows the output live. Useful as a browser-based fallback on a host when you're away from the computer running Nightgrid. Both tools work on the same plain compose folders, so they don't get in each other's way.

![Dockge and Nightgrid sharing compose folders.](fig:app-dockge)

## What you end up with

- Dockge managing a stacks folder on one host
- The same stacks visible and manageable from Nightgrid's Docker view
- Dockge reachable only over the LAN or tailnet

## Part 1 · The stack

- [ ] Pick one stacks folder for both tools, such as `/opt/stacks`. Dockge expects one subfolder per stack with a `compose.yaml` inside.
- [ ] [Docker](view:docker) → **+ New stack** named `dockge`:

```yaml
services:
  dockge:
    image: louislam/dockge:1
    restart: unless-stopped
    environment:
      DOCKGE_STACKS_DIR: /opt/stacks
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./data:/app/data
      - /opt/stacks:/opt/stacks
    ports:
      - "127.0.0.1:5001:5001"
```

- [ ] The stacks path must be the same inside and outside the container (`/opt/stacks:/opt/stacks`), or relative paths in compose files break.
- [ ] Open it with **⧉** and create the admin account on the first screen.

## Part 2 · Share stacks with Nightgrid

- [ ] Nightgrid's [Docker](view:docker) → **STACKS** lists every running compose project, including ones Dockge started. A stack that has never been started isn't a compose project yet; add its folder there to see it before the first up.
- [ ] Edits made in either tool are just file changes; reload the other to see them.
- [ ] Keep secrets in `.env` files next to `compose.yaml`; Nightgrid's stack **Vault** can write them, and Dockge shows and edits the same file.

#### Check

- [ ] Create a stack in Dockge, then find it in Nightgrid's Docker view; stop it in Nightgrid, and Dockge shows it as exited.

## When Nightgrid is closed

- Dockge keeps working on its own: open it over the tailnet from a phone to restart a stack or read its logs.

## Limits and gotchas

- Mounting the Docker socket gives Dockge root-equivalent access to the host. Keep it bound to `127.0.0.1` or the tailnet, never exposed publicly, and use a strong password.
- Nightgrid's snapshots and rollbacks happen only for actions taken in Nightgrid; a pull and up from Dockge doesn't take a snapshot first.
- Dockge manages stacks only in its stacks folder; stacks elsewhere show in its list as unmanaged.

## Related

- [Portainer alongside Nightgrid](kb:apps/portainer)
- [Deploy a new compose stack](kb:howto/deploy-stack)
- [Docker and compose basics](kb:basics/docker-compose)
