/**
 * "Show me" targets: stable `data-guide` ids on real controls, with the view
 * they live in and a short description the model sees. DRIFT can navigate
 * to a view and spotlight one of these; it never clicks anything.
 * `guide.test.ts` checks that every id exists in the markup.
 */
import { SECTIONS, type SectionId } from "../sections";

export interface GuideTarget {
  id: string;
  view: SectionId | null; // null: visible everywhere (sidebar, top bar)
  what: string;
}

const rail: GuideTarget[] = SECTIONS.map((s) => ({ id: `rail-${s.id}`, view: null, what: `sidebar button for the ${s.label} view` }));

export const GUIDE_TARGETS: GuideTarget[] = [
  ...rail,
  { id: "rail-collapse", view: null, what: "collapse or expand the sidebar" },
  { id: "palette", view: null, what: "command palette / search (Mod+K)" },
  { id: "hosts-add", view: null, what: "add a host (+ in the sidebar)" },
  { id: "hosts-import", view: null, what: "import hosts from ~/.ssh/config (IMP)" },
  { id: "docker-containers", view: "docker", what: "Docker containers tab" },
  { id: "docker-stacks", view: "docker", what: "Docker stacks (Compose projects) tab; each stack has Updates (what changed before pulling) and Snapshots (roll back)" },
  { id: "docker-new-stack", view: "docker", what: "deploy wizard: create a new stack from an image or a compose file (in the Stacks tab)" },
  { id: "docker-routes-tab", view: "docker", what: "Routes tab: reverse proxy domains (Caddy, Traefik, Nginx Proxy Manager), the container behind each, certificate expiry" },
  { id: "services-add", view: "services", what: "add a service to the board" },
  { id: "services-import", view: "services", what: "import services from a Homepage services.yaml" },
  { id: "services-integrations", view: "services", what: "add an integration (Plex, AMP, Uptime Kuma, Portainer)" },
  { id: "tunnels-add", view: "tunnels", what: "add an SSH tunnel" },
  { id: "system-tab-units", view: "system", what: "systemd services tab" },
  { id: "system-tab-journal", view: "system", what: "journal (live logs) tab" },
  { id: "system-tab-schedules", view: "system", what: "scheduled jobs tab: systemd timers and cron, next and last runs" },
  { id: "system-tab-timeline", view: "system", what: "incident timeline of the host: alerts, boots, reboots, container crashes, failed units, OOM kills, package upgrades, compose runs and automation, newest first" },
  { id: "system-tab-security", view: "system", what: "security tab: hygiene score, checks with fixes, listening ports" },
  { id: "system-port-map", view: "system", what: "port map: open ports on every host (in the Security tab)" },
  { id: "system-tab-updates", view: "system", what: "package updates tab" },
  { id: "system-tab-disks", view: "system", what: "disk SMART health tab" },
  { id: "system-tab-backups", view: "system", what: "backup status board tab: restic, borg, rsync and rclone jobs on every host, last run, size, failed or stale" },
  { id: "system-tab-vms", view: "system", what: "VMs tab: libvirt/KVM and Proxmox VMs and LXC containers on the host; start, shut down, reboot, force stop, console" },
  { id: "vms-root", view: "system", what: "read virtual machines as root (when this login may not use virsh, qm or pct)" },
  { id: "backups-add", view: "system", what: "add a backup job to the board (a systemd timer or a repository path)" },
  { id: "backups-detect", view: "system", what: "find backup jobs in the host's systemd timers and cron tables" },
  { id: "automation-command", view: "automation", what: "command box for running on several hosts" },
  { id: "automation-run", view: "automation", what: "Run button for a multi-host command" },
  { id: "automation-new-snippet", view: "automation", what: "create a snippet (saved command)" },
  { id: "automation-new-playbook", view: "automation", what: "create a playbook" },
  { id: "term-key-hints", view: "terminal", what: "key hint strip under the terminal: keys of the program in front (nano, vim, less, htop, tmux …); turn off in Settings → Terminal" },
  { id: "netmap-scan", view: "network", what: "scan the network from the chosen host (neighbours, mDNS, Tailscale peers, router port forwards; optional ping sweep)" },
  { id: "netmap-ports", view: "network", what: "audit listening ports on every host for the map" },
  { id: "netmap-graph", view: "network", what: "network map: internet, router and tailnet, your hosts, other devices; red lines are internet-facing" },
  { id: "netmap-add-host", view: "network", what: "add a discovered device as a host (select an unknown device first)" },
  { id: "netmap-label", view: "network", what: "label a device that isn't a host: name, type, #tags and a note (select the device first)" },
  { id: "notes-new", view: "notes", what: "write a new note or runbook" },
  { id: "notes-list", view: "notes", what: "list of notes with search, host and tag filters" },
  { id: "notes-view", view: "notes", what: "the open note; code blocks have Send to terminal and Copy" },
  { id: "terminal-history", view: "terminal", what: "search the shell history of the host in the focused terminal (turn on in Settings → Terminal)" },
  { id: "settings-theme", view: "settings", what: "theme picker" },
  { id: "settings-display", view: "settings", what: "display settings: effects, UI scale, handheld layout, controller" },
  { id: "settings-terminal", view: "settings", what: "terminal settings: font, scrollback, edit files in Nightgrid" },
  { id: "settings-monitoring", view: "settings", what: "monitoring settings: sample interval, history" },
  { id: "settings-services", view: "settings", what: "service check settings" },
  { id: "settings-alerts", view: "settings", what: "alert rules (offline, disk, containers, certificates)" },
  { id: "settings-notifications", view: "settings", what: "notifications, quiet hours, webhook" },
  { id: "settings-desktop", view: "settings", what: "tray and start-up behaviour" },
  { id: "settings-updates", view: "settings", what: "app updates" },
  { id: "settings-backup", view: "settings", what: "encrypted backup export and import, sync folder" },
  { id: "settings-status-page", view: "settings", what: "phone status page: read-only page with hosts, alerts and services on the LAN or Tailscale, secret link" },
  { id: "dashboard-export", view: "settings", what: "save a static HTML snapshot of the dashboard (hosts, alerts, services)" },
  { id: "system-power", view: "system", what: "power buttons for the selected host: Wake (Wake-on-LAN), Suspend, Shut down, Reboot, and Group… for a whole group" },
  { id: "system-group-power", view: "system", what: "suspend, shut down or reboot every host in a group (type the group name to confirm)" },
  { id: "system-power-schedule", view: "system", what: "power schedule (Schedules tab): shut down, suspend, reboot or wake the host at set times and days, as a systemd timer on the host" },
  { id: "timeline-range", view: "system", what: "timeline range: last 24 hours, 7, 30 or 90 days" },
  { id: "timeline-filters", view: "system", what: "timeline filters: alerts, boots and power, containers and stacks, units and OOM, packages, automation, problems only" },
  { id: "timeline-summary", view: "system", what: "Summarize: DRIFT reads the shown timeline and explains the incidents, likely causes and what needs attention" },
  { id: "settings-tailnet", view: "settings", what: "built-in Tailscale: turn on, log in, use subnet routes, list tailnet devices, device name and control server" },
  { id: "settings-keys", view: "settings", what: "SSH keys: list your key pairs and agent keys, copy a public key, make a new ed25519 key, check which hosts accept which key, add or remove a key on a host, rotate a key across hosts" },
  { id: "keys-new", view: "settings", what: "make a new ed25519 SSH key (file name, comment, optional passphrase)" },
  { id: "keys-check", view: "settings", what: "read every host's authorized_keys to see which of your keys each host accepts" },
  { id: "keys-matrix", view: "settings", what: "table of hosts × keys: + adds a key to a host, ● removes it (typed confirmation), OTHER lists keys on the host that aren't yours" },
  { id: "keys-rotate", view: "settings", what: "rotate a key: pick the old and new key and the hosts; adds the new key, tests login, switches Nightgrid to it, then removes the old key" },
  { id: "tailnet-login", view: "settings", what: "LOG IN button for the built-in Tailscale (shown when it needs a login)" },
  { id: "settings-helper", view: "settings", what: "DRIFT helper settings and model download" },
  { id: "settings-keybindings", view: "settings", what: "list of keyboard shortcuts" },
  { id: "settings-diagnostics", view: "settings", what: "log folder and diagnostics" },
];

export const guideTarget = (id: string): GuideTarget | undefined => GUIDE_TARGETS.find((t) => t.id === id);

export const VIEW_IDS: SectionId[] = SECTIONS.map((s) => s.id);
