/**
 * The network map as a topology (D-141): which node each device connects
 * through, subnet zones, an automatic tree layout the user can rearrange, and
 * the difference between this scan and the saved "expected" picture.
 *
 * Pure: takes the graph from `buildGraph` plus the saved layout, so it is
 * unit-tested without a backend.
 */
import type { DeviceLabel, Discovery, LayoutEntry, NetDevice } from "../api";
import { KIND_LABEL, labelKey, type LinkKind, type MapNode, type NetGraph } from "./graph";

/** A router node: the shown scan's (`gw`) or an overlaid network's (`n<id>:gw`). */
export const isRouterId = (id: string) => id === "gw" || /^n\d+:gw$/.test(id);

/** Node box size and spacing, in map units. */
export const NW = 184;
export const NH = 52;
const HG = 18;
const VG = 46;
const ROOT_GAP = 60;

export type Status = "ok" | "new" | "offline" | "unseen" | "ghost" | "exposed";

export interface TopoNode {
  id: string;
  /** The layout key (`LayoutEntry.key`). */
  key: string;
  node: MapNode;
  /** Icon / type id (`KIND_LABEL` ids plus `internet`, `tailnet`). */
  icon: string;
  /** Parent node id. */
  parent: string | null;
  /** The parent was set by the user. */
  manual: boolean;
  x: number;
  y: number;
  /** The user placed it (a saved position). */
  pinned: boolean;
  /** Expected but not in this scan. */
  ghost: boolean;
  status: Status;
  /** Not part of the saved expected picture (only when one exists). */
  unexpected: boolean;
}

export interface TopoLink {
  from: string;
  to: string;
  kind: LinkKind;
  /** The tree edge (device → what it connects through). */
  tree: boolean;
  manual: boolean;
}

export interface Zone {
  id: string;
  label: string;
  kind: "lan" | "routed" | "tailnet";
  members: string[];
}

export interface TopoDiff {
  hasExpected: boolean;
  /** Keys of expected nodes this scan didn't see. */
  missing: string[];
  /** Keys of devices this scan saw that aren't in the expected picture. */
  unexpected: string[];
}

export interface Topology {
  nodes: TopoNode[];
  links: TopoLink[];
  zones: Zone[];
  diff: TopoDiff;
}

/** Device types a device can connect through (besides the router, tailnet and hosts). */
export const PARENT_KINDS = new Set(["router", "ap", "switch", "server", "nas", "vm"]);

const v4 = (ip: string): number | null => {
  const p = ip.split(".");
  if (p.length !== 4) return null;
  let n = 0;
  for (const x of p) {
    if (!/^\d{1,3}$/.test(x) || Number(x) > 255) return null;
    n = n * 256 + Number(x);
  }
  return n;
};
const mask = (prefix: number) => (prefix <= 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0);

export function inSubnet(ip: string, net: string, prefix: number): boolean {
  const a = v4(ip);
  const b = v4(net);
  if (a === null || b === null || prefix < 0 || prefix > 32) return false;
  return ((a & mask(prefix)) >>> 0) === ((b & mask(prefix)) >>> 0);
}

export function networkOf(addr: string, prefix: number): string | null {
  const a = v4(addr);
  if (a === null) return null;
  const n = (a & mask(prefix)) >>> 0;
  return [n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].join(".");
}

/** The saved-layout key of a map node. */
export function nodeKey(n: MapNode): string {
  if (n.kind === "host" && n.hostId !== undefined) return `host:${n.label}`;
  // Each network's router has its own place on the map: keyed by its MAC (D-154).
  if (n.kind === "gateway" && n.device?.mac) return `gw:${labelKey(n.device.mac, n.ip ?? "")}`;
  if (n.kind === "device" && n.device) return labelKey(n.device.mac, n.device.ip);
  return n.id;
}

