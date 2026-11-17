# Forgejo git server

A light, self-hosted git forge (a community fork of Gitea) with issues, pull requests, packages and Actions-compatible CI runners.

![Forgejo with CI and a registry.](fig:app-ci-registry)

## What you end up with

- Forgejo at `git.example.com` with SSH clone on port 2222
- Your repositories, including the one Nightgrid deploys stacks from
- Optional CI with a Forgejo runner

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `forgejo`:

```yaml
services:
  forgejo:
    image: codeberg.org/forgejo/forgejo:11
    restart: unless-stopped
    environment:
      USER_UID: 1000
      USER_GID: 1000
      FORGEJO__server__DOMAIN: git.example.com
      FORGEJO__server__ROOT_URL: https://git.example.com/
      FORGEJO__server__SSH_PORT: 2222
      FORGEJO__service__DISABLE_REGISTRATION: "true"
    volumes:
      - ./data:/data
      - /etc/timezone:/etc/timezone:ro
      - /etc/localtime:/etc/localtime:ro
    ports:
      - "2222:22"
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Use the current major version tag from Forgejo's release notes in place of `11`.
- [ ] Route `git.example.com` to `forgejo:3000` and finish the installer (SQLite is fine for personal use). The first account becomes admin; registration is disabled after that by the variable above.

## Part 2 · Use it with Nightgrid

- [ ] Add your SSH key in Forgejo (Settings → Connections → SSH keys). **Copy public key** in Nightgrid's **Settings → Connections → SSH keys** gives you the line.
- [ ] Each Docker host that deploys stacks from git: add its public key as a read-only **deploy key** on the stacks repository.
- [ ] Then [Deploy stacks from git](kb:howto/stacks-from-git).

## Part 3 · CI runner (optional)

- [ ] Site administration → Actions → Runners → create a registration token.
- [ ] Run `code.forgejo.org/forgejo/runner` on a separate host or VM, register it with the token and your instance URL, and give it a Docker socket of its own. Workflows go in `.forgejo/workflows/` and use GitHub Actions syntax.

## Part 4 · Backups

- [ ] `./data` holds repositories, database and config. Stop-free dump:

```sh
docker exec -u 1000 forgejo-forgejo-1 forgejo dump -c /data/gitea/conf/app.ini -f /data/forgejo-dump.zip
```

#### Check

- [ ] `git clone ssh://git@git.example.com:2222/you/repo.git` works.

## Limits and gotchas

- The runner can execute anything in your workflows; never give it the host's main Docker socket on an important server.
- Upgrade one major version at a time and read the release notes.

## Related

- [Deploy stacks from git](kb:howto/stacks-from-git)
- [Dev and git deploy lab](kb:guides/dev-git-deploy-lab)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
