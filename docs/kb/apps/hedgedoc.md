# HedgeDoc collaborative Markdown

Real-time collaborative Markdown in the browser: share a link, type together, see the rendered page beside the source. Great for meeting notes, quick docs, checklists and slides written in Markdown.

## What you end up with

- HedgeDoc 1.x with PostgreSQL
- Local accounts with registration closed after you sign up
- Notes you can share read-only, as a slide deck or as a published page

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `hedgedoc`, with `DB_PASSWORD` and `SESSION_SECRET` (`openssl rand -hex 32`) in `.env`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: hedgedoc
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: hedgedoc
    volumes:
      - ./db:/var/lib/postgresql/data

  app:
    image: quay.io/hedgedoc/hedgedoc:1.10.0
    restart: unless-stopped
    depends_on: [db]
    environment:
      CMD_DB_URL: postgres://hedgedoc:${DB_PASSWORD}@db:5432/hedgedoc
      CMD_DOMAIN: notes.example.com
      CMD_PROTOCOL_USESSL: "true"
      CMD_URL_ADDPORT: "false"
      CMD_SESSION_SECRET: ${SESSION_SECRET}
      CMD_ALLOW_ANONYMOUS: "false"
      CMD_ALLOW_EMAIL_REGISTER: "true"
    volumes:
      - ./uploads:/hedgedoc/public/uploads
    ports:
      - "127.0.0.1:3000:3000"
```

- [ ] Point the reverse proxy for `notes.example.com` at `127.0.0.1:3000`. On a LAN without a proxy, set `CMD_DOMAIN` to the host's address, `CMD_PROTOCOL_USESSL` to `false`, `CMD_URL_ADDPORT` to `true`, and publish `3000:3000`.
- [ ] The uploads folder must be writable by the container: `sudo chown 10000:65534 uploads` if image uploads fail.

## Part 2 · Lock it down

- [ ] Register your account, then set `CMD_ALLOW_EMAIL_REGISTER` to `false` and redeploy. New accounts can still be added from the container: `docker compose exec app bin/manage_users --add you@example.com`.
- [ ] OIDC login is possible too, with `CMD_OAUTH2_*` variables pointing at Authentik or Keycloak.

## Part 3 · Using it

- [ ] Each note has a permission: **Freely**, **Editable**, **Limited**, **Locked**, **Protected** or **Private**. **Locked** is a good default for published docs.
- [ ] `---` between sections and the **Slide mode** view turn a note into a presentation.
- [ ] `- [ ]` task lists, code blocks, Mermaid diagrams and math all render in the preview.

#### Check

- [ ] Two browsers on the same note see each other's edits live, and anonymous visitors can't create notes.

## In Nightgrid

- Add it to [Services](view:services). Nightgrid's own [Knowledge](view:notes) notes are private to your Nightgrid and searchable by DRIFT; HedgeDoc is for notes you share with other people.
- Back up the database with a dump plus the `uploads` folder.

## Limits and gotchas

- HedgeDoc 2 is a rewrite with a different setup; this article is for the 1.x series. Check the release notes before switching images.
- With `CMD_ALLOW_ANONYMOUS` on, anyone who can reach the site can create notes. Keep it off on anything reachable from the internet.

## Related

- [Outline team wiki](kb:apps/outline)
- [Memos quick notes](kb:apps/memos)
- [Excalidraw whiteboard](kb:apps/excalidraw)
