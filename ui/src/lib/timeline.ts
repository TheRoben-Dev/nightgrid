/** Per-host incident timeline (M10l): grouping, filters and DRIFT text. */
import type { EventKind, HostEvent, Level } from "./api";

export const KIND_LABEL: Record<EventKind, string> = {
  alert: "ALERT",
  recovered: "CLEARED",
  boot: "BOOT",
  power: "POWER",
  container: "CONTAINER",
  unit: "UNIT",
  oom: "OOM",
  packages: "PACKAGES",
  stack: "STACK",
  automation: "AUTOMATION",
};

/** Filter chips: each covers one or more kinds. */
export const FILTERS = [
  { id: "alerts", label: "Alerts", kinds: ["alert", "recovered"] },
  { id: "boots", label: "Boots & power", kinds: ["boot", "power"] },
  { id: "containers", label: "Containers & stacks", kinds: ["container", "stack"] },
  { id: "system", label: "Units & OOM", kinds: ["unit", "oom"] },
  { id: "packages", label: "Packages", kinds: ["packages"] },
  { id: "automation", label: "Automation", kinds: ["automation"] },
] as const satisfies readonly { id: string; label: string; kinds: readonly EventKind[] }[];

export type FilterId = (typeof FILTERS)[number]["id"];

export const RANGES = [
  { days: 1, label: "24 H" },
  { days: 7, label: "7 D" },
  { days: 30, label: "30 D" },
  { days: 90, label: "90 D" },
] as const;

/** Events whose kind is in one of the `on` filters, matching `query`. */
export function filterEvents(events: HostEvent[], on: ReadonlySet<FilterId>, query = "", problemsOnly = false): HostEvent[] {
  const kinds = new Set<EventKind>(FILTERS.filter((f) => on.has(f.id)).flatMap((f) => [...f.kinds]));
  const q = query.trim().toLowerCase();
  return events.filter(
    (e) =>
      kinds.has(e.kind) &&
      (!problemsOnly || e.level === "warn" || e.level === "critical") &&
      (!q || `${e.title} ${e.detail}`.toLowerCase().includes(q)),
  );
}

/** Local `YYYY-MM-DD` for a unix time. */
export function dayKey(at: number): string {
  const d = new Date(at * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Local `HH:MM`. */
export function clock(at: number): string {
  const d = new Date(at * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** Events (newest first) grouped by local day, newest day first. */
export function byDay(events: HostEvent[]): { day: string; events: HostEvent[] }[] {
  const out: { day: string; events: HostEvent[] }[] = [];
  for (const e of events) {
    const k = dayKey(e.at);
    const last = out[out.length - 1];
    if (last && last.day === k) last.events.push(e);
    else out.push({ day: k, events: [e] });
  }
  return out;
}

/** "TODAY", "YESTERDAY" or "MON 10 NOV". */
export function dayLabel(day: string, now = new Date()): string {
  const today = dayKey(now.getTime() / 1000);
  const y = new Date(now);
  y.setDate(y.getDate() - 1);
  if (day === today) return "TODAY";
  if (day === dayKey(y.getTime() / 1000)) return "YESTERDAY";
  const [yy, mm, dd] = day.split("-").map(Number);
  const d = new Date(yy, mm - 1, dd);
  return d.toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" }).replace(",", "").toUpperCase();
}

/** Problems (warn/critical) and counts by level, for the header. */
export function tally(events: HostEvent[]): Record<Level, number> {
  const t: Record<Level, number> = { info: 0, ok: 0, warn: 0, critical: 0 };
  for (const e of events) t[e.level]++;
  return t;
}

/** Plain text for DRIFT, oldest first, capped to the newest `max` lines. */
export function timelineText(events: HostEvent[], max = 150): string {
  const picked = events.slice(0, max).reverse();
  const lines = picked.map((e) => {
    const lvl = e.level === "critical" ? " [critical]" : e.level === "warn" ? " [warn]" : e.level === "ok" ? " [ok]" : "";
    const who = e.origin === "nightgrid" ? "" : " (host log)";
    return `${dayKey(e.at)} ${clock(e.at)} ${KIND_LABEL[e.kind].toLowerCase()}: ${e.title}${lvl}${e.detail ? ` — ${e.detail}` : ""}${who}`;
  });
  if (events.length > max) lines.unshift(`(only the newest ${max} of ${events.length} entries)`);
  return lines.join("\n");
}
