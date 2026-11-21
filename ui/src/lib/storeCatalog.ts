// The feature store (D-205): optional components installed from
// Settings → STORE. The core app keeps the standard homelab essentials;
// everything below is a component. Not installed = hidden, never loaded, no
// background work, data kept. The backend has the same list
// (core/src/components.rs COMPONENTS; a test keeps them in step).

export type StoreShelf = "virtualization" | "monitoring" | "operations" | "network" | "security" | "extras";

export interface StoreComponent {
  id: string;
  label: string;
  shelf: StoreShelf;
  hint: string;
  /** Feature ids (Settings → Features) it brings, with their children. */
  features: string[];
}

export const SHELVES: { id: StoreShelf; label: string }[] = [
  { id: "virtualization", label: "VIRTUALIZATION" },
  { id: "monitoring", label: "MONITORING" },
  { id: "operations", label: "OPERATIONS" },
  { id: "network", label: "NETWORK" },
  { id: "security", label: "SECURITY" },
  { id: "extras", label: "EXTRAS" },
];

export const COMPONENTS: StoreComponent[] = [
  { id: "proxmox", label: "Proxmox", shelf: "virtualization", hint: "Proxmox VE nodes, guests, snapshots, backups, hardware, network, firewall and throwaway sandboxes.", features: ["proxmox"] },
  { id: "desktops", label: "VM desktops", shelf: "virtualization", hint: "VM screens over VNC (built-in client) and RDP through an SSH tunnel.", features: ["desktops"] },
  { id: "ups", label: "UPS", shelf: "monitoring", hint: "NUT readings, battery runtime and the shutdown countdown banner.", features: ["fleet.ups"] },
  { id: "logsearch", label: "Log search", shelf: "monitoring", hint: "Search logs across hosts, saved searches and LOG MATCH alerts.", features: ["fleet.logs"] },
  { id: "expiry", label: "Expiry watch", shelf: "monitoring", hint: "Certificate, domain and token expiry dates with alerts.", features: ["services.expiry"] },
  { id: "power", label: "Power & cost", shelf: "monitoring", hint: "Estimated watts, kWh and cost per host; idle-hour hints.", features: ["fleet.power"] },
  { id: "capacity", label: "Capacity planner", shelf: "monitoring", hint: "Will another workload fit? Headroom from real history.", features: ["fleet.capacity"] },
  { id: "replay", label: "Incident replay", shelf: "monitoring", hint: "Scrub through a past outage: metrics, alerts, events and logs side by side.", features: ["fleet.replay"] },
  { id: "node", label: "Nightgrid Node", shelf: "monitoring", hint: "Use the optional Node agent where installed: gap-free history and alerts while the app is closed.", features: ["fleet.node"] },
  { id: "automation", label: "Automation", shelf: "operations", hint: "Snippets and playbooks run across hosts.", features: ["automation"] },
  { id: "journal", label: "Journal", shelf: "operations", hint: "A changelog of every change made from Nightgrid, plus your own entries.", features: ["fleet.journal"] },
  { id: "blueprints", label: "Blueprints", shelf: "operations", hint: "Save a host's setup as an encrypted file and rebuild it elsewhere.", features: ["fleet.blueprints"] },
  { id: "catalog", label: "Service catalog", shelf: "operations", hint: "Every app with image, version, ports, volumes and covering backup; Markdown export.", features: ["fleet.catalog"] },
  { id: "drills", label: "Restore drills", shelf: "operations", hint: "Scheduled test restores with a pass/fail report.", features: ["fleet.drills"] },
  { id: "netmap", label: "Network map", shelf: "network", hint: "Hosts, containers and ports as a graph; opt-in LAN discovery.", features: ["network"] },
  { id: "speed", label: "Speed tests", shelf: "network", hint: "iperf3 between hosts.", features: ["fleet.speed"] },
  { id: "security", label: "Security", shelf: "security", hint: "Posture score per host with a trend, and every secret's age and rotation date.", features: ["security"] },
  { id: "access", label: "Access audit", shelf: "security", hint: "Who logged in where, across hosts.", features: ["fleet.access"] },
  { id: "drift", label: "DRIFT helper", shelf: "extras", hint: "The local AI helper and its tours. The model (2.5 GB) downloads separately when first used.", features: ["drift"] },
  { id: "sound", label: "UI sounds", shelf: "extras", hint: "Short synthesized blips for alerts, actions, dialogs and quests.", features: ["sound"] },
];

export const componentById = (id: string): StoreComponent | undefined => COMPONENTS.find((c) => c.id === id);

const under = (id: string, root: string) => id === root || id.startsWith(`${root}.`);

/** The component a feature id belongs to (undefined: part of the core app). */
export const ownerOf = (feature: string): StoreComponent | undefined => COMPONENTS.find((c) => c.features.some((f) => under(feature, f)));

/** Whether a feature can run with these components installed (null: never chosen = all). */
export function installedFor(installed: readonly string[] | null | undefined, feature: string): boolean {
  const c = ownerOf(feature);
  return !c || installed == null || installed.includes(c.id);
}

/** The installed list after installing or removing a component. Unknown ids are kept. */
export function setInstalled(installed: readonly string[] | null | undefined, id: string, on: boolean): string[] {
  const all = installed ?? COMPONENTS.map((c) => c.id);
  const rest = all.filter((x) => x !== id);
  return on ? [...rest, id] : rest;
}
