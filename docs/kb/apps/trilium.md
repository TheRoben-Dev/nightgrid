# Trilium notes

A personal knowledge base for large, deeply nested note trees: rich text and Markdown, code notes, diagrams, mind maps, note cloning (one note in several places), scripting, encryption of selected notes and fast full-text search. Desktop apps sync with your server, and the server works in any browser.

## What you end up with

- A TriliumNext server (the maintained continuation of Trilium) in Docker
- Desktop clients syncing to it, and the web UI from anywhere
- Protected (encrypted) notes for sensitive information

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `trilium`:

```yaml
services:
  trilium:
    image: triliumnext/trilium:latest
    restart: unless-stopped
    environment:
      TRILIUM_DATA_DIR: /home/node/trilium-data
    volumes:
      - ./data:/home/node/trilium-data
    ports:
      - "8080:8080"
```

- [ ] Older guides use the image name `triliumnext/notes` or the original `zadam/trilium`; the data folder is compatible going forward, but back it up before switching images.
- [ ] Open port 8080 and choose **I'm a new user**, then set the password. Put it behind your reverse proxy with HTTPS before using it away from home.

## Part 2 · Desktop sync

- [ ] Install the TriliumNext desktop app, choose **I have a server instance**, enter the server URL and password. It syncs in both directions and works offline.
- [ ] Desktop and server versions must match closely; update both together.

## Part 3 · Use it well

- [ ] **Protected notes**: right-click → **Protect subtree**. They're encrypted with your password and stay locked until you enter it in the session.
- [ ] **Clone** a note (Ctrl+C, then **Paste as clone**) to show it in several branches without copies.
- [ ] Types beyond text: **Code**, **Canvas** (Excalidraw), **Mermaid**, **Mind map**, **Relation map**.
- [ ] **Day notes** (journal) and **Templates** keep recurring notes consistent.

#### Check

- [ ] A note written in the desktop app offline appears in the web UI after the laptop reconnects.

## In Nightgrid

- Add the site to [Services](view:services). Trilium makes its own daily, weekly and monthly backups inside `data/backup`; copy the whole `data` folder to your backup target too.
- Runbooks you want DRIFT to read and attach to alerts belong in Nightgrid's [Knowledge](view:notes) notes. Trilium is for the big personal knowledge base around them.

## Limits and gotchas

- One user per server; it's a personal tool, not a team wiki.
- Very large attachments make sync slow; keep big files elsewhere and link to them.
- Forgetting the password locks protected notes for good.

## Related

- [Outline team wiki](kb:apps/outline)
- [Memos quick notes](kb:apps/memos)
- [BookStack wiki](kb:apps/bookstack)
