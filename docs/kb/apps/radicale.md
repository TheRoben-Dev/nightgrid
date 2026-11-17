# Radicale calendars and contacts

A tiny CalDAV and CardDAV server for syncing calendars, tasks and contacts between phones and computers without Google or iCloud. No database, almost no RAM: everything is plain files on disk, easy to back up.

## What you end up with

- Radicale with password logins behind your reverse proxy
- Phones and desktop apps syncing calendars and address books
- A shared family calendar alongside personal ones

## Part 1 · Config and users

- [ ] In [Files](view:files) create `~/stacks/radicale/config/config`:

```ini
[server]
hosts = 0.0.0.0:5232

[auth]
type = htpasswd
htpasswd_filename = /config/users
htpasswd_encryption = bcrypt

[storage]
filesystem_folder = /data/collections
```

- [ ] Create a bcrypt user file in a [Terminal](view:terminal) tab: `htpasswd -B -c ~/stacks/radicale/config/users alice` (from `apache2-utils` or `httpd-tools`), and without `-c` for each further user.

## Part 2 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `radicale`:

```yaml
services:
  radicale:
    image: tomsquest/docker-radicale:latest
    restart: unless-stopped
    volumes:
      - ./config:/config:ro
      - ./data:/data
    ports:
      - "127.0.0.1:5232:5232"
```

- [ ] Point the reverse proxy for `dav.example.com` at `127.0.0.1:5232`, then open it and log in; the web page creates calendars and address books and shows each one's URL.

## Part 3 · Clients

- [ ] **iPhone/iPad and macOS**: add a CalDAV account and a CardDAV account with server `dav.example.com`, your user and password.
- [ ] **Android**: DAVx5, **Login with URL and user name**, `https://dav.example.com/`; it finds all collections.
- [ ] **Thunderbird**: new calendar on the network, paste a calendar URL from the web page; contacts via the built-in CardDAV address book.
- [ ] Shared calendar: create a user like `family` and add that account on each phone too. Radicale has no per-calendar sharing between users, so a shared login is the simple way.

#### Check

- [ ] An event added on a phone shows up on another device after a sync, and `data/collections` contains a matching `.ics` file.

## In Nightgrid

- Add the site to [Services](view:services).
- Back up the `data` folder with your normal file backup; the files are plain iCalendar and vCard, readable even without Radicale.

## Limits and gotchas

- Without `[auth]` set, Radicale accepts anyone. Check that a wrong password is refused before exposing it.
- The web page is only for creating and deleting collections; editing events happens in clients.
- If clients can't discover calendars on iOS, try `https://dav.example.com/<user>/` as the server address.

## Related

- [Nextcloud](kb:apps/nextcloud)
- [Vikunja tasks and projects](kb:apps/vikunja)
- [Caddy reverse proxy](kb:apps/caddy)
