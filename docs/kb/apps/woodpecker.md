# Woodpecker CI

A light CI/CD server that runs pipelines in containers, connected to Forgejo or Gitea. Build images, run tests and deploy on push.

## Part 1 · Connect to Forgejo

- [ ] In Forgejo: Settings → Applications → create an OAuth2 app with redirect URL `https://ci.example.com/authorize`. Note the client id and secret.

## Part 2 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `woodpecker` on a host meant for builds:

```yaml
services:
  server:
    image: woodpeckerci/woodpecker-server:v3
    restart: unless-stopped
    environment:
      WOODPECKER_HOST: https://ci.example.com
      WOODPECKER_FORGEJO: "true"
      WOODPECKER_FORGEJO_URL: https://git.example.com
      WOODPECKER_FORGEJO_CLIENT: ${FORGEJO_CLIENT}
      WOODPECKER_FORGEJO_SECRET: ${FORGEJO_SECRET}
      WOODPECKER_AGENT_SECRET: ${AGENT_SECRET}
      WOODPECKER_ADMIN: you
    volumes:
      - ./server:/var/lib/woodpecker
    networks: [default, proxy]
  agent:
    image: woodpeckerci/woodpecker-agent:v3
    restart: unless-stopped
    depends_on: [server]
    environment:
      WOODPECKER_SERVER: server:9000
      WOODPECKER_AGENT_SECRET: ${AGENT_SECRET}
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
networks:
  proxy:
    external: true
```

- [ ] Secrets in the **Vault**; route `ci.example.com` to `server:8000`.

## Part 3 · A pipeline

- [ ] `.woodpecker.yaml` in a repository:

```yaml
steps:
  - name: test
    image: python:3.13
    commands:
      - pip install -r requirements.txt
      - pytest
```

#### Check

- [ ] A push shows a green pipeline in Woodpecker and a status on the commit in Forgejo.

## Limits and gotchas

- The agent controls the host's Docker; run it on a dedicated build host or VM, never on your main server.
- Forgejo's own Actions runner is the alternative if you prefer GitHub Actions syntax.

## Related

- [Forgejo git server](kb:apps/forgejo)
- [Dev and git deploy lab](kb:guides/dev-git-deploy-lab)
- [Private container registry](kb:apps/docker-registry)
