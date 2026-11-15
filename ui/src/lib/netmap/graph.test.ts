import { describe, expect, it } from "vitest";
import type { Discovery, SecurityReport } from "../api";
import { mockOps } from "../mockOps";
import { buildGraph, exposureIp, matchHost, serviceLabel, summarizePorts } from "./graph";

const hosts = [
  { id: 1, name: "lab", hostname: "192.168.1.40", color: null },
  { id: 2, name: "nas", hostname: "nas.lan", color: null },
  { id: 3, name: "far", hostname: "far.example.org", color: null },
];
const audit = (ports: [number, "local" | "vpn" | "address" | "all"][]): SecurityReport => ({
  score: 90,
  grade: "A",
  checks: [],
  processesPartial: false,
  listeners: ports.map(([port, scope]) => ({ proto: "tcp", addr: "0.0.0.0", port, scope, process: null, container: null, service: null })),
});

describe("network map graph", () => {
  it("reads exposure targets", () => {
    const e = { kind: "upnp" as const, port: 1, public: "x", detail: "" };
    expect(exposureIp({ ...e, target: "192.168.1.20:51820" })).toBe("192.168.1.20");
    expect(exposureIp({ ...e, target: "http://127.0.0.1:8080" })).toBeNull();
    expect(exposureIp({ ...e, target: "http://localhost:3000" })).toBeNull();
    expect(exposureIp({ ...e, target: "http://10.0.0.5:80/app" })).toBe("10.0.0.5");
  });

  it("matches devices to hosts by address or name stem", () => {
    const d = { ip: "192.168.1.20", mac: null, names: ["nas.local"], services: [], seen: ["mdns" as const], os: null, online: null };
    expect(matchHost(d, hosts)?.id).toBe(2);
    expect(matchHost({ ...d, names: [] }, hosts)).toBeUndefined();
    expect(matchHost({ ...d, ip: "192.168.1.40", names: [] }, hosts)?.id).toBe(1);
  });

  it("summarizes listening ports by reach", () => {
    expect(summarizePorts(audit([[22, "all"], [22, "all"], [5432, "local"], [8080, "vpn"], [80, "address"]]).listeners)).toEqual({ lan: 2, tailnet: 1, local: 1 });
  });

  it("without a scan shows only the hosts, unlinked", () => {
    const g = buildGraph(hosts, { 1: audit([[22, "all"]]), 3: "Needs a login first" }, null, null);
    expect(g.nodes.map((n) => n.id)).toEqual(["h1", "h2", "h3"]);
    expect(g.links).toEqual([]);
    expect(g.nodes[0].ports?.lan).toBe(1);
    expect(g.nodes[2].portsError).toMatch(/login/);
  });

  it("places the scan: router, tailnet, known hosts, unknown devices, exposures", async () => {
    const d: Discovery = await mockOps.discover(1, true);
    const g = buildGraph(hosts, {}, d, 1);
    const byId = Object.fromEntries(g.nodes.map((n) => [n.id, n]));
    // The router is a node of its own, not an unknown device.
    expect(byId.gw.sub).toContain("router.lan");
    expect(byId["d192.168.1.1"]).toBeUndefined();
    // nas.local is the known host "nas"; it isn't listed as unknown.
    expect(byId["d192.168.1.20"]).toBeUndefined();
    expect(byId.h2.unseen).toBe(false);
    expect(byId.h3.unseen).toBe(true);
    // Unknown LAN + swept + tailnet devices.
    expect(byId["d192.168.1.52"].col).toBe(3);
    expect(byId["d192.168.1.77"]).toBeDefined();
    expect(g.links).toContainEqual({ from: "ts", to: "d100.101.7.12", kind: "tailnet" });
    // Exposures: Plex UPnP and funnel/tunnel land on the scanned host, WireGuard UPnP on the NAS.
    expect(byId.h1.exposures.map((e) => e.kind).sort()).toEqual(["funnel", "tunnel", "upnp"]);
    expect(byId.h2.exposures.map((e) => e.port)).toEqual([51820]);
    expect(g.links).toContainEqual({ from: "net", to: "gw", kind: "internet" });
    expect(g.links).toContainEqual({ from: "gw", to: "h2", kind: "internet" });
    expect(g.links).toContainEqual({ from: "net", to: "h1", kind: "internet" });
    expect(g.links).toContainEqual({ from: "ts", to: "h1", kind: "tailnet" });
  });

  it("labels mDNS services", () => {
    expect(serviceLabel("_smb._tcp")).toBe("SMB share");
    expect(serviceLabel("_foo._tcp")).toBe("foo");
  });
});

describe("scanning from this computer", () => {
  it("adds a node for it and lands local exposures there", async () => {
    const d = await mockOps.discover(0, false);
    const g = buildGraph([], {}, d, "local");
    const me = g.nodes.find((n) => n.id === "local");
    expect(me?.label).toBe("This computer");
    expect(me?.sub).toBe("192.168.1.40");
    expect(g.links).toContainEqual({ from: "gw", to: "local", kind: "internet" });
    expect(me?.exposures.map((e) => e.kind).sort()).toEqual(["funnel", "tunnel", "upnp"]);
  });
});