export function iconOf(n: MapNode): string {
  switch (n.kind) {
    case "internet":
      return "internet";
    case "gateway":
      return n.named && n.named.kind !== "other" ? n.named.kind : "router";
    case "tailnet":
      return "tailnet";
    case "host":
      return n.id === "local" ? "desktop" : "server";
    default: {
      const k = n.named && n.named.kind !== "other" ? n.named.kind : n.device?.kind && n.device.kind !== "other" ? n.device.kind : (n.hint ?? "other");
      return k in KIND_LABEL ? k : "other";
    }
  }
}

/** Whether a node can be what other devices connect through. */
export function canParent(t: Pick<TopoNode, "node" | "icon">): boolean {
  const k = t.node.kind;
  return k === "gateway" || k === "tailnet" || k === "host" || (k === "device" && PARENT_KINDS.has(t.icon));
}

function statusOf(n: MapNode): Status {
  if (n.exposures.length && n.kind !== "internet") return "exposed";
  if (n.unseen) return "unseen";
  if (n.device?.online === false) return "offline";
  if (n.device?.isNew && !n.device.trusted) return "new";
  return "ok";
}

export interface TopoInput {
  graph: NetGraph;
  disc: Discovery | null;
  layout: readonly LayoutEntry[];
  inventory?: readonly NetDevice[];
  labels?: ReadonlyMap<string, DeviceLabel>;
}

