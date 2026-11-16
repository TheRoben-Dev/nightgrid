/**
 * Overview dashboards (M10o): widgets on a 12-column grid. Placement rules
 * live here (pure functions, tested); the Rust side only validates.
 */
import type { Dashboard, Widget } from "./bindings";

export type { Dashboard, Widget };

export const COLUMNS = 12;
export const MAX_ROWS = 16;
export const MAX_WIDGETS = 48;
export const MAX_DASHBOARDS = 12;
export const MAX_PRESETS = 24;

export type WidgetKind = "nodes" | "alerts" | "services" | "updates" | "disks" | "backups" | "tunnels" | "notes" | "desktops" | "status";

export interface WidgetInfo {
  kind: WidgetKind;
  title: string;
  what: string;
  w: number;
  h: number;
  minW: number;
  minH: number;
}

export const WIDGETS: WidgetInfo[] = [
  { kind: "nodes", title: "NODES", what: "Every host with live CPU, memory and disk", w: 12, h: 4, minW: 3, minH: 2 },
  { kind: "alerts", title: "ALERTS", what: "Active alerts, worst first", w: 6, h: 3, minW: 3, minH: 2 },
  { kind: "services", title: "SERVICES", what: "Service checks: how many are up, and which are down", w: 6, h: 3, minW: 3, minH: 2 },
  { kind: "updates", title: "UPDATES", what: "Pending updates and reboots across hosts", w: 4, h: 3, minW: 3, minH: 2 },
  { kind: "disks", title: "DISKS", what: "Degraded pools and mounts filling up", w: 4, h: 3, minW: 3, minH: 2 },
  { kind: "backups", title: "BACKUPS", what: "Backup jobs that failed or went stale", w: 4, h: 3, minW: 3, minH: 2 },
  { kind: "tunnels", title: "TUNNELS", what: "SSH tunnels and their state", w: 4, h: 3, minW: 3, minH: 2 },
  { kind: "notes", title: "NOTES", what: "Pinned notes", w: 4, h: 3, minW: 3, minH: 2 },
  { kind: "desktops", title: "DESKTOPS", what: "Open VM screens and RDP tunnels", w: 4, h: 2, minW: 3, minH: 2 },
  { kind: "status", title: "SYS.STATUS", what: "Version, platform, database and keychain", w: 4, h: 3, minW: 3, minH: 2 },
];

export const widgetInfo = (kind: string): WidgetInfo | undefined => WIDGETS.find((w) => w.kind === kind);

let seq = 0;
/** A widget id unique within `taken`. */
export function newId(taken: Iterable<string>): string {
  const used = new Set(taken);
  for (;;) {
    const id = `w${Date.now().toString(36).slice(-4)}${(seq++).toString(36)}`;
    if (!used.has(id)) return id;
  }
}

const overlaps = (a: Widget, b: Widget): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

export function clampWidget(w: Widget): Widget {
  const info = widgetInfo(w.kind);
  const minW = info?.minW ?? 1;
  const minH = info?.minH ?? 1;
  const ww = Math.max(minW, Math.min(COLUMNS, Math.round(w.w)));
  const h = Math.max(minH, Math.min(MAX_ROWS, Math.round(w.h)));
  const x = Math.max(0, Math.min(COLUMNS - ww, Math.round(w.x)));
  const y = Math.max(0, Math.round(w.y));
  return { ...w, x, y, w: ww, h };
}

/** Moves every widget up as far as it goes, top to bottom (no gaps). */
export function compact(widgets: Widget[]): Widget[] {
  const sorted = [...widgets].sort((a, b) => a.y - b.y || a.x - b.x);
  const out: Widget[] = [];
  for (const w of sorted) {
    let y = w.y;
    while (y > 0 && !out.some((o) => overlaps(o, { ...w, y: y - 1 }))) y--;
    out.push({ ...w, y });
  }
  return out;
}

/**
 * Puts `moved` at its (clamped) position and pushes whatever it covers
 * down, then compacts. The moved widget keeps its spot.
 */
