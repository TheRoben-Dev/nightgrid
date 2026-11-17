# Grocy household stock

"ERP beyond your fridge": groceries and household supplies with best-before dates, a shopping list that fills itself, chores and battery changes. Scan barcodes with a phone to add and use stock.

## What you end up with

- Stock with due dates and minimum amounts
- A shopping list from what's running low
- Chores and recurring tasks (filters, batteries, smoke alarm tests)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `grocy`:

```yaml
services:
  grocy:
    image: lscr.io/linuxserver/grocy:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
    volumes:
      - ./config:/config
    ports:
      - "9283:80"
```

- [ ] Open port 9283 and log in with `admin` / `admin`. Change the password straight away (user menu → Change password).

## Part 2 · Set up

- [ ] Settings → Feature flags: turn off what you won't use (recipes, equipment, batteries) to keep menus short.
- [ ] Add **Locations** (Fridge, Pantry, Freezer) and **Quantity units** before products.
- [ ] Products: name, default location, minimum stock and default best-before days. Barcode fields let phone scanners find them.

## Part 3 · Use it

- [ ] **Purchase** to add stock, **Consume** to use it. The barcode scanner in the mobile web UI does both.
- [ ] Shopping list → **Add all products below min. stock amount**.
- [ ] Chores: a schedule (every N days, weekly) and who does it; they show up as due on the overview.

#### Check

- [ ] Consume the last item of a product with a minimum stock: it appears on the shopping list.

## In Nightgrid

- [Services](view:services) shows if it's up; `./config` holds the SQLite database, so back up the stack folder.
- Keep it on the LAN or tailnet; there's nothing here worth publishing.

## Limits and gotchas

- The default `admin` / `admin` login works until you change it. Do it before opening the port anywhere.
- Barcode lookups for new products use external plugins; without one you type product details once.

## Related

- [Homebox home inventory](kb:apps/homebox)
- [Mealie recipes](kb:apps/mealie)
- [Home Assistant in Docker](kb:apps/home-assistant)