export function buildTopology({ graph, disc, layout, inventory = [], labels = new Map() }: TopoInput): Topology {
  const saved = new Map(layout.map((e) => [e.key, e]));
  const nodes: TopoNode[] = graph.nodes.map((n) => {
    const icon = iconOf(n);
    return { id: n.id, key: nodeKey(n), node: n, icon, parent: null, manual: false, x: 0, y: 0, pinned: false, ghost: false, status: statusOf(n), unexpected: false };
  });
  const byKey = new Map(nodes.map((t) => [t.key, t]));
  const byId0 = new Map(nodes.map((t) => [t.id, t]));
  // Parents set by hand before routers had their own keys say `gw`: the shown scan's router.
  const gwNode = byId0.get("gw");
  if (gwNode && !byKey.has("gw")) byKey.set("gw", gwNode);

  // Expected nodes this scan didn't see come back as ghosts.
  const expected = layout.filter((e) => e.expected);
  const isDeviceKey = (k: string) => k.startsWith("ip:") || /^[0-9a-f]{2}(:[0-9a-f]{2}){5}$/.test(k);
  const diff: TopoDiff = { hasExpected: expected.length > 0, missing: [], unexpected: [] };
  // A device last seen on another saved network is there, not missing from this one (D-154).
  const thisNet = disc?.network?.id ?? null;
  const elsewhere = (inv: NetDevice | undefined) => thisNet !== null && inv !== undefined && inv.network !== null && inv.network !== thisNet;
  const onThisLan = (ip: string | undefined) =>
    !disc || !ip || disc.ranges.concat(disc.ifaces.filter((f) => f.kind === "private" && !f.addr.includes(":")).map((f) => `${networkOf(f.addr, f.prefix)}/${f.prefix}`)).some((r) => {
      const [n, p] = r.split("/");
      return inSubnet(ip, n, Number(p));
    });
  if (disc && diff.hasExpected) {
    for (const e of expected) {
      if (byKey.has(e.key) || !(isDeviceKey(e.key) || (e.key === "gw" && !byId0.has("gw")))) continue;
      const inv = inventory.find((d) => d.key === e.key);
      if (elsewhere(inv)) continue;
      // Not placed on a network yet (from before saved networks): only missing here when its address is on this scan's LAN.
      if ((inv?.network ?? null) === null && !onThisLan(inv?.ip ?? (e.key.startsWith("ip:") ? e.key.slice(3) : undefined))) continue;
      diff.missing.push(e.key);
      const lab = labels.get(e.key);
      const ip = inv?.ip ?? (e.key.startsWith("ip:") ? e.key.slice(3) : undefined);
      const node: MapNode = {
        id: `x:${e.key}`,
        kind: "device",
        label: lab?.name || inv?.name || ip || e.key,
        sub: `expected · not seen${ip ? ` · ${ip}` : ""}`,
        col: 3,
        row: 0,
        ip,
        exposures: [],
        unseen: true,
        named: lab,
      };
      const kind = lab && lab.kind !== "other" ? lab.kind : (inv?.kind ?? "other");
      const t: TopoNode = { id: node.id, key: e.key, node, icon: kind in KIND_LABEL ? kind : "other", parent: null, manual: false, x: 0, y: 0, pinned: false, ghost: true, status: "ghost", unexpected: false };
      nodes.push(t);
      byKey.set(e.key, t);
    }
    const exp = new Set(expected.map((e) => e.key));
    for (const t of nodes) {
      if (!t.ghost && t.node.kind === "device" && t.node.network === undefined && !exp.has(t.key)) {
        t.unexpected = true;
        diff.unexpected.push(t.key);
      }
    }
  }
  const byId = new Map(nodes.map((t) => [t.id, t]));

  // Parents: the user's choice, else what the scan implies.
  const graphLink = new Map<string, LinkKind>();
  const incoming = new Map<string, string[]>();
  for (const l of graph.links) {
    graphLink.set(`${l.from}>${l.to}`, l.kind);
    if (l.from !== "net") incoming.set(l.to, [...(incoming.get(l.to) ?? []), l.from]);
  }
  // The most specific thing the scan saw it behind: a switch or router, then the main router, then the tailnet.
  const inferred = (t: TopoNode): string | null => {
    if (t.ghost) return byId.has("gw") ? "gw" : null;
    if (t.node.kind === "gateway") return byId.has("net") ? "net" : null;
    if (t.id === "net" || t.id === "ts") return null;
    const f = incoming.get(t.id) ?? [];
    return f.find((x) => !isRouterId(x) && x !== "ts") ?? (f.includes("gw") ? "gw" : (f.find(isRouterId) ?? (f.includes("ts") ? "ts" : null)));
  };
  for (const t of nodes) {
    const p = saved.get(t.key)?.parent;
    const m = p ? byKey.get(p) : undefined;
    if (m && m.id !== t.id && m.node.kind !== "internet") {
      t.parent = m.id;
      t.manual = true;
    } else {
      t.parent = inferred(t);
    }
  }
  // A hand-set parent that loops back falls back to the inferred one.
  for (const t of nodes) {
    const seen = new Set([t.id]);
    let p = t.parent;
    while (p) {
      if (seen.has(p)) {
        t.parent = inferred(t);
        t.manual = false;
        if (t.parent && wouldLoop(byId, t.id, t.parent)) t.parent = null;
        break;
      }
      seen.add(p);
      p = byId.get(p)?.parent ?? null;
    }
  }

  // Links: tree edges first, then the scan's other links (a host's tailnet side, direct exposures).
  const links: TopoLink[] = [];
  const pair = new Set<string>();
  for (const t of nodes) {
    if (!t.parent) continue;
    const g = graphLink.get(`${t.parent}>${t.id}`);
    const kind: LinkKind = g ?? (t.parent === "ts" ? "tailnet" : t.node.device?.routed ? "routed" : t.node.kind === "gateway" ? "wan" : "lan");
    links.push({ from: t.parent, to: t.id, kind, tree: true, manual: t.manual });
    pair.add([t.parent, t.id].sort().join("|"));
  }
  for (const l of graph.links) {
    const k = [l.from, l.to].sort().join("|");
    if (pair.has(k)) continue;
    pair.add(k);
    links.push({ from: l.from, to: l.to, kind: l.kind, tree: false, manual: false });
  }

  const zones = buildZones(nodes, disc, graph);
  const zoneOf = new Map<string, string>();
  for (const z of zones) for (const m of z.members) if (!zoneOf.has(m)) zoneOf.set(m, z.id);

  place(nodes, byId, zoneOf, saved);
  return { nodes, links, zones, diff };
}

