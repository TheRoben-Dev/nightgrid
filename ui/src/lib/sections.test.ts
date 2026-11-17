import { describe, expect, it } from "vitest";
import { RAIL, SECTIONS, railPath, railTarget } from "./sections";

describe("sidebar rows", () => {
  it("every view but Settings is in exactly one row", () => {
    for (const s of SECTIONS) {
      const n = RAIL.filter((r) => r.members.includes(s.id)).length;
      expect(n, s.id).toBe(s.id === "settings" ? 0 : 1);
    }
  });
  it("names where a view lives", () => {
    expect(railPath("system")).toBe("System → One host");
    expect(railPath("fleet")).toBe("System → Fleet");
    expect(railPath("tunnels")).toBe("Network → Tunnels");
    expect(railPath("notes")).toBe("Knowledge → Notes");
    expect(railPath("library")).toBe("Knowledge → Library");
    expect(railPath("settings")).toBe("Settings");
  });
  it("maps a view's guide id to its row's button", () => {
    expect(railTarget("rail-desktops")).toEqual({ id: "rail-proxmox", row: "vms", view: "desktops" });
    expect(railTarget("rail-docker")).toEqual({ id: "rail-docker", row: "docker", view: "docker" });
    expect(railTarget("rail-settings")).toBeNull();
    expect(railTarget("palette")).toBeNull();
  });
});
