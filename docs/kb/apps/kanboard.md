# Kanboard project boards

A small, fast kanban board for tasks and projects: columns, swimlanes, due dates, subtasks and automatic actions. One container, SQLite by default, and almost no resources, which suits a household task board or a repair queue.

## What you end up with

- Kanboard behind your reverse proxy with its data on the host
- An admin account with a new password and your own users
- A board with columns that match how you work

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `kanboard`:

```yaml
services:
  kanboard:
    image: kanboard/kanboard:latest
    restart: unless-stopped
    volumes:
      - ./data:/var/www/app/data
      - ./plugins:/var/www/app/plugins
    ports:
      - "127.0.0.1:8086:80"
```

- [ ] Point the reverse proxy for `board.example.com` at `127.0.0.1:8086`.
- [ ] Log in with `admin` / `admin` and change the password at once under **My profile → Change password**.

## Part 2 · Set it up

- [ ] **Users management → New user** for each person; give them **Project member** or **Project manager** roles per project.
- [ ] **New project**, then **Configure this project → Columns** to rename Backlog / Ready / Work in progress / Done to your own steps (for a repair bench: Received, Diagnosing, Waiting for parts, Ready for pickup).
- [ ] **Automatic actions** move or close tasks on events, such as assigning a task to whoever moves it into "Work in progress".
- [ ] Set **Settings → Application settings → Application URL** to the public address so links in emails and notifications are right.

#### Check

- [ ] A second user can log in, sees only the projects they are a member of, and can drag a task between columns.

## In Nightgrid

- **⧉** in [Docker](view:docker) opens it in a window; add the site to [Services](view:services).
- Back up the `data` folder: it holds the SQLite database and attachments.

## Limits and gotchas

- Kanboard is mature and quiet; new features are rare. If you want a modern drag-and-drop look, see Vikunja.
- Plugins are installed by copying them into `plugins`; only install ones you trust, they run as PHP code inside the app.
- For many users or heavy use, Kanboard also supports PostgreSQL and MySQL through the `DATABASE_URL` variable.

## Related

- [Vikunja tasks and projects](kb:apps/vikunja)
- [Snipe-IT asset inventory](kb:apps/snipe-it)
- [Repair bench server](kb:guides/repair-bench-server)
