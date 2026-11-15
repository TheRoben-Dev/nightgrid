/**
 * Linear trend over stored history, used to say when a disk fills up or
 * memory runs out. Least squares over (time, value); only answers when there
 * is enough history and the fit is reasonable.
 */

export interface Trend {
  /** Change per day, in the metric's units (percentage points). */
  perDay: number;
  /** The fitted value now. */
  now: number;
  /** Days until `target` at this rate, or null when not rising (or past a year). */
  daysLeft: number | null;
  /** Fit quality, 0..1. */
  r2: number;
  /** Days of history used. */
  spanDays: number;
}

/** Below this many points per day the series is too thin to trust. */
const MIN_POINTS = 24;
const MIN_SPAN_DAYS = 2;
/** Slopes under this (points per day) count as flat. */
const FLAT = 0.05;

export function trend(points: { ts: number; value: number | null | undefined }[], target: number, at = Date.now() / 1000): Trend | null {
  const p = points.filter((x): x is { ts: number; value: number } => x.value != null && Number.isFinite(x.value));
  if (p.length < MIN_POINTS) return null;
  const t0 = p[0].ts;
  const spanDays = (p[p.length - 1].ts - t0) / 86400;
  if (spanDays < MIN_SPAN_DAYS) return null;
  // x in days for numerical comfort.
  const xs = p.map((q) => (q.ts - t0) / 86400);
  const ys = p.map((q) => q.value);
  const n = p.length;
  const mx = xs.reduce((a, b) => a + b, 0) / n;
  const my = ys.reduce((a, b) => a + b, 0) / n;
  let sxy = 0;
  let sxx = 0;
  let syy = 0;
  for (let i = 0; i < n; i++) {
    sxy += (xs[i] - mx) * (ys[i] - my);
    sxx += (xs[i] - mx) ** 2;
    syy += (ys[i] - my) ** 2;
  }
  if (sxx === 0) return null;
  const perDay = sxy / sxx;
  const r2 = syy === 0 ? 1 : (sxy * sxy) / (sxx * syy);
  const nowX = (at - t0) / 86400;
  const now = my + perDay * (nowX - mx);
  let daysLeft: number | null = null;
  if (perDay > FLAT && now < target) {
    const d = (target - now) / perDay;
    daysLeft = d <= 365 ? d : null;
  } else if (perDay > FLAT && now >= target) {
    daysLeft = 0;
  }
  return { perDay, now, daysLeft, r2, spanDays };
}

/** "in about 3 weeks" style wording for a forecast. */
export function inWords(days: number): string {
  if (days < 1) return "within a day";
  if (days < 2) return "in about a day";
  if (days < 14) return `in about ${Math.round(days)} days`;
  if (days < 60) return `in about ${Math.round(days / 7)} weeks`;
  return `in about ${Math.round(days / 30)} months`;
}

/**
 * One line for the monitor, or null when there's nothing worth saying.
 * Noisy series (r² < 0.5) aren't forecast: memory especially swings with
 * caches and restarts.
 */
export function describeTrend(t: Trend | null, what: string, target: number): { text: string; tone: "ok" | "warn" | "crit" } | null {
  if (!t) return null;
  const rate = `${t.perDay >= 0 ? "+" : ""}${t.perDay.toFixed(t.perDay < 1 && t.perDay > -1 ? 2 : 1)} %/day over ${Math.round(t.spanDays)} days`;
  if (t.perDay < -FLAT) return { text: `${what}: falling (${rate})`, tone: "ok" };
  if (t.perDay <= FLAT) return { text: `${what}: steady (${rate})`, tone: "ok" };
  if (t.r2 < 0.5) return { text: `${what}: rising slowly but unevenly (${rate})`, tone: "ok" };
  if (t.daysLeft === null) return { text: `${what}: rising slowly (${rate}), not ${target}% within a year`, tone: "ok" };
  const tone = t.daysLeft < 14 ? "crit" : t.daysLeft < 60 ? "warn" : "ok";
  return { text: `${what}: reaches ${target}% ${inWords(t.daysLeft)} (${rate})`, tone };
}
