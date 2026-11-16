import { describe, expect, it } from "vitest";
import { availability, lastMonths, barTitle, barTone, dayBars, DAY, durText, outageStats, pctText, totals } from "./uptime";

describe("uptime", () => {
  const now = 20 * DAY + 3600; // 01:00 UTC on day 20
  const series = { node: { kind: "host" as const, id: 1 }, days: [
    { day: 18 * DAY, up: 86_000, down: 400, asleep: 0 },
    { day: 20 * DAY, up: 3600, down: 0, asleep: 0 },
  ] };

  it("lays out one bar per local day, oldest first", () => {
    const bars = dayBars(series, 4, now, 0);
    expect(bars.map((b) => b.day)).toEqual([17 * DAY, 18 * DAY, 19 * DAY, 20 * DAY]);
    expect(bars.map((b) => barTone(b.pct))).toEqual(["none", "warn", "none", "ok"]);
    // With +2 h the current local day starts at 22:00 UTC the day before.
    expect(dayBars(series, 1, now, 120)[0].day).toBe(20 * DAY - 7200);
    expect(dayBars(undefined, 3, now, 0).every((b) => b.pct === null)).toBe(true);
  });

  it("totals availability over seen time only", () => {
    const t = totals(dayBars(series, 4, now, 0));
    expect(t.seenDays).toBe(2);
    expect(t.down).toBe(400);
    expect(t.pct).toBeCloseTo((89_600 * 100) / 90_000, 6);
    expect(availability(0, 0)).toBeNull();
  });

  it("words percentages and durations", () => {
    expect(pctText(100)).toBe("100%");
    expect(pctText(99.999)).toBe("99.99%");
    expect(pctText(87.34)).toBe("87.3%");
    expect(pctText(null)).toBe("no data");
    expect(durText(42)).toBe("42 s");
    expect(durText(720)).toBe("12 min");
    expect(durText(3 * 3600 + 300)).toBe("3 h 5 min");
    expect(durText(2 * DAY + 4 * 3600)).toBe("2 d 4 h");
  });

  it("explains a bar and sums outages", () => {
    expect(barTitle({ day: 0, pct: null, up: 0, down: 0, asleep: 0 })).toContain("no data");
    expect(barTitle({ day: 0, pct: 99.5, up: 100, down: 600, asleep: 3600 })).toMatch(/99\.50%, 10 min down, 1 h asleep/);
    const o = (start: number, end: number) => ({ id: 1, node: { kind: "host" as const, id: 1 }, start, end, detail: "" });
    expect(outageStats([o(0, 60), o(100, 400)])).toEqual({ count: 2, total: 360, longest: 300 });
  });

  it("lists the last months, newest first, across a year boundary", () => {
    const m = lastMonths(3, new Date(2027, 0, 15));
    expect(m.map((x) => [x.year, x.month])).toEqual([[2027, 1], [2026, 12], [2026, 11]]);
  });
});
