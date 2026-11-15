import { describe, expect, it } from "vitest";
import { describeTrend, inWords, trend } from "./forecast";

const DAY = 86400;
const now = 1_800_000_000;
/** Hourly points over `days`, ending now. */
const series = (days: number, f: (d: number) => number | null) =>
  Array.from({ length: days * 24 + 1 }, (_, i) => {
    const ts = now - days * DAY + i * 3600;
    return { ts, value: f((ts - (now - days * DAY)) / DAY) };
  });

describe("forecast", () => {
  it("projects a steadily filling disk", () => {
    const t = trend(series(14, (d) => 70 + d * 0.5), 100, now)!;
    expect(t.perDay).toBeCloseTo(0.5, 5);
    expect(t.now).toBeCloseTo(77, 5);
    expect(t.daysLeft).toBeCloseTo(46, 5);
    expect(t.r2).toBeCloseTo(1, 5);
    expect(describeTrend(t, "Fullest disk", 100)).toEqual({ text: "Fullest disk: reaches 100% in about 7 weeks (+0.50 %/day over 14 days)", tone: "warn" });
  });
  it("says steady when flat and falling when shrinking", () => {
    const t = trend(series(7, (d) => 60 - d * 0.2), 100, now)!;
    expect(t.daysLeft).toBeNull();
    expect(describeTrend(t, "Memory", 95)?.text).toMatch(/^Memory: falling/);
    expect(describeTrend(trend(series(7, () => 60), 100, now), "Memory", 95)?.text).toMatch(/^Memory: steady/);
  });
  it("needs enough history", () => {
    expect(trend(series(1, () => 50), 100, now)).toBeNull();
    expect(trend(series(14, (d) => (d < 13.5 ? null : 50)), 100, now)).toBeNull();
  });
  it("won't forecast a noisy series", () => {
    const t = trend(series(10, (d) => 50 + d * 0.3 + (Math.floor(d * 24) % 2 ? 15 : -15)), 95, now)!;
    expect(t.r2).toBeLessThan(0.5);
    expect(describeTrend(t, "Memory", 95)?.text).toMatch(/unevenly/);
  });
  it("flags a disk that is about to fill", () => {
    const t = trend(series(3, (d) => 90 + d * 1.5), 100, now)!;
    expect(describeTrend(t, "Fullest disk", 100)?.tone).toBe("crit");
    expect(inWords(0.5)).toBe("within a day");
    expect(inWords(100)).toBe("in about 3 months");
  });
});
