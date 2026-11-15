import { describe, expect, it } from "vitest";
import type { Vm } from "../api";
import { maskUrl } from "./statusPage";
import { vmActions, vmCounts, vmTone } from "./vms.svelte";

const vm = (state: string): Vm => ({ kind: "qemu", id: "100", name: "ha", state, vcpus: 2, memoryMb: 2048, autostart: true });

describe("vms", () => {
  it("offers actions that fit the state", () => {
    expect(vmActions(vm("running"))).toEqual(["shutdown", "reboot", "stop"]);
    expect(vmActions(vm("stopped"))).toEqual(["start"]);
    expect(vmActions(vm("paused"))).toEqual(["stop"]);
  });
  it("tones and counts", () => {
    expect(vmTone(vm("running"))).toBe("ok");
    expect(vmTone(vm("stopped"))).toBe("off");
    expect(vmTone(vm("paused"))).toBe("warn");
    expect(vmCounts({ vms: [vm("running"), vm("stopped")], libvirt: false, proxmox: true, needsRoot: false, notes: [] })).toEqual({ running: 1, total: 2 });
    expect(vmCounts(null)).toEqual({ running: 0, total: 0 });
  });
});

describe("status page", () => {
  it("masks the token", () => {
    const m = maskUrl("http://192.168.1.4:8787/s/3f9c2a71d0b84e6f");
    expect(m).toBe("http://192.168.1.4:8787/s/3f9c" + "•".repeat(12));
    expect(m).not.toContain("d0b84e6f");
  });
});
