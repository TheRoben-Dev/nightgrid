/**
 * Builds the network map (M10i) from what Nightgrid already knows: the host
 * list, port audits (System → Security) and one discovery scan run from a
 * chosen host. Pure, so it is unit-tested without a backend.
 */
import type { Device, DeviceLabel, Discovery, Exposure, Listener, SecurityReport } from "../api";

export interface HostLike {
  id: number;
  name: string;
  hostname: string;
  color: string | null;
}

export type NodeKind = "internet" | "gateway" | "tailnet" | "host" | "device";
export type LinkKind = "internet" | "lan" | "tailnet";

export interface PortSummary {
  /** Reachable from the LAN (bound to every interface or a LAN address). */
  lan: number;
  tailnet: number;
  local: number;
}

export interface MapNode {
  id: string;
  kind: NodeKind;
  label: string;
  sub: string;
  col: number;
  row: number;
  hostId?: number;
  ip?: string;
  device?: Device;
  /** Ways the internet reaches this node. */
  exposures: Exposure[];
  ports?: PortSummary;
  /** Port audit failed (text) for a host. */
  portsError?: string;
  /** Host not seen by the last scan. */
  unseen?: boolean;
  /** The user's label for a device (or the router). */
  named?: DeviceLabel;
}

export interface MapLink {
  from: string;
  to: string;
  kind: LinkKind;
}

export interface NetGraph {
  nodes: MapNode[];
  links: MapLink[];
  cols: number;
}

const IPV4 = /^\d{1,3}(\.\d{1,3}){3}$/;
const isIp = (s: string) => IPV4.test(s) || s.includes(":");
const norm = (s: string) => s.trim().toLowerCase().replace(/\.$/, "");
/** "nas.lan", "nas.local", "NAS" → "nas" (only for names, never addresses). */
const stem = (s: string) => (isIp(s) ? s : norm(s).split(".")[0]);

/** A device's label key: its MAC when the scan saw one, else `ip:<address>` (same as core `device_labels::key_for`). */
export function labelKey(mac: string | null | undefined, ip: string): string {
  const m = mac?.trim();
  return m ? m.toLowerCase().replaceAll("-", ":") : `ip:${ip.trim()}`;
}

/** Short text for a label's kind, shown on map nodes. */
export const KIND_LABEL: Record<string, string> = {
  other: "Other",
  phone: "Phone",
  tablet: "Tablet",
  laptop: "Laptop",
  desktop: "Desktop",
  tv: "TV",
  media: "Media player",
  speaker: "Speaker",
  console: "Game console",
  printer: "Printer",
  camera: "Camera",
  iot: "Smart home",
  nas: "NAS",
  router: "Router",
  ap: "Access point",
  switch: "Switch",
  server: "Server",
  vm: "VM",
};

const LOCAL = /^(127\.|localhost|\[?::1\]?)/i;

