/** Helpers for network tests between hosts (M10x). */
import type { NetTest_Serialize as NetTest } from "./bindings";

/** "941 Mbit/s", "2.35 Gbit/s", "--". */
export function formatBps(b: number | null | undefined): string {
  if (b == null) return "--";
  if (b >= 1e9) return `${(b / 1e9).toFixed(2)} Gbit/s`;
  if (b >= 1e6) return `${(b / 1e6).toFixed(b >= 1e8 ? 0 : 1)} Mbit/s`;
  return `${(b / 1e3).toFixed(0)} kbit/s`;
}

export const formatMs = (ms: number | null | undefined): string => (ms == null ? "--" : ms < 1 ? `${ms.toFixed(2)} ms` : `${ms.toFixed(1)} ms`);

export const pairKey = (t: { fromHost: number; toHost: number }): string => `${t.fromHost}>${t.toHost}`;

/** The best direction of a run, for bars and comparisons. */
export const best = (t: NetTest): number | null => {
  const v = [t.upBps, t.downBps].filter((x): x is number => x != null);
  return v.length ? Math.max(...v) : null;
};

export interface Pair {
  key: string;
  fromHost: number;
  toHost: number;
  latest: NetTest;
  /** Oldest first, for the sparkline. */
  runs: NetTest[];
}

/** Groups results (newest first) into pairs, most recently tested first. */
export function pairs(tests: NetTest[]): Pair[] {
  const by = new Map<string, Pair>();
  for (const t of tests) {
    const k = pairKey(t);
    const p = by.get(k);
    if (p) p.runs.unshift(t);
    else by.set(k, { key: k, fromHost: t.fromHost, toHost: t.toHost, latest: t, runs: [t] });
  }
  return [...by.values()];
}

/** A plain-language tone for a run against its pair's history. */
export function tone(t: NetTest, runs: NetTest[]): "ok" | "warn" | "bad" {
  if (t.error && best(t) == null) return "bad";
  const b = best(t);
  if (b != null && b >= 85e6 && b <= 100e6) return "warn";
  const past = runs.filter((r) => r.id !== t.id && !r.error).map(best).filter((x): x is number => x != null).sort((a, z) => a - z);
  if (b != null && past.length >= 3 && b < past[Math.floor(past.length / 2)] * 0.7) return "warn";
  if ((t.lossPct ?? 0) > 0) return "warn";
  return t.error ? "warn" : "ok";
}