function wouldLoop(byId: Map<string, TopoNode>, id: string, parent: string): boolean {
  let p: string | null = parent;
  const seen = new Set<string>();
  while (p) {
    if (p === id || seen.has(p)) return true;
    seen.add(p);
    p = byId.get(p)?.parent ?? null;
  }
  return false;
}

/** Whether `parent` can be set as `id`'s parent without a loop. */
export function canAttach(topo: Topology, id: string, parent: string): boolean {
  if (id === parent) return false;
  const byId = new Map(topo.nodes.map((t) => [t.id, t]));
  return !wouldLoop(byId, id, parent);
}

function buildZones(nodes: TopoNode[], disc: Discovery | null, graph?: NetGraph): Zone[] {
  const zones: Zone[] = [];
  // Overlaid saved networks: one zone each, with the hosts the scan placed behind its router.
  const overlaid: Zone[] = (graph?.nets ?? []).map((o) => ({
    id: `z:${o.prefix}`,
    label: `${o.name}${o.subnet && !o.name.includes(o.subnet) ? ` · ${o.subnet}` : ""} · last scan`,
    kind: "lan" as const,
    members: nodes
      .filter((t) => t.id.startsWith(o.prefix) || (t.node.kind === "host" && t.node.network === o.id && t.parent?.startsWith(o.prefix)))
      .map((t) => t.id),
  }));
  if (!disc) return overlaid.filter((z) => z.members.length > 0);
  const netName = disc.network?.name;
  const nets = new Set<string>();
  for (const f of disc.ifaces) {
    if (f.kind !== "private" || f.addr.includes(":") || f.prefix < 8) continue;
    const net = networkOf(f.addr, f.prefix);
    if (!net || nets.has(`${net}/${f.prefix}`)) continue;
    nets.add(`${net}/${f.prefix}`);
    const cidr = `${net}/${f.prefix}`;
    const named = netName && zones.length === 0 && (!disc.gateway || inSubnet(disc.gateway, net, f.prefix));
    zones.push({ id: `z:${cidr}`, label: named ? (netName.includes(cidr) ? netName : `${netName} · LAN ${cidr}`) : `LAN ${cidr}`, kind: "lan", members: [] });
  }
  for (const r of disc.ranges) {
    const [net, p] = r.split("/");
    if (!net || !p || nets.has(r)) continue;
    nets.add(r);
    const hops = disc.paths?.find((x) => x.range === r)?.hops.filter((h) => !inSubnet(h, net, Number(p))) ?? [];
    const via = hops.length > 1 ? ` via ${hops.at(-1)} (${hops.length} hops)` : "";
    zones.push({ id: `z:${r}`, label: `ROUTED ${r}${via}`, kind: "routed", members: [] });
  }
  for (const z of zones) {
    const [net, p] = z.id.slice(2).split("/");
    for (const t of nodes) {
      const ip = t.node.ip ?? (t.node.kind === "host" && v4(t.node.sub) !== null ? t.node.sub : null);
      // Only nodes on the scanned network: a host the scan didn't see stays out.
      if (ip && t.node.network === undefined && (t.parent || t.id === "gw") && inSubnet(ip, net, Number(p))) z.members.push(t.id);
    }
  }
  const ts = nodes.filter((t) => t.id === "ts" || t.parent === "ts");
  if (ts.length > 1) zones.push({ id: "z:tailnet", label: "TAILNET", kind: "tailnet", members: ts.map((t) => t.id) });
  return [...zones, ...overlaid].filter((z) => z.members.length > 0);
}

const ORDER: Record<string, number> = { internet: 0, router: 1, tailnet: 2, ap: 3, switch: 4, server: 5, nas: 6, desktop: 7 };

/**
 * Tree layout: each parent above its children; leaf children wrap into a
 * grid, branches sit side by side. Saved positions win, and a node the user
 * hasn't placed moves with its parent.
 */
