/** Starter presets: alert rules, command tiles, dashboards and log searches ready to add. */
import type { AlertSettings, AlertValue, AppSettings, BackupJob, CmdWidget, Drill, PresetKind, PresetRoute, PresetTile, StarterPreset, TraefikRoute } from "./api";
import { MAX_DASHBOARDS, uniqueName, type Dashboard } from "./dashboard";

/** Gallery shelves for presets, after the blueprint shelves. */
export const PRESET_SHELVES: { id: PresetKind; label: string; what: string }[] = [
  { id: "alerts", label: "ALERTS", what: "Alert rules: which alerts fire and their thresholds. Where they're sent stays your choice in Settings → ALERTS." },
  { id: "tiles", label: "TILES", what: "Command tiles for the Overview. They only read, refresh on their own, and run on the host you pick." },
  { id: "dashboard", label: "DASHBOARDS", what: "Overview dashboards for a kind of box, with their own tiles. Added beside your dashboards, never over them." },
  { id: "searches", label: "LOG SEARCHES", what: "Saved searches for Fleet → LOGS across every host. Some alert when new lines match." },
  { id: "routes", label: "PROXY ROUTES", what: "Names for your apps on a Caddy or Traefik proxy Nightgrid found on a host. You see the change before it's saved." },
  { id: "guests", label: "PROXMOX", what: "VM and container shapes. They open Proxmox → Create with the values filled in; you create it there." },
  { id: "drills", label: "DRILLS", what: "Restore drills for each backup job, so you know the backups restore." },
  { id: "posture", label: "POSTURE", what: "Security scoring and network watch settings, as a set." },
];

const PRESET_IDS = new Set<string>(PRESET_SHELVES.map((s) => s.id));
export const isPresetShelf = (s: string): s is PresetKind => PRESET_IDS.has(s);

/** Plain names for alert settings. */
export const ALERT_LABEL: Record<string, string> = {
  hostOffline: "Host offline",
  containerDown: "Container down",
  diskFull: "Disk full",
  diskPercent: "Disk counts as full at",
  serviceDown: "Service check down",
  certExpiring: "Certificate expiring",
  certDays: "Certificate warning, days ahead",
  smartWarning: "SMART warnings",
  backupStale: "Failed or stale backups",
  diskForecast: "Disk filling up soon",
  forecastDays: "Fill-up forecast, days ahead",
  poolDegraded: "Degraded pools and arrays",
  logAnomaly: "Log alerts",
  logRepeatCount: "Log errors before alerting",
  logRepeatMinutes: "…within minutes",
  logGraceMinutes: "Quiet minutes after a restart",
  logNotFoundNoise: "Ignore 404 noise",
  foldDependent: "Fold alerts under their cause",
  quietHours: "Quiet hours",
  quietStart: "Quiet from",
  quietEnd: "Quiet until",
  postureDaily: "Re-score hosts daily",
  rotationNotices: "Rotation reminders",
  enabled: "Network watch",
  minutes: "Scan every (minutes)",
  sweep: "Ping-sweep the subnet",
  alertNew: "Alert on new devices",
  alertMissing: "Alert on missing devices",
  alertRisks: "Alert on risky ports",
};

const hhmm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;

/** An alert value as words. */
export function alertWords(key: string, v: AlertValue | undefined): string {
  if (v === undefined) return "—";
  if (typeof v === "boolean") return v ? "on" : "off";
  if (key === "quietStart" || key === "quietEnd") return hhmm(v);
  if (key === "diskPercent") return `${v} %`;
  return String(v);
}

export interface AlertChange {
  key: string;
  label: string;
  from: string;
  to: string;
  same: boolean;
}

function changes(section: string, patch: Partial<Record<string, AlertValue>> | undefined, current: object): AlertChange[] {
  const cur = current as Record<string, AlertValue>;
  return Object.entries(patch ?? {}).map(([key, to]) => ({
    key: `${section}.${key}`,
    label: ALERT_LABEL[key] ?? key,
    from: alertWords(key, cur[key]),
    to: alertWords(key, to),
    same: cur[key] === to,
  }));
}

/** What a preset would change in the alert settings `current`. */
export const alertChanges = (p: StarterPreset, current: AlertSettings): AlertChange[] => changes("alerts", p.alerts, current);

/** What a preset would change across alerts, security and network watch. */
export const settingsChanges = (p: StarterPreset, s: AppSettings): AlertChange[] => [
  ...changes("security", p.security, s.security),
  ...changes("netWatch", p.netWatch, s.netWatch),
  ...changes("alerts", p.alerts, s.alerts),
];

/** The settings patch a preset makes. */
export const settingsPatch = (p: StarterPreset, s: AppSettings): Partial<AppSettings> => ({
  alerts: { ...s.alerts, ...(p.alerts ?? {}) } as AppSettings["alerts"],
  security: { ...s.security, ...(p.security ?? {}) } as AppSettings["security"],
  netWatch: { ...s.netWatch, ...(p.netWatch ?? {}) } as AppSettings["netWatch"],
});

/** `current` with the preset's alert values. */
export const withAlerts = (p: StarterPreset, current: AlertSettings): AlertSettings => ({ ...current, ...(p.alerts ?? {}) }) as AlertSettings;

/** A tile as a new command widget on `hostId`. */
export const tileWidget = (t: PresetTile, hostId: number): CmdWidget => ({
  id: 0,
  hostId,
  name: t.name,
  command: t.command,
  parse: t.parse,
  pattern: t.pattern,
  unit: t.unit,
  warn: t.warn,
  crit: t.crit,
  below: t.below,
  intervalSecs: t.intervalSecs,
});

