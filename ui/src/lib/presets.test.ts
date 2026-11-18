import { describe, expect, it } from "vitest";
import type { AlertSettings, StarterPreset } from "./api";
import { alertChanges, alertWords, dashboardRoom, isPresetShelf, presetMeta, resolveDashboard, tileWidget, withAlerts } from "./presets";

const base = { diskPercent: 90, quietHours: false, quietStart: 1320, logAnomaly: false } as unknown as AlertSettings;

const preset = (p: Partial<StarterPreset>): StarterPreset => ({
  id: "x",
  kind: "alerts",
  title: "X",
  summary: "s",
  purpose: "p",
  category: "",
  library: [],
  alerts: {},
  tiles: [],
  dashboard: null,
  searches: [],
  ...p,
});

describe("presets", () => {
  it("words alert values", () => {
    expect(alertWords("quietStart", 1380)).toBe("23:00");
    expect(alertWords("diskPercent", 80)).toBe("80 %");
    expect(alertWords("logAnomaly", true)).toBe("on");
  });

  it("lists alert changes and applies them", () => {
    const p = preset({ alerts: { diskPercent: 80, quietHours: true, logAnomaly: false } });
    const c = alertChanges(p, base);
    expect(c.map((x) => [x.key, x.from, x.to, x.same])).toEqual([
      ["diskPercent", "90 %", "80 %", false],
      ["quietHours", "off", "on", false],
      ["logAnomaly", "off", "off", true],
    ]);
    const out = withAlerts(p, base) as unknown as Record<string, unknown>;
    expect(out.diskPercent).toBe(80);
    expect(out.quietStart).toBe(1320);
  });

  it("points dashboard tiles at saved widgets with a free name", () => {
    const d = { name: "NAS watch", widgets: [{ id: "a", kind: "disks", x: 0, y: 0, w: 4, h: 3 }, { id: "b", kind: "tile:1", x: 4, y: 0, w: 2, h: 2 }] };
    const r = resolveDashboard(d, [7, 9], ["NAS watch"]);
    expect(r.name).toBe("NAS watch 2");
    expect(r.widgets[1].kind).toBe("cmd:9");
    expect(() => resolveDashboard(d, [7], [])).toThrow();
  });

  it("builds widgets, card lines and limits", () => {
    const t = { name: "Load", command: "cut -d' ' -f1 /proc/loadavg", parse: "number", pattern: "", unit: "", warn: 4, crit: 8, below: false, intervalSecs: 60, what: "" };
    expect(tileWidget(t, 3)).toMatchObject({ id: 0, hostId: 3, name: "Load", warn: 4 });
    expect(presetMeta(preset({ kind: "tiles", tiles: [t, t] }))).toBe("2 tiles");
    expect(dashboardRoom(12)).toMatch(/12 dashboards/);
    expect(dashboardRoom(2)).toBeNull();
    expect(isPresetShelf("searches")).toBe(true);
    expect(isPresetShelf("app")).toBe(false);
  });
});
