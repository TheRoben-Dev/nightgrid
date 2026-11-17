import { describe, expect, it } from "vitest";
import type { ChangeEntry } from "./api";
import { AREAS, AREA_CODE, asksAboutChanges, byDay, dayKey, dayLabel, diffCounts, diffLines, rangeSince } from "./journal";

const e = (id: number, at: number): ChangeEntry => ({
  id,
  at,
  hostId: 1,
  host: "server01",
  area: "stack",
  title: `entry ${id}`,
  detail: "",
  diff: null,
  ok: true,
  manual: false,
});

describe("journal", () => {
  it("groups by local day, newest first", () => {
    const base = new Date(2026, 10, 17, 12, 0).getTime() / 1000;
    const groups = byDay([e(3, base + 60), e(2, base), e(1, base - 86_400)]);
    expect(groups.map((g) => g.entries.length)).toEqual([2, 1]);
    expect(groups[0].key).toBe("2026-11-17");
    expect(dayLabel(groups[0].key, base)).toBe("Today");
    expect(dayLabel(groups[1].key, base)).toBe("Yesterday");
    expect(dayKey(base)).toBe("2026-11-17");
  });

  it("reads a stored diff", () => {
    const d = "@@ 3 unchanged\n a\n-b\n+c\n+d";
    expect(diffLines(d).map((l) => l.kind)).toEqual(["gap", "same", "del", "add", "add"]);
    expect(diffLines(d)[0].text).toBe("3 unchanged");
    expect(diffCounts(d)).toEqual({ added: 2, removed: 1 });
    expect(diffCounts(null)).toEqual({ added: 0, removed: 0 });
  });

  it("knows ranges and areas", () => {
    expect(rangeSince("7d", 1_000_000)).toBe(1_000_000 - 7 * 86_400);
    expect(rangeSince("all", 1)).toBeNull();
    for (const a of AREAS) expect(AREA_CODE[a.id]).toBeTruthy();
  });

  it("spots questions about changes", () => {
    expect(asksAboutChanges("what changed before plex broke?")).toBe(true);
    expect(asksAboutChanges("What did I deploy yesterday")).toBe(true);
    expect(asksAboutChanges("show me the journal")).toBe(true);
    expect(asksAboutChanges("how do I add a host")).toBe(false);
  });
});
