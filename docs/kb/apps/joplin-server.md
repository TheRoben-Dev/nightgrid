# Joplin Server for note sync

Sync Joplin notes between your phone, laptop and desktop through your own server instead of Dropbox or OneDrive. Joplin's apps support end-to-end encryption, so the server only stores encrypted blobs, and Joplin Server adds sharing and publishing notes as web pages.

## What you end up with

- Joplin Server with PostgreSQL behind your reverse proxy
- All Joplin apps syncing to it, with end-to-end encryption on
- Optional accounts for family members

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `joplin`, with `DB_PASSWORD` in `.env`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: joplin
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: joplin
    volumes:
      - ./db:/var/lib/postgresql/data

  app:
    image: joplin/server:latest
    restart: unless-stopped
    depends_on: [db]
    environment:
      APP_PORT: "22300"
      APP_BASE_URL: https://notes.example.com
      DB_CLIENT: pg
      POSTGRES_HOST: db
      POSTGRES_PORT: "5432"
      POSTGRES_DATABASE: joplin
      POSTGRES_USER: joplin
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    ports:
      - "127.0.0.1:22300:22300"
```

- [ ] Point the reverse proxy for `notes.example.com` at `127.0.0.1:22300`. `APP_BASE_URL` must be exactly the address clients use.
- [ ] Log in at `https://notes.example.com/login` with `admin@localhost` / `admin`, then change the email and password under **Your profile** straight away.

## Part 2 · Clients

- [ ] In each Joplin app: **Settings → Synchronisation → Joplin Server**, the URL, your email and password, **Check synchronisation configuration**.
- [ ] **Settings → Encryption → Enable encryption** on one device, set a master password, sync, then enter the same password on the other devices.
- [ ] Admins can add users under **Admin → Users**; each has a separate notebook space.

#### Check

- [ ] A note created on the phone appears on the desktop after a sync, and the server's database holds only encrypted items (the web UI shows them as encrypted).

## In Nightgrid

- Add the site to [Services](view:services).
- Back up a PostgreSQL dump (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)). Each client also keeps a full local copy, which is a second safety net.

## Limits and gotchas

- Lose the encryption master password and the notes can't be decrypted anywhere. Store it in your password manager.
- Sharing and publishing notes don't work for encrypted notebooks in the same way; check the Joplin docs before relying on them.
- Attachments count toward the server's storage; large PDFs and images add up.

## Related

- [Trilium notes](kb:apps/trilium)
- [Memos quick notes](kb:apps/memos)
- [Nextcloud](kb:apps/nextcloud)
