import { describe, expect, it } from "vitest";
import { FEATURES } from "./features";
import { COMPONENTS, CORE, SHELVES, installedFor, ownerOf, setInstalled } from "./storeCatalog";

describe("feature store catalog", () => {
  it("only brings known features and every component sits on a shelf", () => {
    for (const c of COMPONENTS) {
      for (const f of c.features) expect(FEATURES.some((x) => x.id === f), `${c.id}: ${f}`).toBe(true);
      expect(SHELVES.some((s) => s.id === c.shelf)).toBe(true);
    }
    expect(new Set(COMPONENTS.map((c) => c.id)).size).toBe(COMPONENTS.length);
  });

  it("finds owners, children included", () => {
    expect(ownerOf("proxmox.sandbox")?.id).toBe("proxmox");
    expect(ownerOf("fleet.ups")?.id).toBe("ups");
    expect(ownerOf("fleet")).toBeUndefined();
    expect(ownerOf("docker")).toBeUndefined();
    expect(ownerOf("networkx")).toBeUndefined();
  });

  it("treats a never-chosen list as everything installed", () => {
    expect(installedFor(null, "proxmox")).toBe(true);
    expect(installedFor([], "proxmox")).toBe(false);
    expect(installedFor([], "terminal")).toBe(true);
    expect(installedFor(["ups"], "fleet.ups")).toBe(true);
  });

  it("installs and removes, keeping unknown ids", () => {
    expect(setInstalled(["future-thing"], "ups", true)).toEqual(["future-thing", "ups"]);
    expect(setInstalled(["future-thing", "ups"], "ups", false)).toEqual(["future-thing"]);
    const all = setInstalled(null, "drift", false);
    expect(all).not.toContain("drift");
    expect(all.length).toBe(COMPONENTS.length - 1);
  });

  it("says what every component holds, with an example", () => {
    for (const c of COMPONENTS) {
      expect(c.adds.length, c.id).toBeGreaterThan(0);
      expect(c.example.length, c.id).toBeGreaterThan(20);
    }
    expect(CORE.length).toBeGreaterThan(5);
  });
});
