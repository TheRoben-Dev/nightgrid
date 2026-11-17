/** Power & cost (M12d): presets, formatting and totals for Fleet → POWER. */
import type { HostPower, IdleWindow, UpsPower } from "./api";

/** Typical idle and full-load watts by kind of machine: a starting point. */
export const PRESETS: { id: string; label: string; idle: number; max: number }[] = [
  { id: "pi", label: "Raspberry Pi / SBC", idle: 3, max: 8 },
  { id: "mini", label: "Mini PC (NUC, N100)", idle: 7, max: 30 },
  { id: "laptop", label: "Laptop", idle: 8, max: 45 },
  { id: "nas2", label: "NAS, 2 bays", idle: 12, max: 25 },
  { id: "nas4", label: "NAS, 4+ bays", idle: 25, max: 50 },
  { id: "desktop", label: "Desktop tower", idle: 40, max: 180 },
  { id: "gaming", label: "Gaming / GPU tower", idle: 70, max: 450 },
  { id: "server", label: "Rack server", idle: 90, max: 300 },
];

export const RANGES = [7, 30, 90] as const;

export function fmtW(w: number | null | undefined): string {
  if (w == null) return "--";
  return w >= 100 ? `${Math.round(w)} W` : `${w.toFixed(1)} W`;
}

export function fmtKwh(k: number | null | undefined): string {
  if (k == null) return "--";
  return k >= 100 ? `${Math.round(k)} kWh` : `${k.toFixed(1)} kWh`;
}

/** "€4.20", "42 kr", or null when no price is set. */
export function money(kwh: number | null | undefined, price: number, currency: string): string | null {
  if (kwh == null || !(price > 0)) return null;
  const v = kwh * price;
  const n = v >= 100 ? Math.round(v).toString() : v.toFixed(2);
  const c = currency.trim();
  if (!c) return n;
  // Symbols go in front, words and codes after.
  return /^[$€£¥₹₩₽]$/.test(c) ? `${c}${n}` : `${n} ${c}`;
}

const hh = (h: number) => `${String(h % 24).padStart(2, "0")}:00`;

/** "23:00–07:00", or "all day". */
export const windowText = (w: IdleWindow): string => (w.hours >= 24 ? "all day" : `${hh(w.from)}–${hh(w.to)}`);

/** Minutes east of UTC, as the backend wants them. */
export const utcOffsetMins = (d = new Date()): number => -d.getTimezoneOffset();

export interface Totals {
  watts: number;
  kwhMonth: number;
  /** Hosts with an estimate / all hosts. */
  covered: number;
  hosts: number;
  /** All idle windows together. */
  savableKwh: number;
}

export function totals(hs: HostPower[]): Totals {
  const est = hs.filter((h) => h.avgW != null);
  return {
    watts: est.reduce((a, h) => a + (h.avgW ?? 0), 0),
    kwhMonth: est.reduce((a, h) => a + (h.kwhMonth ?? 0), 0),
    covered: est.length,
    hosts: hs.length,
    savableKwh: hs.reduce((a, h) => a + h.idle.reduce((b, w) => b + w.kwhMonth, 0), 0),
  };
}

/** How the estimates compare with what a UPS supplies, as a sentence. */
export function upsCompare(u: UpsPower): string | null {
  if (u.watts == null || u.estimatedW == null || u.watts <= 0) return null;
  const pct = Math.round((u.estimatedW / u.watts) * 100);
  if (pct >= 85 && pct <= 115) return `Estimates are within ${Math.abs(100 - pct)}% of what it ${u.measured ? "measures" : "reports"}.`;
  if (pct < 85) return `Estimates cover ${pct}%: something on it has no profile, or the profiles are low.`;
  return `Estimates are ${pct - 100}% above it: the profiles may be high.`;
}

/** The preset whose numbers a profile matches, if any. */
export const presetOf = (idle: number, max: number): string => PRESETS.find((p) => p.idle === idle && p.max === max)?.id ?? "";

export function profileProblem(idle: number, max: number): string | null {
  if (!Number.isFinite(idle) || !Number.isFinite(max) || idle < 0 || max <= 0) return "Watts are positive numbers.";
  if (max < idle) return "Full load can't be less than idle.";
  if (max > 5000) return "More than 5 kW: is that one host?";
  return null;
}
