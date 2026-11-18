import { describe, expect, it } from "vitest";
import { ago, ntfyProblem, target, toggleRoute } from "./notices";

describe("notices", () => {
  it("checks ntfy URLs like the backend", () => {
    expect(ntfyProblem("https://ntfy.sh/x")).toMatch(/public/);
    expect(ntfyProblem("https://eu.ntfy.sh/x")).toMatch(/public/);
    expect(ntfyProblem("https://ntfy.home.lan")).toMatch(/topic/);
    expect(ntfyProblem("http://10.0.0.5:8080/alerts")).toBeNull();
    expect(ntfyProblem("ntfy.home.lan/x")).toMatch(/https/);
    expect(ntfyProblem("https://notntfy.sh/x")).toBeNull();
  });
  it("toggles routes", () => {
    expect(toggleRoute([], "ntfy:drill", false)).toEqual(["ntfy:drill"]);
    expect(toggleRoute(["ntfy:drill", "native:alert"], "ntfy:drill", true)).toEqual(["native:alert"]);
  });
  it("splits targets and formats ages", () => {
    expect(target({ view: "fleet/drills" })).toEqual({ view: "fleet", tab: "drills" });
    expect(target({ view: null })).toBeNull();
    expect(ago(1000, 1030)).toBe("just now");
    expect(ago(1000, 1000 + 7200)).toBe("2 h");
  });
});
