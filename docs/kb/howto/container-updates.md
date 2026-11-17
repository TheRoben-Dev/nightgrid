# Update containers without fear

Find out which images have new versions, read what changed, update with a snapshot taken first, and roll back if the new version misbehaves.

## What you end up with

- A list of stacks with newer images, checked only when you ask
- A snapshot of exact image ids, compose files and `.env` before every update
- A rollback that names every file and service it changes

## Before you start

- Stacks running in [Docker](view:docker). Pin tags where you can (`:2`, `:1.18`); a floating `:latest` makes "what changed" hard to answer.

## Part 1 · See what's new

- [ ] [Docker](view:docker) → **Updates** asks each image's registry whether its tag moved. It shows the version you run and the new one.
- [ ] **What changed** opens the project's release notes or compare page in your browser. Read them for breaking changes and migration steps.
- [ ] Across all hosts: [Fleet](view:fleet) → **Check images**, then **Open stack**.

## Part 2 · Update

- [ ] Update one stack at a time. Nightgrid takes a snapshot first (the newest 10 are kept).
- [ ] Watch the logs as it starts. **✦ Explain** sends errors to DRIFT.
- [ ] Database-backed apps: take a dump first (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)). Image rollbacks don't undo a schema migration.

#### Check

- [ ] The service check in [Services](view:services) stays green, and the app works when you log in.

## Part 3 · Roll back

- [ ] **Snapshots** → **Roll back…** lists every file and service it changes and saves the current state first.
- [ ] After a rollback, pin the tag to the version that worked, so the next check doesn't offer the broken one silently.

## Part 4 · Automating it (or not)

- [ ] Auto-updaters (Watchtower and similar) update without snapshots or release notes. For anything with data you care about, update by hand here.
- [ ] A weekly reminder works well: a command widget or a note pinned to the Overview.

## Limits and gotchas

- Registries rate-limit anonymous checks (Docker Hub especially). Checking dozens of images many times a day can hit it.
- Images built locally (`build:` in compose) have no registry to ask.
- A tag that is re-pushed with the same name (`:latest`, `:stable`) shows as changed even when only the base image moved.

## Related

- [Deploy a new compose stack](kb:howto/deploy-stack)
- [Deploy stacks from git](kb:howto/stacks-from-git)
- [Patch every host from one place](kb:howto/fleet-updates)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
