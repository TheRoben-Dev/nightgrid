# Wiki.js documentation wiki

A good-looking wiki for homelab documentation, family how-tos or a small team: Markdown editor, page history, search, and optional sync of all pages to a git repository.

## What you end up with

- A wiki with page tree, tags and full history
- Markdown pages you can keep in git as plain files
- Logins with local accounts or your SSO

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `wiki`, passwords in `.env`:

```yaml
services:
  wiki:
    image: ghcr.io/requarks/wiki:2
    restart: unless-stopped
    depends_on: [db]
    environment:
      DB_TYPE: postgres
      DB_HOST: db
      DB_PORT: "5432"
      DB_USER: wikijs
      DB_PASS: ${DB_PASSWORD}
      DB_NAME: wiki
    ports:
      - "3000:3000"
  db:
    image: postgres:17
    restart: unless-stopped
    environment:
      POSTGRES_USER: wikijs
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: wiki
    volumes:
      - ./db:/var/lib/postgresql/data
```

## Part 2 · First run

- [ ] Open port 3000, create the administrator, set the site URL.
- [ ] Administration → Storage → **Git**: point it at a private repository (for example on your Forgejo) with a deploy key. Pages are then also Markdown files in git.
- [ ] Administration → Authentication: add OIDC from Authentik or Authelia if you run one.

#### Check

- [ ] Create a page, edit it twice, and open History: both versions are there, and the git repo has the file.

## In Nightgrid

- For runbooks tied to alerts and commands you can send to a terminal, [Knowledge](view:notes) notes are closer at hand; keep the wiki for longer docs others read.
- Back up with `pg_dump` and watch the job in [Fleet](view:fleet) → **Backups**.

## Limits and gotchas

- Wiki.js 2 is in maintenance while version 3 is developed; it's stable, but expect few new features.
- Image uploads go into the database unless you set up a storage target.

## Related

- [BookStack wiki](kb:apps/bookstack)
- [Forgejo git server](kb:apps/forgejo)
- [Write runbooks for alerts](kb:howto/runbooks)
