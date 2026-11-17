import { describe, expect, it } from "vitest";
import type { LayoutEntry, NetDevice } from "../api";
import { mockOps } from "../mockOps";
import { buildGraph } from "./graph";
import { buildTopology } from "./layout";
import { overlayGraph, overlayScanner } from "./networks";

const hosts = [
  { id: 1, name: "lab", hostname: "192.168.1.40", color: null },
  { id: 3, name: "builder", hostname: "10.20.0.5", color: null },
];
const home = { id: 1, name: "Home", subnet: "192.168.1.0/24", origin: "1" };
const lab = { id: 2, name: "Lab", subnet: "10.20.0.0/24", origin: "3" };

async function scans() {
  const h = (await mockOps.networkScan(1))!;
  const l = (await mockOps.networkScan(2))!;
  return { h, l };
}

describe("saved networks on the map", () => {
  it("only pins a kept scan to a host, never to this computer", () => {
    expect(overlayScanner("3")).toBe(3);
    expect(overlayScanner("local")).toBeNull();
  });

  it("overlays another network behind its own router, linked through the internet", async () => {
    const { h, l } = await scans();
    const base = buildGraph(hosts, {}, h, 1);
    const g = overlayGraph(base, h, hosts, {}, [{ net: lab, disc: l }]);
    expect(g.nets).toEqual([{ id: 2, name: "Lab", subnet: "10.20.0.0/24", prefix: "n2:" }]);
    // One Internet, two routers.
    expect(g.nodes.filter((n) => n.kind === "internet")).toHaveLength(1);
    expect(g.nodes.filter((n) => n.kind === "gateway").map((n) => n.id)).toEqual(["gw", "n2:gw"]);
    expect(g.links).toContainEqual({ from: "net", to: "n2:gw", kind: "wan" });
    expect(g.nodes.find((n) => n.id === "net")?.sub).toMatch(/^links 2 networks/);
    // The lab's host was "not seen" by the home scan; the lab's scan places it.
    expect(g.nodes.find((n) => n.id === "h3")?.unseen).toBe(false);

    const t = buildTopology({ graph: g, disc: h, layout: [] });
    const p = (id: string) => t.nodes.find((n) => n.id === id)?.parent;
    expect(p("n2:gw")).toBe("net");
    expect(p("gw")).toBe("net");
    expect(p("n2:d10.20.0.12")).toBe("n2:gw");
    expect(p("h3")).toBe("n2:gw");
    expect(p("h1")).not.toMatch(/^n2:/);
    const zone = t.zones.find((z) => z.id === "z:n2:")!;
    expect(zone.label).toBe("Lab · 10.20.0.0/24 · last scan");
    expect(zone.members).toEqual(expect.arrayContaining(["n2:gw", "n2:d10.20.0.12", "h3"]));
    // The shown network's zone carries its name.
    expect(t.zones.find((z) => z.id === "z:192.168.1.0/24")?.label).toBe("Home · LAN 192.168.1.0/24");
    // Overlaid devices aren't "not in the expected map" of the shown network.
    expect(t.nodes.filter((n) => n.id.startsWith("n2:")).every((n) => !n.unexpected)).toBe(true);
  });

  it("draws a device seen by both scans once, and ignores the network on show", async () => {
    const { h } = await scans();
    const base = buildGraph(hosts, {}, h, 1);
    expect(overlayGraph(base, h, hosts, {}, [{ net: home, disc: h }])).toBe(base);
    const again = { ...h, network: { id: 9, name: "Twin", isNew: false } };
    const g = overlayGraph(base, h, hosts, {}, [{ net: { ...home, id: 9, name: "Twin" }, disc: again }]);
    expect(g.nodes.filter((n) => n.kind === "device").length).toBe(base.nodes.filter((n) => n.kind === "device").length);
    expect(g.nodes.some((n) => n.id === "n9:gw")).toBe(false);
  });

  it("never calls another network's devices missing", async () => {
    const { h } = await scans();
    const inv = (key: string, ip: string, network: number | null): NetDevice => ({ key, ip, vendor: null, kind: "other", name: null, origin: "3", firstSeen: 0, lastSeen: 0, trusted: true, network });
    const exp = (key: string): LayoutEntry => ({ key, x: 0, y: 0, parent: null, expected: true });
    const inventory = [
      inv("00:11:32:00:00:12", "10.20.0.12", 2), // the lab's NAS
      inv("52:54:00:00:00:30", "10.20.0.30", null), // seen before saved networks, not on this LAN
      inv("aa:bb:cc:00:00:99", "192.168.1.99", 1), // this network's, gone
      inv("aa:bb:cc:00:00:98", "192.168.1.98", null), // from before saved networks, on this LAN
    ];
    const layout = inventory.map((d) => exp(d.key));
    const t = buildTopology({ graph: buildGraph(hosts, {}, h, 1), disc: h, layout, inventory });
    expect(t.diff.missing.sort()).toEqual(["aa:bb:cc:00:00:98", "aa:bb:cc:00:00:99"]);
    // Overlaid, the lab's NAS is simply there.
    const { l } = await scans();
    const g = overlayGraph(buildGraph(hosts, {}, h, 1), h, hosts, {}, [{ net: lab, disc: l }]);
    const t2 = buildTopology({ graph: g, disc: h, layout, inventory });
    expect(t2.nodes.find((n) => n.key === "00:11:32:00:00:12")?.ghost).toBe(false);
  });
});
