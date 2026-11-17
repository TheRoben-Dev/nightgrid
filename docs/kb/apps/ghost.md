# Ghost blog and newsletter

A clean publishing platform for a blog with an optional email newsletter and members: write in a good editor, publish to the web, send posts by email, and keep everything on your server. Simpler and faster than WordPress when you mostly write.

## What you end up with

- Ghost 5 with MySQL 8 behind your reverse proxy
- Mail set up so member sign-in links and newsletters arrive
- A theme, navigation and the first post

## Before you start

- A domain and HTTPS. Ghost needs to know its public URL from the start.
- For newsletters: a Mailgun account (Ghost's bulk email only supports Mailgun). Transactional mail (sign-in links) works with any SMTP relay.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `ghost`, with `DB_PASSWORD`, `DB_ROOT_PASSWORD` and `SMTP_PASSWORD` in `.env`:

```yaml
services:
  ghost:
    image: ghost:5-alpine
    restart: unless-stopped
    depends_on: [db]
    environment:
      url: https://blog.example.com
      database__client: mysql
      database__connection__host: db
      database__connection__user: ghost
      database__connection__password: ${DB_PASSWORD}
      database__connection__database: ghost
      mail__transport: SMTP
      mail__from: "Blog <noreply@example.com>"
      mail__options__host: smtp.example.com
      mail__options__port: "587"
      mail__options__auth__user: noreply@example.com
      mail__options__auth__pass: ${SMTP_PASSWORD}
    volumes:
      - ./content:/var/lib/ghost/content
    ports:
      - "127.0.0.1:2368:2368"

  db:
    image: mysql:8.0
    restart: unless-stopped
    environment:
      MYSQL_ROOT_PASSWORD: ${DB_ROOT_PASSWORD}
      MYSQL_DATABASE: ghost
      MYSQL_USER: ghost
      MYSQL_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/mysql
```

- [ ] Ghost supports MySQL 8 in production, not MariaDB. Point the reverse proxy at `127.0.0.1:2368`.
- [ ] Open `https://blog.example.com/ghost` and create the owner account right away; the first visitor to that page becomes the owner.

## Part 2 · Set it up

- [ ] **Settings → Design**: pick a theme (Casper is the default), logo and colours. **Navigation** for the menu.
- [ ] **Settings → Membership**: turn members off for a plain blog, or on for free sign-ups and newsletters.
- [ ] **Settings → Newsletters → Mailgun**: domain and API key, if you want to email posts.

#### Check

- [ ] A published post appears on the home page, and signing up as a member sends a working sign-in email.

## In Nightgrid

- Add the blog to [Services](view:services) with **Expiry watch → Add…** for its certificate and domain.
- Back up `content` (images, themes) and a MySQL dump together; see [PostgreSQL and MariaDB in Docker](kb:apps/databases) for the dump pattern.

## Limits and gotchas

- The `url` must match exactly, including `https://`, or links and the admin redirect loop.
- Major Ghost upgrades run database migrations; take a dump before changing the image tag.
- Comments, members and paid subscriptions (Stripe) are built in but optional; a public site with members needs working mail first.

## Related

- [WordPress site in Docker](kb:apps/wordpress)
- [Send mail from your servers](kb:apps/mail-relay)
- [Umami web analytics](kb:apps/umami)
