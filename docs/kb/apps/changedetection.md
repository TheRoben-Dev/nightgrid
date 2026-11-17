# changedetection.io page watcher

Watch web pages for changes: a price drop, a driver or firmware release, a stock status, a government notice. It checks on a schedule and notifies you with a diff.

![Watch, filter, diff, notify.](fig:app-changedetection)

## What you end up with

- A list of watched pages with change history and diffs
- Filters so only the part you care about counts (a CSS selector or text)
- Notifications through ntfy, email, Discord and many more

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `changedetection`:

```yaml
services:
  changedetection:
    image: ghcr.io/dgtlmoon/changedetection.io:latest
    restart: unless-stopped
    environment:
      BASE_URL: http://changes.lan:5000
    volumes:
      - ./datastore:/datastore
    ports:
      - "5000:5000"
```

- [ ] Settings → set a password (it starts without one).

## Part 2 · Watch something

- [ ] Paste a URL, add a tag, **Watch**. Edit → **Filters & Triggers**: a CSS selector such as `.price` or `#downloads table` ignores the rest of the page.
- [ ] **Trigger/wait for text** only notifies when given text appears (for example "In stock").
- [ ] Notifications: an Apprise URL like `ntfys://ntfy.example.com/changes` per watch or as the default.

## Part 3 · Pages that need JavaScript

- [ ] Add a headless browser service and point changedetection at it:

```yaml
  browser:
    image: dgtlmoon/sockpuppetbrowser:latest
    restart: unless-stopped
    cap_add: [SYS_ADMIN]
```

- [ ] And on the main service: `PLAYWRIGHT_DRIVER_URL: ws://browser:3000`. Then pick "Playwright Chromium" as the fetch method on watches that need it.

#### Check

- [ ] **Recheck** a watch; the history shows a snapshot, and a second change shows a diff.

## In Nightgrid

- Watching vendor pages for firmware and BIOS updates pairs well with [Fleet](view:fleet) → **Updates**, which only knows about OS packages and container images.
- Add it to [Services](view:services) so a dead watcher doesn't go unnoticed.

## Limits and gotchas

- Respect sites: check every few hours, not every minute. Some block frequent fetches.
- The headless browser uses a lot of RAM; only use it for pages that really need it.

## Related

- [ntfy push notifications](kb:apps/ntfy)
- [Diun image update notifications](kb:apps/diun)
- [Patch every host from one place](kb:howto/fleet-updates)
