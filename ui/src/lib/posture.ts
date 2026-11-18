/** Security posture (1.1, D-168): wording, ordering and tones for Security → POSTURE. */
import type { PostureCheck, PostureHost, PostureRun, PostureStatus } from "./api";

export function gradeTone(g: string | null | undefined): "ok" | "warn" | "bad" | "muted" {
  if (!g) return "muted";
  return g === "A" || g === "B" ? "ok" : g === "C" ? "warn" : "bad";
}

export const statusMark: Record<PostureStatus, string> = { pass: "✓", warn: "!", fail: "✗", unknown: "?" };

/** Failed first, then warnings, heaviest first; passed and unknown after. */
export function ordered(checks: readonly PostureCheck[]): PostureCheck[] {
  const rank: Record<PostureStatus, number> = { fail: 0, warn: 1, unknown: 2, pass: 3 };
  return [...checks].sort((a, b) => rank[a.status] - rank[b.status] || b.weight - a.weight);
}

/** Points a check costs: all of its weight when failed, half when warned. */
export const cost = (c: PostureCheck): number => (c.status === "fail" ? c.weight : c.status === "warn" ? Math.floor(c.weight / 2) : 0);

/** "+4" / "−12" / "" against the run before. */
export function delta(h: PostureHost): string {
  if (h.previous == null || h.latest.score == null) return "";
  const d = h.latest.score - h.previous;
  return d === 0 ? "" : d > 0 ? `+${d}` : `−${-d}`;
}

/** Hosts worst first; unread hosts last. */
export function worstFirst(list: readonly PostureHost[]): PostureHost[] {
  return [...list].sort((a, b) => (a.latest.score ?? 101) - (b.latest.score ?? 101));
}

export interface FleetSummary {
  hosts: number;
  average: number | null;
  below: number;
  failing: number;
}

export function summary(list: readonly PostureHost[]): FleetSummary {
  const scored = list.filter((h) => h.latest.score != null);
  const avg = scored.length ? Math.round(scored.reduce((a, h) => a + (h.latest.score ?? 0), 0) / scored.length) : null;
  return {
    hosts: list.length,
    average: avg,
    below: scored.filter((h) => (h.latest.score ?? 0) < 75).length,
    failing: list.reduce((n, h) => n + h.latest.checks.filter((c) => c.status === "fail").length, 0),
  };
}

/** Sparkline points (0–100 → y) for a trend. */
export function sparkline(trend: readonly number[], w = 80, h = 18): string {
  if (trend.length < 2) return "";
  const step = w / (trend.length - 1);
  return trend.map((v, i) => `${(i * step).toFixed(1)},${(h - (v / 100) * h).toFixed(1)}`).join(" ");
}

export const fixesFor = (r: PostureRun): PostureCheck[] => ordered(r.checks).filter((c) => c.status === "fail" || c.status === "warn");
