import { describe, expect, it } from "vitest";
import type { Discovery, LayoutEntry } from "../api";
import { mockOps } from "../mockOps";
import { buildGraph } from "./graph";
import { bounds, buildTopology, canAttach, expectedEntries, inSubnet, networkOf, NH, NW, type Topology } from "./layout";

const hosts = [
  { id: 1, name: "lab", hostname: "192.168.1.40", color: null },
  { id: 2, name: "nas", hostname: "nas.lan", color: null },
];
const e = (key: string, p: Partial<LayoutEntry> = {}): LayoutEntry => ({ key, x: null, y: null, parent: null, expected: false, ...p });

async function topo(layout: LayoutEntry[] = [], ranges: string[] = []): Promise<{ t: Topology; d: Discovery }> {
  const d = await mockOps.discover(1, true, ranges);
  const graph = buildGraph(hosts, {}, d, 1);
  return { t: buildTopology({ graph, disc: d, layout }), d };
}
const node = (t: Topology, id: string) => t.nodes.find((n) => n.id === id)!;

describe("network topology", () => {
  it("does subnet math", () => {
    expect(inSubnet("192.168.1.77", "192.168.1.0", 24)).toBe(true);
    expect(inSubnet("192.168.2.1", "192.168.1.0", 24)).toBe(false);
    expect(inSubnet("fe80::1", "192.168.1.0", 24)).toBe(false);
    expect(networkOf("10.20.30.40", 16)).toBe("10.20.0.0");
  });

  it("infers what each node connects through", async () => {
    const { t } = await topo();
    expect(node(t, "gw").parent).toBe("net");
    expect(node(t, "net").parent).toBeNull();
    expect(node(t, "d192.168.1.48").parent).toBe("gw");
    expect(node(t, "d100.101.7.12").parent).toBe("ts");
    expect(node(t, "h1").parent).toBe("gw");
    expect(node(t, "d192.168.1.48").icon).toBe("iot");
    expect(node(t, "gw").icon).toBe("router");
    // The scanned host's tailnet side stays as a second, non-tree link.
    expect(t.links).toContainEqual({ from: "ts", to: "h1", kind: "tailnet", tree: false, manual: false });
    expect(t.links.find((l) => l.to === "gw" && l.tree)?.kind).toBe("internet");
  });

  it("keeps the user's parent and refuses loops", async () => {
    const { t } = await topo([e("24:0a:c4:aa:00:48", { parent: "b8:27:eb:12:34:56" })]);
    const lamp = node(t, "d192.168.1.48");
    expect(lamp.parent).toBe("d192.168.1.31");
    expect(lamp.manual).toBe(true);
    expect(t.links).toContainEqual({ from: "d192.168.1.31", to: "d192.168.1.48", kind: "lan", tree: true, manual: true });
    expect(canAttach(t, "d192.168.1.31", "d192.168.1.48")).toBe(false);
    expect(canAttach(t, "d192.168.1.48", "gw")).toBe(true);

    const { t: loop } = await topo([e("24:0a:c4:aa:00:48", { parent: "b8:27:eb:12:34:56" }), e("b8:27:eb:12:34:56", { parent: "24:0a:c4:aa:00:48" })]);
    const a = node(loop, "d192.168.1.48");
    const b = node(loop, "d192.168.1.31");
    // At most one of the two hand-set parents survives, and nothing loops.
    expect([a.manual, b.manual].filter(Boolean).length).toBeLessThan(2);
    for (const n of loop.nodes) {
      const seen = new Set<string>();
      for (let p: string | null = n.id; p; p = node(loop, p).parent) {
        expect(seen.has(p)).toBe(false);
        seen.add(p);
      }
    }
  });

  it("lays out without overlaps, and saved positions carry their children", async () => {
    const { t } = await topo();
    for (const a of t.nodes)
      for (const b of t.nodes) {
        if (a === b) continue;
        expect(Math.abs(a.x - b.x) >= NW || Math.abs(a.y - b.y) >= NH, `${a.id} overlaps ${b.id}`).toBe(true);
      }
    const gw = node(t, "gw");
    const lamp = node(t, "d192.168.1.48");
    const { t: moved } = await topo([e("gw", { x: gw.x + 500, y: gw.y + 100 })]);
    expect(node(moved, "gw").pinned).toBe(true);
    expect(node(moved, "d192.168.1.48").x).toBe(lamp.x + 500);
    expect(node(moved, "d192.168.1.48").y).toBe(lamp.y + 100);
    expect(bounds([gw], 10).w).toBe(NW + 20);
  });

  it("puts subnets and the tailnet in zones", async () => {
    const { t } = await topo([], ["10.20.0.0/24"]);
    const lan = t.zones.find((z) => z.id === "z:192.168.1.0/24")!;
    expect(lan.members).toContain("gw");
    expect(lan.members).toContain("d192.168.1.48");
    expect(t.zones.find((z) => z.kind === "routed")?.members).toEqual(["d10.20.0.10", "d10.20.0.25"]);
    expect(t.links.find((l) => l.to === "d10.20.0.10")?.kind).toBe("routed");
    expect(t.zones.find((z) => z.kind === "tailnet")?.members).toContain("ts");
  });

  it("diffs a scan against the expected picture", async () => {
    const { t: first } = await topo();
    const layout = new Map<string, LayoutEntry>();
    const saved = expectedEntries(first, layout).filter((x) => x.key !== "ip:192.168.1.77");
    saved.push(e("aa:bb:cc:00:00:99", { expected: true, x: 0, y: 900 }));
    const { t } = await topo(saved);
    expect(t.diff.hasExpected).toBe(true);
    expect(t.diff.missing).toEqual(["aa:bb:cc:00:00:99"]);
    expect(t.diff.unexpected).toEqual(["ip:192.168.1.77"]);
    const ghost = node(t, "x:aa:bb:cc:00:00:99");
    expect(ghost.ghost).toBe(true);
    expect(ghost.status).toBe("ghost");
    expect(ghost.parent).toBe("gw");
    expect(node(t, "d192.168.1.77").unexpected).toBe(true);
    // Everything was placed when saved, so nothing moved.
    expect(node(t, "d192.168.1.48").x).toBe(Math.round(node(first, "d192.168.1.48").x));
  });
});