/** The preset's dashboard with `tile:<n>` pointing at saved widget ids, named so it doesn't clash. */
export function resolveDashboard(d: Dashboard, tileIds: number[], names: string[]): Dashboard {
  return {
    name: uniqueName(d.name, names),
    widgets: d.widgets.map((w) => {
      const n = w.kind.startsWith("tile:") ? Number(w.kind.slice(5)) : null;
      if (n == null) return { ...w };
      const id = tileIds[n];
      if (id == null) throw new Error(`${w.kind} has no tile`);
      return { ...w, kind: `cmd:${id}` };
    }),
  };
}

/** `sub.domain`, lowercased; null if the domain isn't a plain name. */
export function routeDomain(r: PresetRoute, domain: string): string | null {
  const d = domain.trim().toLowerCase().replace(/^\.+|\.+$/g, "");
  if (!/^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)*[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(d) || !d.includes(".")) return null;
  return `${r.sub}.${d}`;
}

/** Traefik routes after adding the preset's (names and domains already there are skipped). */
export function withTraefikRoutes(existing: TraefikRoute[], routes: PresetRoute[], domain: string, resolver: string | null): { routes: TraefikRoute[]; added: string[] } {
  const out = [...existing];
  const added: string[] = [];
  for (const r of routes) {
    const host = routeDomain(r, domain);
    if (!host || out.some((x) => x.name === r.name || x.domains.includes(host))) continue;
    out.push({ name: r.name, domains: [host], upstream: r.upstream, entrypoints: [], tls: true, certResolver: resolver });
    added.push(host);
  }
  return { routes: out, added };
}

/** Drills to add: one per backup job that has none. */
export function drillsFor(jobs: BackupJob[], existing: Drill[], everyDays: number): Drill[] {
  const have = new Set(existing.filter((d) => d.kind === "job").map((d) => d.jobId));
  return jobs
    .filter((j) => !have.has(j.id))
    .map((j) => ({ id: null, name: `${j.name} restore`.slice(0, 80), kind: "job", hostId: j.hostId, jobId: j.id, node: null, guestKind: null, vmid: null, storage: null, targetStorage: null, everyDays, enabled: true }));
}

/** Why a dashboard can't be added, or null. */
export const dashboardRoom = (count: number): string | null => (count >= MAX_DASHBOARDS ? `You already have ${MAX_DASHBOARDS} dashboards, the most there can be. Remove one on the Overview first.` : null);

/** Whether the preset needs a host (it brings tiles). */
export const needsHost = (p: StarterPreset): boolean => (p.tiles ?? []).length > 0 || (p.routes ?? []).length > 0;

/** The card's last line. */
export function presetMeta(p: StarterPreset): string {
  const parts: string[] = [];
  const a = Object.keys(p.alerts ?? {}).length;
  if (a) parts.push(`${a} setting${a === 1 ? "" : "s"}`);
  const t = (p.tiles ?? []).length;
  if (t) parts.push(`${t} tile${t === 1 ? "" : "s"}`);
  if (p.dashboard) parts.push(`${p.dashboard.widgets.length} widgets`);
  const s = (p.searches ?? []).length;
  if (s) parts.push(`${s} search${s === 1 ? "" : "es"}`);
  if ((p.searches ?? []).some((x) => x.alert)) parts.push("alerts on match");
  const r = (p.routes ?? []).length;
  if (r) parts.push(`${r} route${r === 1 ? "" : "s"}`);
  if (p.guest) parts.push(`${p.guest.kind === "vm" ? "VM" : "container"} · ${p.guest.cores} cores · ${p.guest.memoryMb >= 1024 ? `${p.guest.memoryMb / 1024} GB` : `${p.guest.memoryMb} MB`} · ${p.guest.diskGb} GB disk`);
  if (p.drills) parts.push(p.drills.everyDays === 7 ? "weekly" : `every ${p.drills.everyDays} days`);
  const n = Object.keys(p.security ?? {}).length + Object.keys(p.netWatch ?? {}).length;
  if (n) parts.push(`${n} security setting${n === 1 ? "" : "s"}`);
  return parts.join(" · ");
}

/** The confirmation sentence before a preset is added. */
export function presetConfirm(p: StarterPreset, host: string): string {
  switch (p.kind) {
    case "alerts":
      return "Change these alert settings? You can change them back in Settings → ALERTS.";
    case "tiles":
      return `Add ${p.tiles.length} tile${p.tiles.length === 1 ? "" : "s"} for ${host}? They only read, and you can remove them from the Overview.`;
    case "dashboard":
      return `Add the “${p.dashboard?.name}” dashboard${p.tiles.length ? ` with ${p.tiles.length} tiles for ${host}` : ""}? Your other dashboards stay as they are.`;
    case "searches":
      return `Save ${p.searches.length} log search${p.searches.length === 1 ? "" : "es"}? They cover every host and can be edited in Fleet → LOGS.`;
    case "routes":
      return `Save these routes to the proxy on ${host}? The proxy checks the new config first and keeps the old one if it won't load.`;
    case "guests":
      return "Open Proxmox → Create with these values?";
    case "drills":
      return "Add these drills? They run on their schedule on connected hosts and only restore into a temporary place.";
    case "posture":
      return "Change these settings? You can change them back in Settings.";
  }
}