function place(nodes: TopoNode[], byId: Map<string, TopoNode>, zoneOf: Map<string, string>, saved: Map<string, LayoutEntry>) {
  const kids = new Map<string, TopoNode[]>();
  const roots: TopoNode[] = [];
  for (const t of nodes) {
    if (t.parent && byId.has(t.parent)) {
      const l = kids.get(t.parent) ?? [];
      l.push(t);
      kids.set(t.parent, l);
    } else roots.push(t);
  }
  const rank = (t: TopoNode) => (t.node.kind === "host" ? 5.5 : (ORDER[t.icon] ?? 10));
  const sort = (l: TopoNode[]) =>
    l.sort(
      (a, b) =>
        (zoneOf.get(a.id) ?? "~").localeCompare(zoneOf.get(b.id) ?? "~") ||
        rank(a) - rank(b) ||
        Number(a.ghost) - Number(b.ghost) ||
        a.node.label.localeCompare(b.node.label) ||
        a.id.localeCompare(b.id),
    );
  sort(roots);
  for (const l of kids.values()) sort(l);

  // A block is one zone's worth of children: leaves in a grid, then branches.
  type Block = { leaves: TopoNode[]; branches: TopoNode[]; cols: number; gw: number; w: number; h: number };
  const ZG = HG * 3;
  const size = new Map<string, { w: number; h: number; blocks: Block[]; bw: number }>();
  const block = (list: TopoNode[], maxCols: number): Block => {
    const leaves = list.filter((c) => !kids.has(c.id));
    const branches = list.filter((c) => kids.has(c.id));
    const bs = branches.map(measure);
    const cols = leaves.length ? Math.min(leaves.length, maxCols, Math.max(2, Math.ceil(Math.sqrt(leaves.length * 1.6)))) : 0;
    const rows = cols ? Math.ceil(leaves.length / cols) : 0;
    const gw = cols ? cols * (NW + HG) - HG : 0;
    const gh = rows ? rows * (NH + HG) - HG : 0;
    const brW = bs.reduce((x, y) => x + y.w, 0) + Math.max(0, bs.length - 1) * HG * 2;
    return { leaves, branches, cols, gw, w: gw + brW + (gw && brW ? HG * 2 : 0), h: Math.max(gh, ...bs.map((y) => y.h), 0) };
  };
  const blocksOf = (list: TopoNode[], maxCols: number): Block[] => {
    const groups = new Map<string, TopoNode[]>();
    for (const c of list) {
      const z = zoneOf.get(c.id) ?? "";
      groups.set(z, [...(groups.get(z) ?? []), c]);
    }
    return [...groups.values()].map((g) => block(g, maxCols));
  };
  const widthOf = (bl: Block[]) => bl.reduce((x, y) => x + y.w, 0) + Math.max(0, bl.length - 1) * ZG;
  function measure(t: TopoNode): { w: number; h: number } {
    const ch = kids.get(t.id) ?? [];
    const blocks = blocksOf(ch, 4);
    const bw = widthOf(blocks);
    const w = Math.max(NW, bw);
    const h = NH + (ch.length ? VG + Math.max(0, ...blocks.map((y) => y.h)) : 0);
    size.set(t.id, { w, h, blocks, bw });
    return { w, h };
  }
  const auto = new Map<string, { x: number; y: number }>();
  const putBlocks = (blocks: Block[], x: number, y: number) => {
    let cx = x;
    for (const bl of blocks) {
      bl.leaves.forEach((c, i) => {
        auto.set(c.id, { x: cx + (i % bl.cols) * (NW + HG), y: y + Math.floor(i / bl.cols) * (NH + HG) });
      });
      let bx = cx + bl.gw + (bl.gw ? HG * 2 : 0);
      for (const br of bl.branches) {
        put(br, bx, y);
        bx += size.get(br.id)!.w + HG * 2;
      }
      cx += bl.w + ZG;
    }
  };
  function put(t: TopoNode, x: number, y: number) {
    const s = size.get(t.id)!;
    auto.set(t.id, { x: x + (s.w - NW) / 2, y });
    putBlocks(s.blocks, x + (s.w - s.bw) / 2, y + NH + VG);
  }
  // Trees side by side; lone nodes (hosts not seen, this computer) in a small grid after them.
  let x = 0;
  let bottom = 0;
  for (const r of roots.filter((r) => kids.has(r.id))) {
    const m = measure(r);
    put(r, x, 0);
    x += m.w + ROOT_GAP;
    bottom = Math.max(bottom, m.h);
  }
  const lone = roots.filter((r) => !kids.has(r.id));
  if (lone.length) putBlocks([block(lone, Math.max(2, Math.floor((x || NW * 4) / (NW + HG))))], 0, bottom ? bottom + VG * 2 : 0);

  // Saved positions, and the offset they carry down to unplaced children.
  const done = new Set<string>();
  const final = (t: TopoNode): { dx: number; dy: number } => {
    const a = auto.get(t.id) ?? { x: 0, y: 0 };
    if (done.has(t.id)) return { dx: t.x - a.x, dy: t.y - a.y };
    done.add(t.id);
    const e = saved.get(t.key);
    if (e && e.x !== null && e.y !== null && e.x !== undefined && e.y !== undefined) {
      t.x = e.x;
      t.y = e.y;
      t.pinned = true;
    } else {
      const p = t.parent ? byId.get(t.parent) : undefined;
      const d = p ? final(p) : { dx: 0, dy: 0 };
      t.x = a.x + d.dx;
      t.y = a.y + d.dy;
    }
    return { dx: t.x - a.x, dy: t.y - a.y };
  };
  for (const t of nodes) final(t);

  // A node the user hasn't placed never lands on top of one they have.
  const hit = (a: TopoNode, b: TopoNode) => Math.abs(a.x - b.x) < NW + 4 && Math.abs(a.y - b.y) < NH + 4;
  const fixed = nodes.filter((t) => t.pinned);
  if (!fixed.length) return;
  const taken = [...fixed];
  for (const t of nodes) {
    if (t.pinned) continue;
    for (let i = 0; i < 400 && taken.some((o) => hit(t, o)); i++) t.y += NH + HG;
    taken.push(t);
  }
}

