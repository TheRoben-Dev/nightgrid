# Roadmap

What's planned after the shipped milestones (see `CHANGELOG.md` for what's done). The order can change; M11 stays last.

| Batch | Scope |
| --- | --- |
| **M10i** · Network map | Visual graph of hosts, containers and ports; opt-in LAN discovery from a chosen host (neighbour table, ping sweep, mDNS, Tailscale peers, Docker networks), click to scan only, unknown devices → "Add as host"; internet-facing ports highlighted separately from LAN-only (public IPs, UPnP mappings, Tailscale Funnel, cloudflared tunnels, proxy routes), no external probe service. Also: a Custom color scheme in place of Plain (curated swatches per palette slot), and DRIFT reads Notes (indexed with the docs, re-indexed on save, answers cite the note; per-note "hide from DRIFT"). |
| **M10j** · Built-in Tailscale | Opt-in embedded tailnet client (log in once with a link, node state in the keychain); SSH, SFTP, Docker, metrics and tunnels dial through it; accept subnet routes; MagicDNS names; per-host tailnet fallback address with automatic LAN ↔ tailnet switching; falls back to a system Tailscale when one is connected. Also: terminal key hints (the session-only shell hook reports the program in front; a hint strip shows its keys, e.g. nano, vim, less, htop, tmux; data-driven so DRIFT reads the same maps; can be turned off). |
| **M10k** · Keys & power | SSH key manager (generate, push to hosts, rotate, which key works where); Wake-on-LAN, reboot and shutdown per host or group, with schedules. |
| **M10l** · Watch & history | Expiry watch for certificates, domains and tokens; config drift diff (`/etc`, compose files, packages) between hosts or against a saved baseline; per-host incident timeline (alerts, restarts, deploys, rollbacks) with a DRIFT summary. |
| **M10m** · VM desktops | Graphical guest consoles in a tab: embedded noVNC streamed over the existing SSH connection (`qm vncproxy`, libvirt's local VNC), no API token or open ports; one-click RDP for Windows guests through an SSH tunnel and the system RDP client. |
| **M10n** · Layout customization | Dashboard edit mode (drag, resize, add, remove widgets, reset); per-workspace layouts and named presets; layouts included in profile export. |
| **M10o** · DRIFT skills | Short, grounded tasks a small local model does well: explain a command before running it (flags risky ones like `rm -rf`, `chmod 777`, `--force`); plain words → a command draft you review and send to a terminal (never run by itself); compose/config review (missing restart policies, `:latest`, ports on 0.0.0.0, inline secrets); a daily summary from Nightgrid's own data (restarts, disk, pending updates, alerts); alert triage (likely cause + next step, linked to the view); terminal output or history → a draft runbook note; cron/regex/jq helper both ways; offline error lookup (exit codes, systemd states, common Docker errors) from a bundled reference that M11 extends. |
| **M10p** · DRIFT as a guide | Multi-step walkthroughs across views (spotlight, wait for the user's action, next step; never clicks); "what can I do here?"; replayable first-run tours. |
| **M10q** · Proxmox suite (later) | Full Proxmox VE management in the app, so its web UI isn't needed: create, clone and template VMs and containers; ISOs and storage; snapshots and backups (vzdump, Proxmox Backup Server); hardware edits (CPU, memory, disks, NICs, PCI/USB passthrough); bridges and networking; node and cluster status; task log; firewall. Builds on M10h and M10m. Scheduled once a Proxmox host is in use. |
| **M11** · Knowledge base | Offline articles and quest-log-style guides with their own images, cross-linked with back/forward; indexed by DRIFT; decode effect on open. |

## Later: after the standalone app is finished

Not scheduled. Nothing here gets built until the app is more or less complete as a standalone product (after M11 and the Proxmox suite).

| Idea | Scope |
| --- | --- |
| **Nightgrid node** · optional companion service | Opt-in, single static Rust binary deployed to a host over SSH; opens no ports and talks to the app only through the SSH tunnel or the tailnet. Read-only by default with per-host allowed actions; one-click uninstall; updates pushed by the app. What it adds: metric history and alerts while the app is closed; scheduled jobs that run with this computer off; a Wake-on-LAN relay for other LAN devices; event streaming instead of polling; server-side log anomaly detection and daily-summary prep for DRIFT; hosting the phone status page. |