export function place(widgets: Widget[], moved: Widget): Widget[] {
  const m = clampWidget(moved);
  const rest = widgets.filter((w) => w.id !== m.id).sort((a, b) => a.y - b.y || a.x - b.x);
  const fixed: Widget[] = [m];
  for (const w of rest) {
    let cur = { ...w };
    while (fixed.some((f) => overlaps(f, cur))) {
      const hit = fixed.filter((f) => overlaps(f, cur));
      cur = { ...cur, y: Math.max(...hit.map((f) => f.y + f.h)) };
    }
    fixed.push(cur);
  }
  // Compact everything but let the moved widget rise only if nothing is above it.
  return compact(fixed);
}

/** The first free spot (top-left first) for a new widget of `kind`. */
export function add(widgets: Widget[], kind: WidgetKind): Widget[] {
  const info = widgetInfo(kind);
  const w = info?.w ?? 4;
  const h = info?.h ?? 3;
  const id = newId(widgets.map((x) => x.id));
  const bottom = widgets.reduce((n, x) => Math.max(n, x.y + x.h), 0);
  for (let y = 0; y <= bottom; y++) {
    for (let x = 0; x + w <= COLUMNS; x++) {
      const c: Widget = { id, kind, x, y, w, h };
      if (!widgets.some((o) => overlaps(o, c))) return [...widgets, c];
    }
  }
  return [...widgets, { id, kind, x: 0, y: bottom, w, h }];
}

export const remove = (widgets: Widget[], id: string): Widget[] => compact(widgets.filter((w) => w.id !== id));

/** Converts a pointer movement in pixels into grid steps. */
export function gridDelta(dx: number, dy: number, colWidth: number, rowHeight: number, gap: number): { dx: number; dy: number } {
  return { dx: Math.round(dx / (colWidth + gap)), dy: Math.round(dy / (rowHeight + gap)) };
}

function d(name: string, items: [WidgetKind, number, number, number, number][]): Dashboard {
  return { name, widgets: items.map(([kind, x, y, w, h], i) => ({ id: `${kind}-${i}`, kind, x, y, w, h })) };
}

/** Built-in starting points. */
export const BUILTIN_PRESETS: Dashboard[] = [
  d("Overview", [
    ["nodes", 0, 0, 12, 4],
    ["alerts", 0, 4, 6, 3],
    ["services", 6, 4, 6, 3],
    ["updates", 0, 7, 4, 3],
    ["disks", 4, 7, 4, 3],
    ["backups", 8, 7, 4, 3],
  ]),
  d("Operations", [
    ["alerts", 0, 0, 8, 4],
    ["disks", 8, 0, 4, 4],
    ["updates", 0, 4, 4, 3],
    ["backups", 4, 4, 4, 3],
    ["tunnels", 8, 4, 4, 3],
    ["nodes", 0, 7, 12, 4],
  ]),
  d("Minimal", [
    ["nodes", 0, 0, 8, 4],
    ["alerts", 8, 0, 4, 4],
  ]),
  d("Notes & links", [
    ["notes", 0, 0, 6, 5],
    ["services", 6, 0, 6, 5],
    ["desktops", 0, 5, 6, 2],
    ["status", 6, 5, 6, 3],
  ]),
];

export const defaultDashboards = (): Dashboard[] => [structuredClone(BUILTIN_PRESETS[0])];

/** A copy of `p` with fresh ids, named `name`. */
export function fromPreset(p: Dashboard, name = p.name): Dashboard {
  const ids: string[] = [];
  return {
    name,
    widgets: p.widgets.map((w) => {
      const id = newId(ids);
      ids.push(id);
      return { ...w, id };
    }),
  };
}

/** A name not already in `names` ("Dashboard 2", "Dashboard 3", …). */
export function uniqueName(base: string, names: string[]): string {
  if (!names.includes(base)) return base;
  for (let i = 2; ; i++) if (!names.includes(`${base} ${i}`)) return `${base} ${i}`;
}

/** Widgets in reading order, for a single column on narrow windows. */
export const readingOrder = (widgets: Widget[]): Widget[] => [...widgets].sort((a, b) => a.y - b.y || a.x - b.x);
