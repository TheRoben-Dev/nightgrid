# Starters

Ready-made setups and runbooks shown in Nightgrid under **Fleet → BLUEPRINTS → STARTERS** and **Knowledge → Starters** (D-177). They are data, read by `core/src/starters.rs`; every file here must be listed in `BLUEPRINT_FILES` or `RUNBOOK_FILES` there (a test checks).

Rules for every starter:

- Generic: no host names, addresses, domains or personal details. Use placeholders.
- No secrets. Anything secret is a prompt with `secret: true` (and usually `generate: true`), named so the `.env` vault also treats it as secret (`*_PASSWORD`, `*_TOKEN`, `*_KEY`…), with no default.
- Explain the purpose in plain words, and link at least one Library article (`shelf/slug` under `docs/kb`).
- Nothing applies without the user previewing the plan and confirming.

## Blueprints (`blueprints/<id>.yaml`)

```yaml
id: media-server            # same as the file name
title: Media server
summary: One line for the card.
category: media             # a Library topic id
purpose: |
  Plain words: what it is for, what it opens, what stays private.
library: [apps/jellyfin, apps/arr-stack]
stacks:
  - name: media             # compose project and folder name
    compose: |              # written as <folder>/media/compose.yaml
      services: ...         # every ${VAR} used here needs an env prompt
    files:                  # optional extra files, relative to the stack folder
      - { path: conf/app.conf, text: "..." }
    env:
      - name: TZ
        label: Time zone    # the question asked
        default: Etc/UTC
        help: Optional hint.
        secret: false       # masked, kept as a vault secret
        generate: false     # empty → 32 random letters and digits
        path: false         # must be an absolute folder
    ports:
      - { port: 8096/tcp, what: Jellyfin }            # reachable on the LAN: also in firewall.allow
      - { port: 8989/tcp, what: Sonarr, local: true } # bound to 127.0.0.1
firewall:                   # optional
  allow: [8096/tcp]
  limit: [22/tcp]           # ufw rate limit
  denyIncoming: false       # suggested only: always a step by hand
  enable: false             # suggested only: always a step by hand
timers:                     # optional Nightgrid timers (systemd, as root unless runAs)
  - { name: docker-image-prune, command: docker image prune -af, calendar: "Sun *-*-* 04:30:00", persistent: true }
manual:                     # steps left to the user, shown as BY HAND
  - { title: Finish the wizard, detail: Open http://<host>:8096 ... }
```

## Runbooks (`runbooks/<id>.md`)

A YAML header between `---` lines, then the note's Markdown (`- [ ]` steps, fenced commands, `[label](view:id)` and `[label](kb:shelf/slug)` links):

```markdown
---
id: disk-full
title: "Runbook: disk full or filling up"
summary: One line for the list.
alertKinds: [diskFull, diskFilling]   # empty: not offered on alerts
tags: [runbook, starter, disk]
library: [linux/disk-full]
---
What the alert means, then the steps.
```
