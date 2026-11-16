/**
 * Uptime history helpers (M10t): one bar per day, availability and outage
 * wording. Availability is up out of up + down: time Nightgrid wasn't
 * running, and time a host was asleep, doesn't count either way.
 */
import type { DepNode, UptimeDay, UptimeOutage, UptimeSeries } from "./api";

export const DAY = 86_400;

/** Minutes east of UTC, for local-midnight days. */
export const utcOffsetMin = (d = new Date()): number => -d.getTimezoneOffset();

export interface DayBar {
  day: number;
  /** Percent up, or null when nothing was seen that day. */
  pct: number | null;
  up: number;
  down: number;
  asleep: number;
}

export const nodeKey = (n: DepNode): string => `${n.kind}:${n.id}`;

export function availability(up: number, down: number): number | null {
  return up + down > 0 ? (up * 100) / (up + down) : null;
}

/** `days` bars ending today (local), oldest first, empty where nothing was recorded. */
export function dayBars(s: UptimeSeries | undefined, days: number, nowSecs = Date.now() / 1000, offsetMin = utcOffsetMin()): DayBar[] {
  const off = offsetMin * 60;
  const today = Math.floor((nowSecs + off) / DAY) * DAY - off;
  const by = new Map<number, UptimeDay>((s?.days ?? []).map((d) => [d.day, d]));
  const out: DayBar[] = [];
  for (let i = days - 1; i >= 0; i--) {
    const day = today - i * DAY;
    const d = by.get(day);
    out.push(d ? { day, pct: availability(d.up, d.down), up: d.up, down: d.down, asleep: d.asleep } : { day, pct: null, up: 0, down: 0, asleep: 0 });
  }
  return out;
}

export interface Totals {
  pct: number | null;
  up: number;
  down: number;
  asleep: number;
  /** Days with anything recorded. */
  seenDays: number;
}

export function totals(bars: DayBar[]): Totals {
  const t = bars.reduce((a, b) => ({ up: a.up + b.up, down: a.down + b.down, asleep: a.asleep + b.asleep }), { up: 0, down: 0, asleep: 0 });
  return { ...t, pct: availability(t.up, t.down), seenDays: bars.filter((b) => b.up + b.down + b.asleep > 0).length };
}

export type BarTone = "ok" | "warn" | "bad" | "none";

export function barTone(pct: number | null): BarTone {
  if (pct == null) return "none";
  if (pct >= 99.9) return "ok";
  if (pct >= 99) return "warn";
  return "bad";
}

/** "100%", "99.95%", "87.3%". */
export function pctText(p: number | null): string {
  if (p == null) return "no data";
  if (p >= 100) return "100%";
  return p >= 99 ? `${(Math.floor(p * 100) / 100).toFixed(2)}%` : `${p.toFixed(1)}%`;
}

/** "45 s", "12 min", "3 h 5 min", "2 d 4 h". */
export function durText(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.round(s / 60)} min`;
  if (s < DAY) {
    const m = Math.round((s % 3600) / 60);
    return m ? `${Math.floor(s / 3600)} h ${m} min` : `${Math.floor(s / 3600)} h`;
  }
  const h = Math.round((s % DAY) / 3600);
  return h ? `${Math.floor(s / DAY)} d ${h} h` : `${Math.floor(s / DAY)} d`;
}

const fmtDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const fmtTime = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });

/** Tooltip for one bar. */
export function barTitle(b: DayBar): string {
  const date = fmtDay.format(new Date(b.day * 1000));
  if (b.up + b.down + b.asleep === 0) return `${date}: no data (Nightgrid wasn't watching)`;
  const parts = [pctText(b.pct)];
  if (b.down) parts.push(`${durText(b.down)} down`);
  if (b.asleep) parts.push(`${durText(b.asleep)} asleep`);
  return `${date}: ${parts.join(", ")}`;
}

export function outageWhen(o: UptimeOutage): string {
  return fmtTime.format(new Date(o.start * 1000));
}

/** Seconds an outage lasted (the last round it was seen down, minus its start). */
export const outageLen = (o: UptimeOutage): number => Math.max(0, o.end - o.start);

/** Count, total and longest of a list of outages. */
export function outageStats(list: UptimeOutage[]): { count: number; total: number; longest: number } {
  return { count: list.length, total: list.reduce((a, o) => a + outageLen(o), 0), longest: list.reduce((a, o) => Math.max(a, outageLen(o)), 0) };
}
