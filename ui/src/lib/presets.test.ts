import { describe, expect, it } from "vitest";
import type { AlertSettings, StarterPreset } from "./api";
import { alertChanges, alertWords, dashboardRoom, drillsFor, isPresetShelf, presetMeta, resolveDashboard, routeDomain, tileWidget, withAlerts, withTraefikRoutes } from "./presets";

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
  routes: [],
  guest: null,
  drills: null,
  security: {},
  netWatch: {},
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
      ["alerts.diskPercent", "90 %", "80 %", false],
      ["alerts.quietHours", "off", "on", false],
      ["alerts.logAnomaly", "off", "off", true],
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

  it("builds proxy routes under the user's domain and skips ones already there", () => {
    const r = { name: "jellyfin", sub: "jellyfin", upstream: "jellyfin:8096", what: "" };
    expect(routeDomain(r, " Home.Example.com. ")).toBe("jellyfin.home.example.com");
    expect(routeDomain(r, "not a domain")).toBeNull();
    expect(routeDomain(r, "localhost")).toBeNull();
    const existing = [{ name: "jellyfin", domains: ["x.home.example.com"], upstream: "a:1", entrypoints: [], tls: true, certResolver: null }];
    const two = [r, { ...r, name: "sonarr", sub: "sonarr", upstream: "sonarr:8989" }];
    const out = withTraefikRoutes(existing, two, "home.example.com", "le");
    expect(out.added).toEqual(["sonarr.home.example.com"]);
    expect(out.routes.at(-1)).toMatchObject({ tls: true, certResolver: "le", upstream: "sonarr:8989" });
  });

  it("adds one drill per backup job without one", () => {
    const jobs = [{ id: 1, hostId: 2, name: "photos" }, { id: 2, hostId: 2, name: "docs" }] as unknown as Parameters<typeof drillsFor>[0];
    const have = [{ kind: "job", jobId: 1 }] as unknown as Parameters<typeof drillsFor>[1];
    const d = drillsFor(jobs, have, 7);
    expect(d).toHaveLength(1);
    expect(d[0]).toMatchObject({ jobId: 2, hostId: 2, name: "docs restore", everyDays: 7, kind: "job", enabled: true });
  });
});
