import { describe, expect, it } from "vitest";
import type { GrowthEntry, GrowthReport, Host } from "./api";
import { childPath, deltaText, growingEntries, linkSuggestions, parentPath } from "./growth";

const e = (path: string, size: number, delta: number | null, o: Partial<GrowthEntry> = {}): GrowthEntry => ({ path, size, dir: true, recent: false, delta, vmImage: false, hint: null, ...o });

describe("what's growing", () => {
  it("walks paths on POSIX and Windows hosts", () => {
    expect(childPath("/mnt/h", "vms/win")).toBe("/mnt/h/vms/win");
    expect(childPath("/", "srv")).toBe("/srv");
    expect(childPath("H:\\", "vms/win")).toBe("H:\\vms\\win");
    expect(parentPath("/mnt/h/vms/win", "/mnt/h")).toBe("/mnt/h/vms");
    expect(parentPath("/mnt/h/vms", "/mnt/h")).toBe("/mnt/h");
    expect(parentPath("/mnt/h", "/mnt/h")).toBe("/mnt/h");
    expect(parentPath("H:\\vms\\win", "H:\\")).toBe("H:\\vms");
    expect(deltaText(null)).toBe("new");
    expect(deltaText(-3e8)).toBe("−300 MB");
  });

  it("lists what explains the growth, not every parent", () => {
    const r = {
      prevAt: 1,
      entries: [e("vms", 900, 6e9), e("vms/web.vhdx", 400, 5.8e9, { dir: false, vmImage: true }), e("media", 300, 1e9), e("logs", 1, -5)],
    } as GrowthReport;
    expect(growingEntries(r).map((x) => x.path)).toEqual(["vms/web.vhdx", "media"]);
    expect(growingEntries({ ...r, prevAt: null })).toEqual([]);
  });

  it("suggests the host a VM image belongs to", () => {
    const hosts = [{ id: 3, name: "web01" }, { id: 4, name: "db" }] as Host[];
    const s = linkSuggestions([e("VMs/Web01.vhdx", 1, 1, { dir: false, vmImage: true })], hosts);
    expect(s.map((x) => x.host.name)).toEqual(["web01"]);
  });
});
