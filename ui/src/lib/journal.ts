/**
 * The homelab journal (0.10.81): wording, day grouping and diff lines for
 * Fleet → JOURNAL. Pure, so it's tested without the view.
 */
import type { ChangeArea, ChangeEntry } from "./api";

export const AREAS: { id: ChangeArea; label: string }[] = [
  { id: "stack", label: "Stacks" },
  { id: "container", label: "Containers" },
  { id: "file", label: "Files" },
  { id: "config", label: "Config" },
  { id: "firewall", label: "Firewall" },
  { id: "jobs", label: "Cron & timers" },
  { id: "proxy", label: "Proxy" },
  { id: "secrets", label: ".env vault" },
  { id: "system", label: "System" },
  { id: "power", label: "Power" },
  { id: "proxmox", label: "Proxmox" },
  { id: "automation", label: "Automation" },
  { id: "keys", label: "SSH keys" },
  { id: "integrations", label: "Integrations" },
  { id: "blueprint", label: "Blueprints" },
  { id: "note", label: "Notes" },
];

export const areaLabel = (a: ChangeArea): string => AREAS.find((x) => x.id === a)?.label ?? a;

/** Short HUD code shown in the entry's gutter. */
export const AREA_CODE: Record<ChangeArea, string> = {
  stack: "STK",
  container: "CTR",
  file: "FIL",
  config: "CFG",
  firewall: "FW",
  jobs: "JOB",
  proxy: "PRX",
  secrets: "ENV",
  system: "SYS",
  power: "PWR",
  proxmox: "PVE",
  automation: "AUT",
  keys: "KEY",
  integrations: "INT",
  blueprint: "BLU",
  note: "NOTE",
};

export type RangeId = "1d" | "7d" | "30d" | "90d" | "1y" | "all";
export const RANGES: { id: RangeId; label: string; days: number | null }[] = [
  { id: "1d", label: "24H", days: 1 },
  { id: "7d", label: "7D", days: 7 },
  { id: "30d", label: "30D", days: 30 },
  { id: "90d", label: "90D", days: 90 },
  { id: "1y", label: "1Y", days: 365 },
  { id: "all", label: "ALL", days: null },
];

/** Unix seconds the range starts at, or null for everything. */
export function rangeSince(id: RangeId, nowSecs: number): number | null {
  const d = RANGES.find((r) => r.id === id)?.days;
  return d == null ? null : nowSecs - d * 86_400;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** Local calendar day key, e.g. "2026-11-17". */
export function dayKey(at: number): string {
  const d = new Date(at * 1000);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** "Today", "Yesterday" or "Mon 3 Nov 2026". */
export function dayLabel(key: string, nowSecs: number): string {
  if (key === dayKey(nowSecs)) return "Today";
  if (key === dayKey(nowSecs - 86_400)) return "Yesterday";
  const [y, m, d] = key.split("-").map(Number);
  const date = new Date(y, m - 1, d);
  return date.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short", year: "numeric" });
}

export const timeOf = (at: number): string => {
  const d = new Date(at * 1000);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
};

/** Entries grouped by local day, newest day first (input is newest first). */
export function byDay(entries: ChangeEntry[]): { key: string; entries: ChangeEntry[] }[] {
  const out: { key: string; entries: ChangeEntry[] }[] = [];
  for (const e of entries) {
    const key = dayKey(e.at);
    const last = out[out.length - 1];
    if (last?.key === key) last.entries.push(e);
    else out.push({ key, entries: [e] });
  }
  return out;
}

export type DiffKind = "add" | "del" | "same" | "gap";

/** A stored diff, line by line. */
export function diffLines(diff: string): { kind: DiffKind; text: string }[] {
  return diff.split("\n").map((l) => {
    if (l.startsWith("@@")) return { kind: "gap" as const, text: l.slice(2).trim() };
    if (l.startsWith("+")) return { kind: "add" as const, text: l.slice(1) };
    if (l.startsWith("-")) return { kind: "del" as const, text: l.slice(1) };
    return { kind: "same" as const, text: l.slice(1) };
  });
}

export function diffCounts(diff: string | null): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const l of diff?.split("\n") ?? []) {
    if (l.startsWith("+")) added++;
    else if (l.startsWith("-")) removed++;
  }
  return { added, removed };
}

/**
 * Whether a DRIFT question asks what changed (so the journal goes in its
 * context even when no host is named).
 */
export function asksAboutChanges(q: string): boolean {
  return /\b(what|anything|who)\b.{0,30}\b(chang\w*|did i|deploy\w*|edit\w*|touch\w*|modif\w*)|\bbefore (it|this|that|\w+) (broke|stopped|failed|went down)|\b(journal|changelog|recent changes)\b/i.test(q);
}
