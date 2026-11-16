import type { ActionId } from "./keys";

export type SectionId = "overview" | "terminal" | "files" | "docker" | "services" | "tunnels" | "system" | "fleet" | "desktops" | "network" | "automation" | "notes" | "settings";

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
  { id: "desktops", label: "Desktops", code: "DSK", action: "nav.desktops", milestone: "M10n" },
  { id: "network", label: "Network", code: "NET", action: "nav.network", milestone: "M10i" },
  { id: "automation", label: "Automation", code: "AUT", action: "nav.automation", milestone: "M8" },
  { id: "notes", label: "Notes", code: "NTS", action: "nav.notes", milestone: "M10e" },
  { id: "settings", label: "Settings", code: "CFG", action: "nav.settings", milestone: "M0" },
];

export const sectionById = (id: SectionId): Section => SECTIONS.find((s) => s.id === id) ?? SECTIONS[0];
