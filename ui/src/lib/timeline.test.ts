import { describe, expect, it } from "vitest";
import type { HostEvent } from "./api";
import { byDay, dayKey, dayLabel, filterEvents, silencedCount, tally, timelineText, type FilterId } from "./timeline";

const at = (y: number, m: number, d: number, h = 12, min = 0) => Math.floor(new Date(y, m - 1, d, h, min).getTime() / 1000);
const ev = (t: number, kind: HostEvent["kind"], level: HostEvent["level"], title: string, detail = "", origin: HostEvent["origin"] = "host"): HostEvent => ({
  at: t,
  kind,
  level,
  title,
  detail,
  origin,
  silenced: null,
});

const events: HostEvent[] = [
  ev(at(2026, 11, 15, 21, 5), "alert", "critical", "HOST OFFLINE: nas", "no answer", "nightgrid"),
  ev(at(2026, 11, 15, 9), "container", "warn", "web restarted", "after exit 1"),
  ev(at(2026, 11, 14, 8), "packages", "info", "apt: upgraded 5"),
  ev(at(2026, 11, 14, 7), "boot", "info", "Booted"),
];

describe("timeline", () => {
  it("filters by kind, level and text", () => {
    const all = new Set<FilterId>(["alerts", "boots", "containers", "system", "packages", "automation"]);
    expect(filterEvents(events, all)).toHaveLength(4);
    expect(filterEvents(events, new Set<FilterId>(["boots"])).map((e) => e.title)).toEqual(["Booted"]);
    expect(filterEvents(events, all, "", true)).toHaveLength(2);
    expect(filterEvents(events, all, "EXIT")).toHaveLength(1);
    expect(filterEvents(events, new Set())).toHaveLength(0);
  });

  it("groups by local day and labels days", () => {
    const g = byDay(events);
    expect(g.map((d) => d.day)).toEqual(["2026-11-15", "2026-11-14"]);
    expect(g[0].events).toHaveLength(2);
    const now = new Date(2026, 10, 15, 22);
    expect(dayLabel("2026-11-15", now)).toBe("TODAY");
    expect(dayLabel("2026-11-14", now)).toBe("YESTERDAY");
    expect(dayLabel("2026-11-10", now)).toMatch(/10 NOV/);
    expect(dayKey(at(2026, 1, 2))).toBe("2026-01-02");
  });

  it("counts levels and writes text for DRIFT, oldest first", () => {
    expect(tally(events)).toEqual({ info: 2, ok: 0, warn: 1, critical: 1 });
    const text = timelineText(events);
    const lines = text.split("\n");
    expect(lines[0]).toBe("2026-11-14 07:00 boot: Booted (host log)");
    expect(lines[3]).toBe("2026-11-15 21:05 alert: HOST OFFLINE: nas [critical] — no answer");
    expect(timelineText(events, 2).split("\n")[0]).toBe("(only the newest 2 of 4 entries)");
  });

  it("marks acknowledged and muted alerts and leaves them out of the problems", () => {
    const acked = { ...ev(at(2026, 11, 16, 9), "alert", "warn", "DISK FULL: nas", "/volume1 is 94% full", "nightgrid"), silenced: "ack" as const };
    const loud = ev(at(2026, 11, 16, 10), "alert", "critical", "HOST OFFLINE: nas", "no answer", "nightgrid");
    const all = [loud, acked];
    expect(tally(all)).toEqual({ info: 0, ok: 0, warn: 0, critical: 1 });
    expect(silencedCount(all)).toBe(1);
    const alerts = new Set<FilterId>(["alerts"]);
    expect(filterEvents(all, alerts, "", true)).toEqual([loud]);
    expect(filterEvents(all, alerts)).toEqual(all);
    expect(timelineText(all)).toContain("DISK FULL: nas [warn] — /volume1 is 94% full (acknowledged)");
  });
});
