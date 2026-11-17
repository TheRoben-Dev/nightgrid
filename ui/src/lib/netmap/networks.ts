/**
 * Saved networks on the map (D-153). The scan on show is the main graph;
 * other saved networks can be overlaid from their last scan. Each overlaid
 * network keeps its own router and devices (ids prefixed `n<id>:`), while
 * the things networks share are one node each: the Internet (every router's
 * uplink, which is how two LANs with no route between them reach each
 * other), the tailnet (which links its devices across networks directly),
 * and the saved hosts and this computer.
 *
 * Pure, so it is unit-tested without a backend.
 */
import type { DeviceLabel, Discovery, SecurityReport } from "../api";
import { buildGraph, labelKey, type HostLike, type MapLink, type MapNode, type NetGraph } from "./graph";

export interface SavedNetLike {
  id: number;
  name: string;
  subnet: string | null;
  /** A host id as text, or `local`. */
  origin: string;
}

export interface Overlay {
  net: SavedNetLike;
  disc: Discovery;
}

/** Ids shared by every network: the Internet, the tailnet, hosts, this computer. */
const shared = (id: string) => id === "net" || id === "ts" || id === "local" || /^h\d+$/.test(id);

/** The scanner of a kept scan, as `buildGraph` takes it. This computer moves between networks, so a kept scan from it isn't pinned to where it is now. */
export function overlayScanner(origin: string): number | null {
  return /^\d+$/.test(origin) ? Number(origin) : null;
}

export function overlayGraph(
  base: NetGraph,
  baseDisc: Discovery | null,
  hosts: HostLike[],
  audits: Record<number, SecurityReport | string>,
  overlays: readonly Overlay[],
  labels: ReadonlyMap<string, DeviceLabel> = new Map(),
): NetGraph {
  const baseNet = baseDisc?.network?.id ?? null;
  const list = overlays.filter((o) => o.net.id !== baseNet);
  if (!list.length) return base;
  const nodes: MapNode[] = base.nodes.map((n) => ({ ...n, exposures: [...n.exposures] }));
  const links: MapLink[] = base.links.map((l) => ({ ...l }));
  const byId = new Map(nodes.map((n) => [n.id, n]));
  // One physical device, one node: what the shown scan has, an overlay doesn't add again.
  // A switch the scanner's LLDP named, or a router on a traceroute, has no device but a MAC or an address.
  const keyOf = (n: MapNode) =>
    n.device ? labelKey(n.device.mac, n.device.ip) : n.lldp?.mac ? labelKey(n.lldp.mac, "") : n.kind === "device" && n.ip ? labelKey(null, n.ip) : null;
  const deviceAt = new Map<string, string>();
  for (const n of nodes) {
    const k = keyOf(n);
    if (k) deviceAt.set(k, n.id);
  }
  const baseAddrs = new Set(baseDisc?.ifaces.map((i) => i.addr) ?? []);
  // Hosts the shown scan placed stay where it put them.
  const placed = new Set(baseDisc ? nodes.filter((n) => n.kind === "host" && !n.unseen).map((n) => n.id) : []);
  const link = (from: string, to: string, kind: MapLink["kind"]) => {
    if (from === to || links.some((l) => l.from === from && l.to === to)) return;
    links.push({ from, to, kind });
  };
  const nets = [];
  for (const o of list) {
    const prefix = `n${o.net.id}:`;
    nets.push({ id: o.net.id, name: o.net.name, subnet: o.net.subnet, prefix });
    const g = buildGraph(hosts, audits, o.disc, overlayScanner(o.net.origin), labels);
    const ids = new Map<string, string>();
    for (const n of g.nodes) {
      if (shared(n.id)) {
        ids.set(n.id, n.id);
        const have = byId.get(n.id);
        if (!have) {
          const add = { ...n, exposures: [...n.exposures], network: n.kind === "host" ? undefined : o.net.id };
          nodes.push(add);
          byId.set(n.id, add);
        } else if (n.kind === "host" && !n.unseen && !placed.has(n.id) && (have.unseen || !baseDisc)) {
          have.unseen = false;
          have.ip ??= n.ip;
          have.network = o.net.id;
        } else if (n.kind === "internet") {
          have.exposures.push(...n.exposures);
        }
        continue;
      }
      const k = keyOf(n);
      const same = k ? deviceAt.get(k) : undefined;
      if (same || (n.ip && baseAddrs.has(n.ip))) {
        // The shown scan's own device (or its scanner): links land on it.
        ids.set(n.id, same ?? "local");
        continue;
      }
      const id = prefix + n.id;
      ids.set(n.id, id);
      const add: MapNode = { ...n, id, exposures: [...n.exposures], network: o.net.id };
      if (n.kind === "gateway") add.note = `The router of ${o.net.name}, from its last scan.`;
      nodes.push(add);
      byId.set(id, add);
      if (k) deviceAt.set(k, id);
    }
    for (const l of g.links) {
      const from = ids.get(l.from);
      const to = ids.get(l.to);
      if (!from || !to) continue;
      if (placed.has(to) && from.startsWith(prefix)) continue;
      link(from, to, l.kind);
    }
  }
  const internet = byId.get("net");
  const routers = nodes.filter((n) => n.kind === "gateway");
  if (internet && routers.length > 1) {
    internet.sub = `links ${routers.length} networks${internet.exposures.length ? ` · ${internet.exposures.length} ways in` : ""}`;
    internet.note = `Each network reaches the internet through its own router, and that's the only path between networks with no VPN or route between them: devices on one LAN can't see the other's local addresses.`;
  }
  const ts = byId.get("ts");
  if (ts && list.length) ts.note = "Devices on the tailnet reach each other directly, whichever network they sit on.";
  return { nodes, links, cols: base.cols, nets };
}
