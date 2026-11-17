import { describe, expect, it } from "vitest";
import { applyPreset, FEATURES, featureOn, PRESETS, presetOf, toggleFeature } from "./features";
import { SECTIONS } from "./sections";

describe("features", () => {
  it("parents turn children off; overview and settings stay on", () => {
    expect(featureOn(["fleet"], "fleet.logs")).toBe(false);
    expect(featureOn(["fleet"], "fleetx")).toBe(true);
    expect(featureOn(["overview", "settings"], "overview")).toBe(true);
    expect(featureOn(["overview", "settings"], "settings")).toBe(true);
  });

  it("every section but overview, settings and the library can be turned off", () => {
    for (const s of SECTIONS) {
      if (s.id === "overview" || s.id === "settings" || s.id === "library") continue;
      expect(FEATURES.some((f) => f.id === s.id)).toBe(true);
    }
    for (const f of FEATURES) if (f.parent) expect(FEATURES.some((p) => p.id === f.parent)).toBe(true);
  });

  it("presets round-trip and keep unknown ids", () => {
    for (const p of PRESETS) expect(presetOf(applyPreset([], p.id))).toBe(p.id);
    const next = applyPreset(["from-newer-version"], "simple");
    expect(next).toContain("from-newer-version");
    expect(presetOf(next)).toBe("simple");
    expect(presetOf(["docker"])).toBeNull();
    for (const p of PRESETS) for (const d of p.disabled) expect(FEATURES.some((f) => f.id === d)).toBe(true);
  });

  it("turning a child on under an off parent keeps its siblings off", () => {
    const next = toggleFeature(["fleet"], "fleet.ups", true);
    expect(featureOn(next, "fleet")).toBe(true);
    expect(featureOn(next, "fleet.ups")).toBe(true);
    expect(featureOn(next, "fleet.logs")).toBe(false);
    expect(toggleFeature(next, "fleet.ups", false)).toContain("fleet.ups");
    expect(toggleFeature(["docker"], "docker", true)).toEqual([]);
  });
});
