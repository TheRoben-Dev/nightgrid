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
  { id: "docker-stacks", view: "docker", what: "Docker stacks (Compose projects) tab" },
  { id: "services-add", view: "services", what: "add a service to the board" },
  { id: "services-import", view: "services", what: "import services from a Homepage services.yaml" },
  { id: "services-integrations", view: "services", what: "add an integration (Plex, AMP, Uptime Kuma, Portainer)" },
  { id: "tunnels-add", view: "tunnels", what: "add an SSH tunnel" },
  { id: "system-tab-units", view: "system", what: "systemd services tab" },
  { id: "system-tab-journal", view: "system", what: "journal (live logs) tab" },
  { id: "system-tab-schedules", view: "system", what: "scheduled jobs tab: systemd timers and cron, next and last runs" },
  { id: "system-tab-updates", view: "system", what: "package updates tab" },
  { id: "system-tab-disks", view: "system", what: "disk SMART health tab" },
  { id: "automation-command", view: "automation", what: "command box for running on several hosts" },
  { id: "automation-run", view: "automation", what: "Run button for a multi-host command" },
  { id: "automation-new-snippet", view: "automation", what: "create a snippet (saved command)" },
  { id: "automation-new-playbook", view: "automation", what: "create a playbook" },
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
  { id: "settings-helper", view: "settings", what: "DRIFT helper settings and model download" },
  { id: "settings-keybindings", view: "settings", what: "list of keyboard shortcuts" },
  { id: "settings-diagnostics", view: "settings", what: "log folder and diagnostics" },
];

export const guideTarget = (id: string): GuideTarget | undefined => GUIDE_TARGETS.find((t) => t.id === id);

export const VIEW_IDS: SectionId[] = SECTIONS.map((s) => s.id);
