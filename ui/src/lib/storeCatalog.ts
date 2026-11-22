// The feature store (D-206): optional components installed from
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
  /** What it adds and where it shows up once installed (shown on the card). */
  adds: string[];
  /** One real thing you'd use it for. */
  example: string;
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
  { id: "proxmox", label: "Proxmox", shelf: "virtualization", hint: "Proxmox VE nodes, guests, snapshots, backups, hardware, network, firewall and throwaway sandboxes.", features: ["proxmox"], adds: ["VMs → Proxmox view (Mod+Shift+P)", "Guests, snapshots, backups, create/clone VMs and containers", "HARDWARE, NETWORK and FIREWALL tabs per node", "Sandboxes: throwaway clones that expire"], example: "Snapshot a VM before an upgrade and roll it back from Nightgrid, no Proxmox web UI needed." },
  { id: "desktops", label: "VM desktops", shelf: "virtualization", hint: "VM screens over VNC (built-in client) and RDP through an SSH tunnel.", features: ["desktops"], adds: ["VMs → Desktops view (Mod+Shift+M)", "Built-in VNC client over SSH", "RDP through an SSH tunnel to your system client"], example: "Open a Windows VM's screen from your laptop without exposing VNC or RDP to the network." },
  { id: "ups", label: "UPS", shelf: "monitoring", hint: "NUT readings, battery runtime and the shutdown countdown banner.", features: ["fleet.ups"], adds: ["System → Fleet → UPS tab", "Battery, load and runtime from NUT", "Shutdown countdown banner and UPS alerts"], example: "See how many minutes the NAS has left on battery during a power cut." },
  { id: "logsearch", label: "Log search", shelf: "monitoring", hint: "Search logs across hosts, saved searches and LOG MATCH alerts.", features: ["fleet.logs"], adds: ["System → Fleet → LOGS tab", "One search across journals and container logs on every host", "Saved searches and LOG MATCH alerts"], example: "Find every “out of memory” line across all hosts in the last 24 hours." },
  { id: "expiry", label: "Expiry watch", shelf: "monitoring", hint: "Certificate, domain and token expiry dates with alerts.", features: ["services.expiry"], adds: ["Expiry panel in the Services view", "Certificate, domain and token expiry dates", "Alerts before something expires"], example: "Get warned 14 days before a reverse proxy certificate runs out." },
  { id: "power", label: "Power & cost", shelf: "monitoring", hint: "Estimated watts, kWh and cost per host; idle-hour hints.", features: ["fleet.power"], adds: ["System → Fleet → POWER tab", "Estimated watts, kWh and cost per host", "Hints for hours a host sits idle"], example: "Work out what the old desktop server costs per month to keep running." },
  { id: "capacity", label: "Capacity planner", shelf: "monitoring", hint: "Will another workload fit? Headroom from real history.", features: ["fleet.capacity"], adds: ["System → Fleet → CAPACITY tab", "Headroom from real CPU, memory and disk history"], example: "Check whether a new game server container still fits on a host before deploying it." },
  { id: "replay", label: "Incident replay", shelf: "monitoring", hint: "Scrub through a past outage: metrics, alerts, events and logs side by side.", features: ["fleet.replay"], adds: ["System → Fleet → REPLAY tab", "Metrics, alerts, events, changes and logs on one timeline"], example: "Scrub through last night's outage to see which change came right before it." },
  { id: "node", label: "Nightgrid Node", shelf: "monitoring", hint: "Use the optional Node agent where installed: gap-free history and alerts while the app is closed.", features: ["fleet.node"], adds: ["System → Fleet → NODE tab (hub setup and tools)", "History without gaps and alerts while the app was closed", "Live samples without SSH polling"], example: "Install the Node agent on a server so its graphs keep recording while your laptop is off." },
  { id: "automation", label: "Automation", shelf: "operations", hint: "Snippets and playbooks run across hosts.", features: ["automation"], adds: ["Automation view", "Snippets with {{variables}} run on many hosts at once", "Playbooks with danger checks"], example: "Run apt upgrade on every Debian host in one go and read the output per host." },
  { id: "journal", label: "Journal", shelf: "operations", hint: "A changelog of every change made from Nightgrid, plus your own entries.", features: ["fleet.journal"], adds: ["System → Fleet → JOURNAL tab", "Every change made from Nightgrid, plus your own entries"], example: "Look up who restarted a container last Tuesday and why." },
  { id: "blueprints", label: "Blueprints", shelf: "operations", hint: "Save a host's setup as an encrypted file and rebuild it elsewhere.", features: ["fleet.blueprints"], adds: ["System → Fleet → BLUEPRINTS tab", "A host's setup saved as an encrypted file", "Rebuild it on another host"], example: "Move a host's stacks, cron jobs and config to new hardware." },
  { id: "catalog", label: "Service catalog", shelf: "operations", hint: "Every app with image, version, ports, volumes and covering backup; Markdown export.", features: ["fleet.catalog"], adds: ["System → Fleet → CATALOG tab", "Every app with image, version, ports, volumes and covering backup", "Markdown export"], example: "Print one page that lists every service and where its data is backed up." },
  { id: "drills", label: "Restore drills", shelf: "operations", hint: "Scheduled test restores with a pass/fail report.", features: ["fleet.drills"], adds: ["System → Fleet → DRILLS tab", "Scheduled test restores with a pass/fail report"], example: "Prove each month that the database backup actually restores." },
  { id: "netmap", label: "Network map", shelf: "network", hint: "Hosts, containers and ports as a graph; opt-in LAN discovery.", features: ["network"], adds: ["Network → Map", "Hosts, containers and ports as a graph", "Opt-in LAN discovery from a host you pick"], example: "Spot which ports are open to the internet across the whole homelab." },
  { id: "speed", label: "Speed tests", shelf: "network", hint: "iperf3 between hosts.", features: ["fleet.speed"], adds: ["System → Fleet → SPEED tab", "iperf3 tests between any two hosts"], example: "Check whether the NAS link really runs at 2.5 GbE." },
  { id: "security", label: "Security", shelf: "security", hint: "Posture score per host with a trend, and every secret's age and rotation date.", features: ["security"], adds: ["Security view", "Posture score per host with a trend", "Every secret's age and rotation date"], example: "See which host lost points after a config change, and which SSH key is two years old." },
  { id: "access", label: "Access audit", shelf: "security", hint: "Who logged in where, across hosts.", features: ["fleet.access"], adds: ["System → Fleet → ACCESS tab", "Logins across hosts, nothing stored"], example: "Check who logged in to the NAS over SSH this week." },
  { id: "drift", label: "DRIFT helper", shelf: "extras", hint: "The local AI helper and its tours. The model (2.5 GB) downloads separately when first used.", features: ["drift"], adds: ["DRIFT helper panel and its guided tours", "Answers about Nightgrid and your hosts, all offline", "The model (2.5 GB) downloads when first used"], example: "Ask “why is the backup alert red?” and get pointed at the right tab." },
  { id: "sound", label: "UI sounds", shelf: "extras", hint: "Short synthesized blips for alerts, actions, dialogs and quests.", features: ["sound"], adds: ["Short blips for alerts, actions, dialogs and quests", "Volume and categories in Settings → SOUND"], example: "Hear when a critical alert lands while you're in another app." },
];

/** What the core app holds without installing anything (Settings → STORE). */
export const CORE: { label: string; what: string }[] = [
  { label: "Hosts & Overview", what: "SSH hosts, dashboards with tiles, alerts and the inbox" },
  { label: "Terminal & Files", what: "SSH and local terminals, split panes, SFTP browser and editor" },
  { label: "Docker", what: "Containers, compose stacks, images, logs and shells" },
  { label: "Services", what: "Health checks and the status board" },
  { label: "System", what: "Units, journal, firewall, cron, updates and power for one host" },
  { label: "Fleet", what: "Updates, disks, backups and uptime across all hosts" },
  { label: "Network", what: "Tunnels and network tools (ping, DNS, ports, Wake-on-LAN)" },
  { label: "Knowledge", what: "Your notes and runbooks, plus the Library" },
  { label: "Settings", what: "Profiles, device sync, SSH keys, built-in Tailscale, themes" },
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
