// Settings → Features (0.10.48): what can be turned off, and the presets.
// Off hides the feature (sidebar, palette, keys, DRIFT) and stops its
// background work; its data is kept and comes back when it's turned on.
// The backend knows the same ids (core/src/settings.rs FEATURES).

import type { SectionId } from "./sections";

export interface Feature {
  id: string;
  label: string;
  hint: string;
  /** Turning the parent off turns this off too. */
  parent?: string;
}

export const FEATURES: Feature[] = [
  { id: "terminal", label: "Terminal", hint: "SSH and local terminals, split panes, snippets in the terminal." },
  { id: "files", label: "Files", hint: "SFTP browser and the in-app editor." },
  { id: "docker", label: "Docker", hint: "Containers, compose stacks, images. Off: no container-down alerts." },
  { id: "services", label: "Services", hint: "Health checks for web services. Off: no background checks." },
  { id: "services.expiry", parent: "services", label: "Expiry watch", hint: "Certificate, domain and token dates. Off: no expiry alerts or certificate re-reads." },
  { id: "tunnels", label: "Tunnels", hint: "SSH port forwards. Off: auto-start tunnels don't start." },
  { id: "system", label: "System", hint: "Units, journal, firewall, updates and power for one host." },
  { id: "fleet", label: "Fleet", hint: "Checks across all hosts. Off: none of the checks below run." },
  { id: "fleet.updates", parent: "fleet", label: "Updates", hint: "Pending packages and image updates across hosts." },
  { id: "fleet.disks", parent: "fleet", label: "Disks", hint: "SMART, pools, fill-up forecast. Off: no SMART or pool scans." },
  { id: "fleet.backups", parent: "fleet", label: "Backups", hint: "restic, borg and rsync jobs. Off: no backup checks." },
  { id: "fleet.ups", parent: "fleet", label: "UPS", hint: "NUT readings and the shutdown countdown. Off: UPS is not read." },
  { id: "fleet.uptime", parent: "fleet", label: "Uptime", hint: "Uptime history and monthly reports. Off: nothing is recorded." },
  { id: "fleet.logs", parent: "fleet", label: "Log search", hint: "Cross-host log search. Off: no saved-search or anomaly alerts." },
  { id: "fleet.access", parent: "fleet", label: "Access audit", hint: "Who logged in where." },
  { id: "fleet.speed", parent: "fleet", label: "Speed tests", hint: "iperf3 between hosts." },
  { id: "desktops", label: "Desktops", hint: "VM screens over VNC and RDP." },
  { id: "proxmox", label: "Proxmox", hint: "Proxmox VE nodes, guests, storage, network and firewall." },
  { id: "network", label: "Network map", hint: "Hosts, containers and ports as a graph; LAN discovery." },
  { id: "nettools", label: "Network tools", hint: "Ping, path, DNS, port check, HTTP/TLS, Wake-on-LAN, subnet and MAC lookups." },
  { id: "automation", label: "Automation", hint: "Snippets and playbooks across hosts." },
  { id: "notes", label: "Knowledge", hint: "Your notes and runbooks (and, later, the built-in guides)." },
  { id: "drift", label: "DRIFT helper", hint: "The local AI helper. Off: the model is never loaded." },
];

/** Sections that can't be turned off. */
const ALWAYS: SectionId[] = ["overview", "settings"];

export const featureById = (id: string): Feature | undefined => FEATURES.find((f) => f.id === id);

/** On unless it or a parent is in the disabled list. */
export function featureOn(disabled: readonly string[], id: string): boolean {
  if ((ALWAYS as string[]).includes(id)) return true;
  return !disabled.some((d) => id === d || id.startsWith(`${d}.`));
}

export function sectionOn(disabled: readonly string[], id: SectionId): boolean {
  return featureOn(disabled, id);
}

export type PresetId = "simple" | "standard" | "everything";

export const PRESETS: { id: PresetId; label: string; hint: string; disabled: string[] }[] = [
  {
    id: "simple",
    label: "SIMPLE",
    hint: "Terminal, files, Docker, services and one host's system view.",
    disabled: ["tunnels", "fleet", "desktops", "proxmox", "network", "nettools", "automation", "notes", "drift", "services.expiry"],
  },
  {
    id: "standard",
    label: "STANDARD",
    hint: "Everyday homelab: no Proxmox, VM screens, UPS, log search, access audit or speed tests.",
    disabled: ["proxmox", "desktops", "fleet.ups", "fleet.logs", "fleet.access", "fleet.speed"],
  },
  {
    id: "everything",
    label: "EVERYTHING",
    hint: "Every view and check.",
    disabled: [],
  },
];

const norm = (list: readonly string[]) => [...new Set(list)].sort().join(",");

/** The preset whose list matches, or null for a custom mix. */
export function presetOf(disabled: readonly string[]): PresetId | null {
  const known = disabled.filter((d) => featureById(d));
  return PRESETS.find((p) => norm(p.disabled) === norm(known))?.id ?? null;
}

/**
 * The disabled list after turning `id` on or off. Unknown ids (from a newer
 * version's profile) are kept. Turning a child on turns its parent on and
 * the parent's other children off, so only the asked one appears.
 */
export function toggleFeature(disabled: readonly string[], id: string, on: boolean): string[] {
  let next = disabled.filter((d) => d !== id);
  if (!on) return [...next, id];
  const f = featureById(id);
  if (f?.parent && next.includes(f.parent)) {
    next = next.filter((d) => d !== f.parent);
    for (const sib of FEATURES.filter((s) => s.parent === f.parent && s.id !== id)) next.push(sib.id);
  }
  return next;
}

/** Applying a preset keeps unknown ids. */
export function applyPreset(disabled: readonly string[], preset: PresetId): string[] {
  const unknown = disabled.filter((d) => !featureById(d));
  return [...unknown, ...(PRESETS.find((p) => p.id === preset)?.disabled ?? [])];
}