/** Bounding box of a set of nodes, padded. */
export function bounds(ns: readonly Pick<TopoNode, "x" | "y">[], pad = 0): { x: number; y: number; w: number; h: number } {
  if (!ns.length) return { x: 0, y: 0, w: NW, h: NH };
  let x1 = Infinity;
  let y1 = Infinity;
  let x2 = -Infinity;
  let y2 = -Infinity;
  for (const n of ns) {
    x1 = Math.min(x1, n.x);
    y1 = Math.min(y1, n.y);
    x2 = Math.max(x2, n.x + NW);
    y2 = Math.max(y2, n.y + NH);
  }
  return { x: x1 - pad, y: y1 - pad, w: x2 - x1 + pad * 2, h: y2 - y1 + pad * 2 };
}

/** The layout entry to save for a node, keeping what's already saved. */
export function entryFor(t: TopoNode, layout: ReadonlyMap<string, LayoutEntry>, patch: Partial<LayoutEntry>): LayoutEntry {
  const e = layout.get(t.key);
  return { key: t.key, x: e?.x ?? null, y: e?.y ?? null, parent: e?.parent ?? null, expected: e?.expected ?? false, ...patch };
}

/**
 * "Save as expected": every node now on the map, where it is now, with the
 * user's parents kept. Nodes no longer seen drop out of the picture.
 */
export function expectedEntries(topo: Topology, layout: ReadonlyMap<string, LayoutEntry>): LayoutEntry[] {
  return topo.nodes
    .filter((t) => !t.ghost)
    .map((t) => ({ key: t.key, x: Math.round(t.x), y: Math.round(t.y), parent: layout.get(t.key)?.parent ?? null, expected: true }));
}
