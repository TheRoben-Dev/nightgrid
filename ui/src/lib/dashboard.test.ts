import { describe, expect, it } from "vitest";
import { add, BUILTIN_PRESETS, clampWidget, compact, COLUMNS, fromPreset, gridDelta, place, readingOrder, remove, uniqueName, type Widget } from "./dashboard";

const w = (id: string, x: number, y: number, ww: number, h: number, kind = "alerts"): Widget => ({ id, kind, x, y, w: ww, h });
const overlap = (a: Widget, b: Widget) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
const tidy = (ws: Widget[]) => ws.every((a, i) => a.x >= 0 && a.x + a.w <= COLUMNS && ws.slice(0, i).every((b) => !overlap(a, b)));
const byId = (ws: Widget[], id: string) => ws.find((x) => x.id === id)!;

describe("dashboard grid", () => {
  it("built-in presets are valid layouts", () => {
    for (const p of BUILTIN_PRESETS) {
      expect(tidy(p.widgets), p.name).toBe(true);
      expect(new Set(p.widgets.map((x) => x.id)).size).toBe(p.widgets.length);
    }
  });

  it("clamps size and position to the grid and the widget's minimum", () => {
    expect(clampWidget(w("a", 11, -2, 4, 99))).toEqual(w("a", 8, 0, 4, 16));
    expect(clampWidget(w("a", 0, 0, 1, 1))).toEqual(w("a", 0, 0, 3, 2));
  });

  it("compacts upwards without overlapping", () => {
    const out = compact([w("a", 0, 5, 6, 2), w("b", 6, 9, 6, 2), w("c", 0, 10, 12, 1)]);
    expect(byId(out, "a").y).toBe(0);
    expect(byId(out, "b").y).toBe(0);
    expect(byId(out, "c").y).toBe(2);
    expect(tidy(out)).toBe(true);
  });

  it("moving onto a widget pushes it down", () => {
    const start = [w("a", 0, 0, 6, 2), w("b", 6, 0, 6, 2), w("c", 0, 2, 12, 2)];
    const out = place(start, { ...byId(start, "c"), y: 0, w: 6 });
    expect(tidy(out)).toBe(true);
    expect(byId(out, "c")).toMatchObject({ x: 0, y: 0, w: 6 });
    expect(byId(out, "a").y).toBe(2);
    expect(byId(out, "b").y).toBe(0);
  });

  it("resizing wider pushes neighbours below", () => {
    const start = [w("a", 0, 0, 6, 2), w("b", 6, 0, 6, 2)];
    const out = place(start, { ...byId(start, "a"), w: 12 });
    expect(byId(out, "a")).toMatchObject({ x: 0, y: 0, w: 12 });
    expect(byId(out, "b").y).toBe(2);
    expect(tidy(out)).toBe(true);
  });

  it("adds into the first free spot and removes with compaction", () => {
    const start = [w("a", 0, 0, 6, 3)];
    const added = add(start, "updates");
    const n = added[1];
    expect(n).toMatchObject({ kind: "updates", x: 6, y: 0, w: 4, h: 3 });
    const full = add([w("a", 0, 0, 12, 3)], "disks");
    expect(full[1]).toMatchObject({ x: 0, y: 3 });
    const removed = remove([w("a", 0, 0, 12, 3), w("b", 0, 3, 12, 2)], "a");
    expect(removed).toEqual([w("b", 0, 0, 12, 2)]);
  });

  it("presets copy with fresh ids, names stay unique, reading order", () => {
    const c = fromPreset(BUILTIN_PRESETS[0], "Mine");
    expect(c.name).toBe("Mine");
    expect(c.widgets.map((x) => x.id)).not.toEqual(BUILTIN_PRESETS[0].widgets.map((x) => x.id));
    expect(new Set(c.widgets.map((x) => x.id)).size).toBe(c.widgets.length);
    expect(uniqueName("Dashboard", ["Dashboard", "Dashboard 2"])).toBe("Dashboard 3");
    expect(uniqueName("Media", ["Home"])).toBe("Media");
    expect(readingOrder([w("b", 6, 0, 6, 2), w("c", 0, 2, 6, 2), w("a", 0, 0, 6, 2)]).map((x) => x.id)).toEqual(["a", "b", "c"]);
    expect(gridDelta(130, -95, 100, 80, 10)).toEqual({ dx: 1, dy: -1 });
  });
});
