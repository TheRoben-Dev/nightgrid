# Get the most out of DRIFT

DRIFT is Nightgrid's offline helper. It answers from this Library, your notes and live facts about your hosts, checks commands before you run them, and walks you through views. Nothing leaves this computer.

## What you end up with

- Answers grounded in the Library and your own notes, with sources listed
- Exact checks for commands, compose files, errors and cron expressions
- Guided tours that point at controls without clicking them

## Part 1 · Turn it on

- [ ] **Settings → Helper**: turn it on and **Download** the model once (about 2.5 GB, resumable, checked by SHA-256).
- [ ] Open DRIFT with `Mod+J`, the **DRIFT** button, or **Ask DRIFT** in the palette.

## Part 2 · Ask well

- [ ] Name the host: "why is the media host slow?" gets its live state, last error and alerts.
- [ ] Select log lines or an error first; they attach as chips. Click a chip to leave it out.
- [ ] Put a command in backticks for a **command check**: "is `rm -rf $DIR/` safe?".
- [ ] Attach a compose file for a **compose review**.
- [ ] Ask about an app: "how do I set up Vaultwarden?" pulls the matching Library guide.
- [ ] "give me a daily summary" covers the last 24 hours of alerts, restarts and checks.

## Part 3 · Act on answers

- [ ] **Show me** switches view and outlines the control.
- [ ] **TERMINAL** on a one-line command types it at a prompt; you press Enter. DANGER commands need a second click.
- [ ] **Save as runbook note** turns an answer into a note.
- [ ] **What I was given** shows the exact prompt and sources.

## Part 4 · Tours

- [ ] **HERE?** at the top of the panel lists the controls in the current view and the tours for it.
- [ ] Tours: Getting around, Add your first host, Customize the Overview, Watch your services, Check fleet health, Back up your setup and Open a VM's screen. They don't need the model.

## Part 5 · Shell history (optional)

- [ ] Tick **DRIFT may read shell history** in a host's editor so "when did I last restart nginx?" works. Secret-looking lines are dropped; nothing is kept.

## Limits and gotchas

- Answers can be wrong. Read commands before running them; Nightgrid still confirms anything destructive.
- The model runs on the GPU on macOS and the CPU elsewhere; long answers are slower on CPU. **Free memory** unloads it.
- Notes ticked **Hide from DRIFT** are never read.

## Related

- [Write runbooks for alerts](kb:howto/runbooks)
- [Work faster in the terminal](kb:howto/terminal)
- [Local AI GPU server](kb:guides/local-ai-gpu-server)
