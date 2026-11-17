# Snipe-IT asset inventory

Track hardware with asset tags, serials, warranties and who has what: lab gear, shop stock, loaners or the family's laptops. Built for IT departments, works well for a homelab or a repair counter.

## What you end up with

- Every device with model, serial, purchase date, warranty end and location
- Check-out and check-in history per asset
- Printable asset tag labels with QR codes

## Before you start

- A host with Docker and about 1 GB of free RAM
- A name for it on your reverse proxy, or a LAN port

## Part 1 · The stack

- [ ] Generate an app key once: `docker run --rm snipe/snipe-it php artisan key:generate --show` (copy the `base64:...` value).
- [ ] [Docker](view:docker) → **+ New stack** named `snipeit`. Put the passwords and key in the `.env` (Nightgrid writes variables there with mode 600):

```yaml
services:
  snipeit:
    image: snipe/snipe-it:latest
    restart: unless-stopped
    depends_on: [db]
    ports:
      - "8085:80"
    environment:
      APP_URL: http://snipeit.lan:8085
      APP_KEY: ${APP_KEY}
      APP_TIMEZONE: Etc/UTC
      DB_CONNECTION: mysql
      DB_HOST: db
      DB_DATABASE: snipeit
      DB_USERNAME: snipeit
      DB_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./data:/var/lib/snipeit
  db:
    image: mariadb:11
    restart: unless-stopped
    environment:
      MARIADB_DATABASE: snipeit
      MARIADB_USER: snipeit
      MARIADB_PASSWORD: ${DB_PASSWORD}
      MARIADB_RANDOM_ROOT_PASSWORD: "1"
    volumes:
      - ./db:/var/lib/mysql
```

- [ ] `APP_URL` must match the address you open it at, or logins and links break.

## Part 2 · Set it up

- [ ] Open the page and run the pre-flight and setup wizard; create the admin user.
- [ ] Settings → Asset Tags: a prefix and auto-increment (for example `LAB-0001`).
- [ ] Add Manufacturers, Categories and Models first, then Assets: assets hang off a model, which saves typing the same specs over and over.
- [ ] Custom fields (Settings → Custom Fields) for MAC address, RAM or CPU, attached to a fieldset per category.

## Part 3 · Day to day

- [ ] **Checkout** an asset to a person, location or another asset (a drive into a server); check-in logs it back.
- [ ] Labels: select assets → Generate Labels, print on label sheets or a label printer.
- [ ] Warranty: set months on the model; the dashboard lists assets whose warranty ends soon.

#### Check

- [ ] Scan a printed QR label with a phone: it opens that asset's page.

## In Nightgrid

- Add the page to [Services](view:services) so you see it's up and when its certificate ends.
- Back up `./db` with a database dump, not a file copy of a running MariaDB: see [Back up databases the right way](kb:apps/databases).
- Put each server's asset tag in its [Knowledge](view:notes) note, so DRIFT can answer "when does the NAS warranty end?" from your notes.

## Limits and gotchas

- Mail settings (`MAIL_*` variables) are needed for password resets and checkout emails.
- Upgrades run database migrations on start; take a dump before pulling a new image.
- The API (Personal Access Tokens) can feed it from scripts, such as `dmidecode` output from each server.

## Related

- [Inventory a machine's hardware](kb:linux/hardware-inventory)
- [Homebox home inventory](kb:apps/homebox)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
