# NocoDB spreadsheet database

An Airtable-style app on your own server: tables that look like spreadsheets but have real field types, links between tables, forms, kanban and gallery views, and a REST API. Good for inventories, collections, small CRMs and anything that outgrew a spreadsheet.

## What you end up with

- NocoDB with PostgreSQL for its data
- A base with linked tables, views and a public form
- An API token for scripts

## Part 1 · The stack

- [ ] Create a secret with `openssl rand -hex 32`. [Docker](view:docker) → **+ New stack** named `nocodb`, with it and `DB_PASSWORD` in `.env`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: nocodb
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: nocodb
    volumes:
      - ./db:/var/lib/postgresql/data

  nocodb:
    image: nocodb/nocodb:latest
    restart: unless-stopped
    depends_on: [db]
    environment:
      NC_DB: pg://db:5432?u=nocodb&p=${DB_PASSWORD}&d=nocodb
      NC_AUTH_JWT_SECRET: ${NC_SECRET}
      NC_PUBLIC_URL: https://db.example.com
    volumes:
      - ./data:/usr/app/data
    ports:
      - "8080:8080"
```

- [ ] Open port 8080 and sign up: the first account is the super admin. Turn off open sign-ups in **Account settings**.

## Part 2 · Build a base

- [ ] **New base** → tables such as `Items` and `Locations`. Field types: text, number, currency, date, attachment, checkbox, single/multi select, rating, barcode and more.
- [ ] A **Links** field connects tables (each item has a location; each location lists its items). **Lookup** and **Rollup** fields pull values across the link.
- [ ] Add views: **Grid**, **Kanban** by a select field, **Gallery** for items with photos, **Calendar** by a date, and a **Form** you can share as a link.

## Part 3 · API

- [ ] **Account → Tokens** creates an API token. Each table shows its REST endpoints under **Details → APIs**, with copyable `curl` examples.

#### Check

- [ ] Submitting the shared form adds a row, and the API returns it with `curl`.

## In Nightgrid

- **⧉** in [Docker](view:docker) opens it in a window; add it to [Services](view:services).
- Back up the database dump and the `data` folder (attachments live there).

## Limits and gotchas

- NocoDB can also connect to an existing database and show its tables; edits then change that database directly. Be careful pointing it at an app's own database.
- Very large tables (hundreds of thousands of rows) get slow in grid views; use filters.
- Public forms and shared views are open to anyone with the link; keep them off sensitive tables.

## Related

- [Homebox home inventory](kb:apps/homebox)
- [Snipe-IT asset inventory](kb:apps/snipe-it)
- [Database web UIs: Adminer and pgAdmin](kb:apps/db-admin-uis)
