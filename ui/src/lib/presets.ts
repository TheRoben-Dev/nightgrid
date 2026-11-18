/** Starter presets: alert rules, command tiles, dashboards and log searches ready to add. */
import type { AlertSettings, AlertValue, CmdWidget, PresetKind, PresetTile, StarterPreset } from "./api";
import { MAX_DASHBOARDS, uniqueName, type Dashboard } from "./dashboard";

/** Gallery shelves for presets, after the blueprint shelves. */
export const PRESET_SHELVES: { id: PresetKind; label: string; what: string }[] = [
  { id: "alerts", label: "ALERTS", what: "Alert rules: which alerts fire and their thresholds. Where they're sent stays your choice in Settings → ALERTS." },
  { id: "tiles", label: "TILES", what: "Command tiles for the Overview. They only read, refresh on their own, and run on the host you pick." },
  { id: "dashboard", label: "DASHBOARDS", what: "Overview dashboards for a kind of box, with their own tiles. Added beside your dashboards, never over them." },
  { id: "searches", label: "LOG SEARCHES", what: "Saved searches for Fleet → LOGS across every host. Some alert when new lines match." },
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

/** What a preset would change in `current`, in settings order. */
export function alertChanges(p: StarterPreset, current: AlertSettings): AlertChange[] {
  const cur = current as unknown as Record<string, AlertValue>;
  return Object.entries(p.alerts ?? {}).map(([key, to]) => ({
    key,
    label: ALERT_LABEL[key] ?? key,
    from: alertWords(key, cur[key]),
    to: alertWords(key, to),
    same: cur[key] === to,
  }));
}

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

/** Why a dashboard can't be added, or null. */
export const dashboardRoom = (count: number): string | null => (count >= MAX_DASHBOARDS ? `You already have ${MAX_DASHBOARDS} dashboards, the most there can be. Remove one on the Overview first.` : null);

/** Whether the preset needs a host (it brings tiles). */
export const needsHost = (p: StarterPreset): boolean => (p.tiles ?? []).length > 0;

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
  }
}
