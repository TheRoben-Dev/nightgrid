# Mealie recipes

Collect recipes from any website into a clean, searchable cookbook with meal plans and shopping lists the whole household can use.

## What you end up with

- Recipes imported from a URL with ingredients and steps parsed
- Weekly meal plans and shared shopping lists

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `mealie`:

```yaml
services:
  mealie:
    image: ghcr.io/mealie-recipes/mealie:latest
    restart: unless-stopped
    environment:
      ALLOW_SIGNUP: "false"
      BASE_URL: https://recipes.example.com
      TZ: Etc/UTC
    volumes:
      - ./data:/app/data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `recipes.example.com` to `mealie:9000`.
- [ ] Log in with the default admin shown in Mealie's docs and change the password and email at once.

## Part 2 · Use it

- [ ] Create a household and invite family with invite links.
- [ ] Import recipes by URL; the browser bookmarklet makes it one click.

#### Check

- [ ] A recipe URL imports with ingredients split out.

## Limits and gotchas

- Pin a version tag; major versions have changed the data model before. Back up `./data` (it includes the SQLite database).

## Related

- [Deploy a new compose stack](kb:howto/deploy-stack)
- [Caddy reverse proxy](kb:apps/caddy)
