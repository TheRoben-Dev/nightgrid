# Verdaccio private npm registry

A private npm registry and caching proxy. Publish your own packages to it, and let it cache everything you install from npmjs.org, so CI and new machines install fast and keep working when the public registry has a bad day.

## What you end up with

- Verdaccio in Docker behind your reverse proxy
- npm, pnpm and yarn on your machines and CI using it as their registry
- Your own scoped packages published privately

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `verdaccio`:

```yaml
services:
  verdaccio:
    image: verdaccio/verdaccio:6
    restart: unless-stopped
    environment:
      VERDACCIO_PUBLIC_URL: https://npm.example.com
    volumes:
      - ./conf:/verdaccio/conf
      - ./storage:/verdaccio/storage
    ports:
      - "127.0.0.1:4873:4873"
```

- [ ] Start it once so it writes the default `conf/config.yaml`; if the folders aren't writable, `sudo chown -R 10001:65533 conf storage` (the image's user) and redeploy.
- [ ] Point the reverse proxy for `npm.example.com` at `127.0.0.1:4873`.

## Part 2 · Lock it down

- [ ] In `conf/config.yaml`, under `auth: htpasswd:`, set `max_users: -1` after you've created your own user (`npm adduser --registry https://npm.example.com`), so nobody else can register.
- [ ] Under `packages`, give your scope private rules, for example `'@home/*': { access: $authenticated, publish: $authenticated }`, and leave `'**'` proxying to `npmjs` for everything else.
- [ ] Redeploy after editing the config.

## Part 3 · Use it

- [ ] Per machine or project: `npm config set registry https://npm.example.com/` (or a `.npmrc` with `registry=`). pnpm and yarn read the same setting.
- [ ] Publish: `npm publish --registry https://npm.example.com` from a package named `@home/something`.
- [ ] CI: put the registry and an auth token in the CI secrets and write an `.npmrc` at the start of the job.

#### Check

- [ ] `npm view lodash version` succeeds through the registry and `storage` gains a `lodash` folder; a second install on another machine is served from the cache.

## In Nightgrid

- Add the site to [Services](view:services); watch `storage` in [Fleet](view:fleet) → **Disks**, it grows with every cached package version.
- Back up `storage` for your own published packages; cached public packages can always be fetched again.

## Limits and gotchas

- Cached packages aren't pruned automatically. Clear old versions occasionally, or accept that the folder grows.
- If Verdaccio is down, installs fail on machines that point at it. Keep a note on how to switch back (`npm config delete registry`).
- The same image can't proxy PyPI or other ecosystems; use a dedicated cache for those.

## Related

- [Private container registry](kb:apps/docker-registry)
- [Forgejo git server](kb:apps/forgejo)
- [Woodpecker CI](kb:apps/woodpecker)
