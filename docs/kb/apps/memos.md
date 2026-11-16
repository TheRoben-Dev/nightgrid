# Memos quick notes

A lightweight, timeline-style notes app for quick thoughts, snippets and links, with Markdown and tags. Fast to run, fast to use.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `memos`:

```yaml
services:
  memos:
    image: neosmemo/memos:stable
    restart: unless-stopped
    volumes:
      - ./data:/var/opt/memos
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `memos.example.com` to `memos:5230`. The first account is the admin; turn off sign-ups in settings.

#### Check

- [ ] A memo with a `#tag` appears under that tag.

## Limits and gotchas

- Back up `./data` (SQLite and attachments).
- For runbooks you want next to your hosts, Nightgrid's own [Knowledge](view:notes) notes can send commands to a terminal.

## Related

- [Write runbooks for alerts](kb:howto/runbooks)
- [BookStack wiki](kb:apps/bookstack)
