import { describe, expect, it } from "vitest";
import type { HostCapacity, Replay } from "./api";
import { WORKLOADS, catalogMatch, duration, eventsUntil, fit, pointAt, sizing } from "./fleet11";

const GB = 1e9;
const cap = (p: Partial<HostCapacity>): HostCapacity => ({ hostId: 1, spec: { hostId: 1, cores: 8, memTotal: 32 * GB, at: 0 }, cpuAvg: 10, cpuP95: 30, memAvg: 40, memP95: 50, gpuP95: null, hours: 200, disks: [{ mount: "/", size: 500 * GB, used: 100 * GB, daysLeft: null }], ...p });
const w = (id: string) => WORKLOADS.find((x) => x.id === id)!;

describe("capacity", () => {
  it("fits, gets tight and refuses", () => {
    expect(fit(cap({}), w("container")).verdict).toBe("fits");
    expect(fit(cap({ memP95: 80 }), w("windows-vm")).verdict).toBe("no");
    const t = fit(cap({ cpuP95: 70 }), w("transcode-1080"));
    expect([t.verdict, t.cpuAfter]).toEqual(["tight", 95]);
    expect(fit(cap({ disks: [{ mount: "/", size: 40 * GB, used: 35 * GB, daysLeft: null }] }), w("db")).why).toContain("no disk");
    expect(fit(cap({ spec: null }), w("db")).verdict).toBe("unknown");
  });
  it("sizes a server with headroom", () => {
    expect(sizing([{ w: w("windows-vm"), n: 1 }, { w: w("linux-vm"), n: 2 }])).toEqual({ cores: 8, memGb: 32, diskGb: 256 });
  });
});

describe("replay", () => {
  const r: Replay = {
    incident: { kind: "outage", hostId: 1, serviceId: null, title: "x", detail: "", start: 100, end: 200, level: "critical" },
    from: 0,
    to: 300,
    metrics: [0, 60, 120, 180].map((ts) => ({ ts, cpu: ts, mem: null, swap: null, load1: null, disk: null, netRx: null, netTx: null, diskRead: null, diskWrite: null, temp: null, gpu: null, gpuMem: null, gpuTemp: null, gpuPower: null })),
    events: [{ at: 50, source: "change", level: "info", title: "a", detail: "", hostId: 1 }, { at: 150, source: "event", level: "warn", title: "b", detail: "", hostId: 1 }],
  };
  it("scrubs", () => {
    expect(eventsUntil(r, 100).map((e) => e.title)).toEqual(["a"]);
    expect(pointAt(r.metrics, 130)?.cpu).toBe(120);
    expect(pointAt(r.metrics, -1)).toBeNull();
    expect([duration(45), duration(600), duration(7200)]).toEqual(["45 s", "10 min", "2.0 h"]);
  });
  it("matches catalog entries", () => {
    const e = { name: "jellyfin", image: "j:1", version: null, state: "running", health: null, project: "media", ports: [], urls: ["https://media.lan"], mounts: [{ source: "/srv/media", destination: "/m", kind: "bind", name: null, backedUpBy: null }], coverage: "none" as const };
    expect([catalogMatch(e, "MEDIA.lan"), catalogMatch(e, "/srv"), catalogMatch(e, "plex")]).toEqual([true, true, false]);
  });
});
