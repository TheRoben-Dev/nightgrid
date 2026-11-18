import type { ActionId } from "./keys";

export type SectionId = "overview" | "terminal" | "files" | "docker" | "services" | "tunnels" | "system" | "fleet" | "security" | "desktops" | "proxmox" | "network" | "nettools" | "automation" | "notes" | "library" | "settings";

export interface Section {
  id: SectionId;
  label: string;
  code: string; // short HUD code shown in the rail
  action: ActionId;
  milestone: string; // milestone that fills the section in
}

export const SECTIONS: Section[] = [
  { id: "overview", label: "Overview", code: "OVR", action: "nav.overview", milestone: "M4" },
  { id: "terminal", label: "Terminal", code: "TRM", action: "nav.terminal", milestone: "M1" },
  { id: "files", label: "Files", code: "FIL", action: "nav.files", milestone: "M2" },
  { id: "docker", label: "Docker", code: "DKR", action: "nav.docker", milestone: "M3" },
  { id: "services", label: "Services", code: "SVC", action: "nav.services", milestone: "M5" },
  { id: "tunnels", label: "Tunnels", code: "TUN", action: "nav.tunnels", milestone: "M5" },
  { id: "system", label: "System", code: "SYS", action: "nav.system", milestone: "M6" },
  { id: "fleet", label: "Fleet", code: "FLT", action: "nav.fleet", milestone: "M10m" },
  { id: "security", label: "Security", code: "SEC", action: "nav.security", milestone: "1.1" },
  { id: "desktops", label: "Desktops", code: "DSK", action: "nav.desktops", milestone: "M10n" },
  { id: "proxmox", label: "Proxmox", code: "PVE", action: "nav.proxmox", milestone: "M10r" },
  { id: "network", label: "Network", code: "NET", action: "nav.network", milestone: "M10i" },
  { id: "nettools", label: "Network tools", code: "NTL", action: "nav.nettools", milestone: "M10i" },
  { id: "automation", label: "Automation", code: "AUT", action: "nav.automation", milestone: "M8" },
  { id: "notes", label: "Knowledge", code: "KNW", action: "nav.notes", milestone: "M10e" },
  { id: "library", label: "Library", code: "LIB", action: "nav.library", milestone: "M11" },
  { id: "settings", label: "Settings", code: "CFG", action: "nav.settings", milestone: "M0" },
];

export const sectionById = (id: SectionId): Section => SECTIONS.find((s) => s.id === id) ?? SECTIONS[0];

/**
 * The sidebar (0.10.50): related views share one entry and switch with tabs
 * under the view title. Every view keeps its id, shortcut and feature toggle;
 * Settings sits in the sidebar's foot.
 */
export interface RailEntry {
  id: string;
  label: string;
  code: string;
  members: SectionId[];
  /** Tab names when the views share the entry (default: the view's label). */
  tabs?: Partial<Record<SectionId, string>>;
}

export const RAIL: RailEntry[] = [
  { id: "overview", label: "Overview", code: "OVR", members: ["overview"] },
  { id: "terminal", label: "Terminal", code: "TRM", members: ["terminal"] },
  { id: "files", label: "Files", code: "FIL", members: ["files"] },
  { id: "docker", label: "Docker", code: "DKR", members: ["docker"] },
  { id: "services", label: "Services", code: "SVC", members: ["services"] },
  { id: "system", label: "System", code: "SYS", members: ["system", "fleet"], tabs: { system: "One host", fleet: "Fleet" } },
  { id: "security", label: "Security", code: "SEC", members: ["security"] },
  { id: "network", label: "Network", code: "NET", members: ["network", "nettools", "tunnels"], tabs: { network: "Map", nettools: "Tools" } },
  { id: "vms", label: "VMs", code: "VMS", members: ["proxmox", "desktops"] },
  { id: "automation", label: "Automation", code: "AUT", members: ["automation"] },
  { id: "notes", label: "Knowledge", code: "KNW", members: ["notes", "library"], tabs: { notes: "Notes", library: "Library" } },
];

/** The sidebar entry a view belongs to (Settings has none: it's in the foot). */
export const railFor = (id: SectionId): RailEntry | undefined => RAIL.find((r) => r.members.includes(id));

/**
 * Where a view lives in the sidebar, in words: "System" for a row's own
 * view, "System → Fleet" for a view that is a tab of another row.
 */
export function railPath(id: SectionId): string {
  const label = sectionById(id).label;
  const r = railFor(id);
  if (!r || r.members.length < 2) return label;
  return `${r.label} → ${r.tabs?.[id] ?? label}`;
}

/**
 * A `rail-<view>` guide id points at the sidebar row that holds the view
 * (`rail-<first member>`). Returns that id and the row to open the view in,
 * or null for any other id (Settings, collapse).
 */
export function railTarget(guideId: string): { id: string; row: string; view: SectionId } | null {
  if (!guideId.startsWith("rail-")) return null;
  const view = guideId.slice(5) as SectionId;
  const r = railFor(view);
  return r ? { id: `rail-${r.members[0]}`, row: r.id, view } : null;
}