/** The address part of an exposure's internal side (`ip:port`, a URL, …). */
export function exposureIp(e: Exposure): string | null {
  const t = e.target.replace(/^[a-z]+:\/\//i, "");
  if (LOCAL.test(t)) return null; // lands on the scanned host itself
  const m = /^\[?([0-9a-f.:]+?)\]?(?::\d+)?(?:\/.*)?$/i.exec(t);
  return m && IPV4.test(m[1]) ? m[1] : null;
}

export function summarizePorts(ls: Listener[]): PortSummary {
  const s: PortSummary = { lan: 0, tailnet: 0, local: 0 };
  const seen = new Set<string>();
  for (const l of ls) {
    const key = `${l.proto}/${l.port}/${l.scope}`;
    if (seen.has(key)) continue;
    seen.add(key);
    if (l.scope === "local") s.local++;
    else if (l.scope === "vpn") s.tailnet++;
    else s.lan++;
  }
  return s;
}

/** Which host a discovered device is, by address or by name. */
export function matchHost(d: Device, hosts: HostLike[]): HostLike | undefined {
  return hosts.find((h) => {
    const hn = norm(h.hostname);
    if (d.ip === hn) return true;
    if (isIp(hn)) return false;
    const names = d.names.map(norm);
    return names.includes(hn) || names.some((n) => stem(n) === stem(hn));
  });
}

export function buildGraph(
  hosts: HostLike[],
  audits: Record<number, SecurityReport | string>,
  disc: Discovery | null,
  /** Where the scan ran: a host id, "local" (this computer), or null. */
  scanFrom: number | "local" | null,
  /** The user's device labels, by `labelKey`. */
  labels: ReadonlyMap<string, DeviceLabel> = new Map(),
): NetGraph {
  const scanHostId = typeof scanFrom === "number" ? scanFrom : null;
  const nodes: MapNode[] = [];
  const links: MapLink[] = [];
  const hostNode = new Map<number, MapNode>();
  const devices = disc?.devices ?? [];
  const exposures = disc?.exposures ?? [];
  const tailnetIface = disc?.ifaces.some((i) => i.kind === "tailnet") ?? false;
  const lanIface = disc?.ifaces.some((i) => i.kind === "private" && !i.addr.includes(":")) ?? false;
  const scanAddrs = new Set(disc?.ifaces.map((i) => i.addr) ?? []);

  // Column 2: every known host.
  hosts.forEach((h, row) => {
    const a = audits[h.id];
    const n: MapNode = {
      id: `h${h.id}`,
      kind: "host",
      label: h.name,
      sub: h.hostname,
      col: 2,
      row,
      hostId: h.id,
      exposures: [],
      ports: a && typeof a !== "string" ? summarizePorts(a.listeners) : undefined,
      portsError: typeof a === "string" ? a : undefined,
      unseen: !!disc,
    };
    hostNode.set(h.id, n);
    nodes.push(n);
  });

  const link = (from: string, to: string, kind: LinkKind) => {
    const old = links.find((l) => l.from === from && l.to === to);
    if (!old) links.push({ from, to, kind });
    else if (kind === "internet") old.kind = kind; // an exposed path wins over the plain LAN link
  };

  // Scanned from this computer: it gets a node of its own under the hosts.
  if (scanFrom === "local" && disc) {
    const addr = disc.ifaces.find((i) => i.kind === "private" && !i.addr.includes(":"))?.addr ?? disc.ifaces[0]?.addr ?? "";
    const n: MapNode = { id: "local", kind: "host", label: "This computer", sub: addr, col: 2, row: hosts.length, ip: addr || undefined, exposures: [] };
    nodes.push(n);
  }

  const gw = disc?.gateway ?? null;
  const hasTailnet = tailnetIface || devices.some((d) => d.seen.includes("tailscale"));
  if (gw) nodes.push({ id: "gw", kind: "gateway", label: "Router", sub: gw, col: 1, row: 0, ip: gw, exposures: [] });
  if (hasTailnet) nodes.push({ id: "ts", kind: "tailnet", label: "Tailnet", sub: "Tailscale peers", col: 1, row: gw ? 1 : 0, exposures: [] });

  // The scanned host sits on the networks its interfaces are on.
  const scanNode = scanFrom === "local" ? nodes.find((n) => n.id === "local") : scanHostId !== null ? hostNode.get(scanHostId) : undefined;
  if (scanNode && disc) {
    scanNode.unseen = false;
    if (gw && lanIface) link("gw", scanNode.id, "lan");
    if (tailnetIface) link("ts", scanNode.id, "tailnet");
  }

  // Devices: known hosts get linked, the rest become unknown devices (column 3).
  let row = 0;
  for (const d of devices) {
    const viaTs = d.seen.includes("tailscale");
    const net = viaTs ? "ts" : gw ? "gw" : null;
    if (d.ip === gw) {
      const g = nodes.find((n) => n.id === "gw");
      if (g) {
        g.device = d;
        g.named = labels.get(labelKey(d.mac, d.ip));
        if (g.named?.name) g.label = g.named.name;
        if (d.names[0]) g.sub = `${gw} · ${d.names[0]}`;
      }
      continue;
    }
    const h = scanAddrs.has(d.ip) ? scanNode : (() => {
      const m = matchHost(d, hosts);
      return m ? hostNode.get(m.id) : undefined;
    })();
    if (h) {
      h.unseen = false;
      h.ip ??= d.ip;
      if (net) link(net, h.id, viaTs ? "tailnet" : "lan");
      continue;
    }
    const named = labels.get(labelKey(d.mac, d.ip)) ?? labels.get(labelKey(null, d.ip));
    const label = named?.name || d.names[0] || d.ip;
    const n: MapNode = {
      id: `d${d.ip}`,
      kind: "device",
      label,
      named,
      sub:
        named && named.kind !== "other"
          ? `${KIND_LABEL[named.kind] ?? named.kind} · ${d.ip}`
          : named?.name || d.names[0]
            ? d.ip
            : d.mac
              ? d.mac
              : seenText(d),
      col: 3,
      row: row++,
      ip: d.ip,
      device: d,
      exposures: [],
    };
    nodes.push(n);
    if (net) link(net, n.id, viaTs ? "tailnet" : "lan");
  }

  // Internet exposures land on the scanned host (local targets) or on the LAN address they forward to.
  if (exposures.length) {
    nodes.push({ id: "net", kind: "internet", label: "Internet", sub: `${exposures.length} way${exposures.length === 1 ? "" : "s"} in`, col: 0, row: 0, exposures: [...exposures] });
    for (const e of exposures) {
      const ip = exposureIp(e);
      const target =
        ip === null || scanAddrs.has(ip)
          ? scanNode
          : nodes.find((n) => (n.kind === "host" || n.kind === "device") && n.ip === ip) ??
            (() => {
              const m = hosts.find((h) => norm(h.hostname) === ip);
              return m ? hostNode.get(m.id) : undefined;
            })();
      if (e.kind === "upnp" && gw) link("net", "gw", "internet");
      if (target) {
        target.exposures.push(e);
        link(e.kind === "upnp" && gw ? "gw" : "net", target.id, "internet");
      }
    }
  }

  return { nodes, links, cols: 4 };
}

export function seenText(d: Device): string {
  const by: Record<string, string> = { neighbour: "neighbour table", ping: "ping", mdns: "mDNS", tailscale: "Tailscale", gateway: "gateway" };
  return d.seen.map((s) => by[s] ?? s).join(", ");
}

/** A friendly label for an mDNS service type. */
export function serviceLabel(t: string): string {
  const m = /^_([a-z0-9-]+)\._(tcp|udp)/i.exec(t);
  const known: Record<string, string> = {
    http: "Web",
    https: "Web (TLS)",
    smb: "SMB share",
    afpovertcp: "AFP share",
    ssh: "SSH",
    sftp: "SFTP",
    airplay: "AirPlay",
    raop: "AirPlay audio",
    googlecast: "Chromecast",
    ipp: "Printer",
    printer: "Printer",
    hap: "HomeKit",
    homekit: "HomeKit",
    spotify: "Spotify Connect",
    octoprint: "OctoPrint",
    workstation: "Workstation",
  };
  return m ? (known[m[1].toLowerCase()] ?? m[1]) : t;
}

/**
 * Device labels as short lines for DRIFT: all of them on the Network view
 * (up to `max`), elsewhere only those the question names (name, tag, address).
 */
export function labelLines(
  labels: readonly DeviceLabel[],
  devices: readonly Device[],
  question: string,
  all: boolean,
  max = 20,
): string[] {
  const words = new Set(question.toLowerCase().match(/[a-z0-9.:'-]+/g) ?? []);
  const ipOf = (l: DeviceLabel) => devices.find((d) => labelKey(d.mac, d.ip) === l.key)?.ip ?? (l.key.startsWith("ip:") ? l.key.slice(3) : null);
  const hit = (l: DeviceLabel) => {
    const ip = ipOf(l);
    return (
      (ip !== null && words.has(ip)) ||
      l.tags.some((t) => words.has(t)) ||
      l.name.toLowerCase().split(/\s+/).some((w) => w.length > 2 && words.has(w)) ||
      words.has(l.key)
    );
  };
  return labels
    .filter((l) => all || hit(l))
    .slice(0, max)
    .map((l) => {
      const ip = ipOf(l);
      const kind = l.kind !== "other" ? ` (${KIND_LABEL[l.kind] ?? l.kind})` : "";
      const where = [ip, l.key.startsWith("ip:") ? null : `MAC ${l.key}`].filter(Boolean).join(", ");
      const tags = l.tags.length ? ` #${l.tags.join(" #")}` : "";
      const note = l.note ? `: ${l.note.replace(/\s+/g, " ").slice(0, 200)}` : "";
      return `${l.name || "Unnamed device"}${kind}${where ? ` [${where}]` : ""}${tags}${note}`;
    });
}
