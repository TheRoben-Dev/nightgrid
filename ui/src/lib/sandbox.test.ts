import { describe, expect, it } from "vitest";
import { bridgeChoices, expiryText, expiryTone, ownTags, sandboxName, sandboxOf, sandboxes } from "./sandbox";
import type { PveGuest } from "./proxmox";

const g = (vmid: number, tags: string[], name = "media"): PveGuest => ({
  vmid, kind: "qemu", node: "pve1", name, status: "stopped", template: false, cpu: 0, maxcpu: 2, mem: 0, maxmem: 0, disk: 0, maxdisk: 0, uptime: 0, tags, lock: null, hastate: null,
});

describe("sandbox", () => {
  it("reads the marks from tags", () => {
    expect(sandboxOf(g(100, ["prod"]))).toBeNull();
    const s = sandboxOf(g(9000, ["web", "nightgrid-sandbox", "ng-sbx-of-100", "ng-sbx-until-1800000000"]));
    expect(s?.source).toBe(100);
    expect(s?.until).toBe(1800000000);
    expect(sandboxOf(g(9001, ["nightgrid-sandbox"]))?.until).toBeNull();
  });

  it("lists soonest expiry first, never last", () => {
    const list = sandboxes([g(9002, ["nightgrid-sandbox"]), g(9001, ["nightgrid-sandbox", "ng-sbx-until-50"]), g(100, []), g(9000, ["nightgrid-sandbox", "ng-sbx-until-40"])]);
    expect(list.map((s) => s.guest.vmid)).toEqual([9000, 9001, 9002]);
  });

  it("hides our tags", () => {
    expect(ownTags(["web", "nightgrid-sandbox", "ng-sbx-of-1", "ng-sbx-until-2"])).toEqual(["web"]);
  });

  it("says when it expires", () => {
    expect(expiryText(null)).toBe("never expires");
    expect(expiryText(1000 + 600, 1000)).toBe("expires in 10 min");
    expect(expiryText(1000 + 3 * 3600 + 1200, 1000)).toBe("expires in 3 h 20 min");
    expect(expiryText(1000 + 3 * 86400, 1000)).toBe("expires in 3 days");
    expect(expiryText(1000 - 300, 1000)).toBe("overdue by 5 min");
    expect(expiryTone(900, 1000)).toBe("err");
    expect(expiryTone(1000 + 1800, 1000)).toBe("warn");
    expect(expiryTone(null, 1000)).toBe("ok");
  });

  it("names the copy", () => {
    expect(sandboxName(g(100, []))).toBe("media-sandbox");
    expect(sandboxName(g(100, [], ""))).toBe("guest-100-sandbox");
    expect(sandboxName(g(100, [], "a".repeat(70))).length).toBeLessThanOrEqual(63);
  });

  it("puts port-less bridges first", () => {
    const c = bridgeChoices([
      { name: "vmbr0", cidr: null, vlanAware: false, ports: "eno1", comment: "", active: true },
      { name: "vmbr9", cidr: null, vlanAware: false, ports: "", comment: "lab", active: true },
    ]);
    expect(c[0]).toMatchObject({ name: "vmbr9", isolated: true });
    expect(c[1].label).toContain("ports: eno1");
  });
});
