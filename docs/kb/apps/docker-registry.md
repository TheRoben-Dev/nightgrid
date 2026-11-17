# Private container registry

Host your own images with the Distribution registry: push from CI or your laptop, pull on your servers, nothing public.

## Part 1 · The stack

- [ ] Make a login file:

```sh
mkdir -p /opt/stacks/registry/auth
docker run --rm --entrypoint htpasswd httpd:2 -Bbn ci '<password>' > /opt/stacks/registry/auth/htpasswd
```

- [ ] [Docker](view:docker) → **+ New stack** named `registry`:

```yaml
services:
  registry:
    image: registry:3
    restart: unless-stopped
    environment:
      REGISTRY_AUTH: htpasswd
      REGISTRY_AUTH_HTPASSWD_REALM: Registry
      REGISTRY_AUTH_HTPASSWD_PATH: /auth/htpasswd
      REGISTRY_STORAGE_DELETE_ENABLED: "true"
    volumes:
      - ./data:/var/lib/registry
      - ./auth:/auth
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `registry.example.com` to `registry:5000` with HTTPS (Docker refuses plain HTTP registries unless configured as insecure).

## Part 2 · Use it

```sh
docker login registry.example.com
docker tag myapp:1.0 registry.example.com/myapp:1.0
docker push registry.example.com/myapp:1.0
```

- [ ] On servers, `docker login` once; compose files reference `registry.example.com/myapp:1.0`.
- [ ] Nightgrid's **Updates** check asks registries for new tags; private ones work when the host is logged in.

## Part 3 · Clean up

- [ ] Deleting tags doesn't free space until garbage collection runs (stop pushes first):

```sh
docker exec registry-registry-1 registry garbage-collect /etc/distribution/config.yml --delete-untagged
```

#### Check

- [ ] A push from one machine pulls on another.

## Limits and gotchas

- Forgejo and Gitea have a built-in package registry that may be all you need ([Forgejo git server](kb:apps/forgejo)).
- Registry version 2 images use `/etc/docker/registry/config.yml` instead of the path above.

## Related

- [Forgejo git server](kb:apps/forgejo)
- [Woodpecker CI](kb:apps/woodpecker)
