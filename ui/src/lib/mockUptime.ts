/**
 * Browser-only stand-in for uptime history (M10t): a deterministic 400 days
 * for a few hosts and services, with a handful of outages.
 */
import type { Node, Outage, UptimeDay, UptimeSeries } from "./bindings";

const DAY = 86_400;
const now = () => Math.floor(Date.now() / 1000);
const nodes: Node[] = [
  { kind: "host", id: 1 },
  { kind: "host", id: 2 },
  { kind: "host", id: 3 },
  { kind: "service", id: 1 },
  { kind: "service", id: 2 },
  { kind: "service", id: 3 },
];

/** Small deterministic noise per node and day. */
function rnd(a: number, b: number): number {
  const x = Math.sin(a * 12.9898 + b * 78.233) * 43758.5453;
  return x - Math.floor(x);
}
const seed = (n: Node) => (n.kind === "host" ? n.id : 10 + n.id);

export const mockUptime = {
  async days(days: number, utcOffsetMin: number): Promise<UptimeSeries[]> {
    const off = utcOffsetMin * 60;
    const today = Math.floor((now() + off) / DAY) * DAY - off;
    return nodes.map((node) => {
      const out: UptimeDay[] = [];
      for (let i = days - 1; i >= 0; i--) {
        const day = today - i * DAY;
        const r = rnd(seed(node), Math.floor(day / DAY));
        if (r < 0.06) continue; // app not running that day
        const seen = i === 0 ? Math.floor((now() - day) * 0.9) : 80_000;
        const down = r > 0.97 ? Math.floor(seen * 0.04) : r > 0.9 ? Math.floor(r * 600) : 0;
        const asleep = node.kind === "host" && node.id === 3 ? 6 * 3600 : 0;
        out.push({ day, up: Math.max(0, seen - down - asleep), down, asleep });
      }
      return { node, days: out };
    });
  },
  async outages(node: Node | null, from: number, to: number): Promise<Outage[]> {
    const list: Outage[] = [];
    let id = 1;
    for (const n of nodes) {
      for (let d = Math.floor(to / DAY); d * DAY >= from; d--) {
        const r = rnd(seed(n), d);
        if (r <= 0.9) continue;
        const start = d * DAY + Math.floor(r * 50_000);
        const len = r > 0.97 ? 3200 : Math.floor(r * 600);
        list.push({ id: id++, node: n, start, end: start + len, detail: n.kind === "host" ? "connection timed out" : "HTTP 502" });
      }
    }
    return list.filter((o) => !node || (o.node.kind === node.kind && o.node.id === node.id)).sort((a, b) => b.start - a.start);
  },
};
