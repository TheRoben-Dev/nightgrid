/**
 * Browser-only stand-in for tunnels: saved forwards whose state and traffic
 * are simulated. Used by `pnpm dev`, tests and screenshots.
 */
import type {
  Tunnel,
  TunnelEvent,
  TunnelInput_Deserialize as TunnelInput,
  TunnelState,
  TunnelView,
} from "./bindings";

let nextId = 1;
let nextTemp = -1;
const states = new Map<number, TunnelState>();
const traffic = new Map<number, { rx: number; tx: number; active: number; total: number }>();
let listener: ((e: TunnelEvent) => void) | null = null;
let timer: ReturnType<typeof setInterval> | null = null;

const bracket = (h: string) => (h.includes(":") ? `[${h}]` : h);
const spec = (t: Tunnel) => {
  const bind = `${bracket(t.bindAddr)}:${t.bindPort}`;
  if (t.kind === "dynamic") return `-D ${bind}`;
  return `${t.kind === "local" ? "-L" : "-R"} ${bind}:${bracket(t.destHost)}:${t.destPort}`;
};
const isPublic = (a: string) => !["127.0.0.1", "localhost", "::1"].includes(a) && !a.startsWith("127.");

let tunnels: Tunnel[] = [
  { id: nextId++, hostId: 1, name: "Admin UI", kind: "local", bindAddr: "127.0.0.1", bindPort: 9443, destHost: "localhost", destPort: 9443, autoStart: true },
  { id: nextId++, hostId: 1, name: "SOCKS", kind: "dynamic", bindAddr: "127.0.0.1", bindPort: 1080, destHost: "", destPort: 0, autoStart: false },
  { id: nextId++, hostId: 2, name: "Dev server back", kind: "remote", bindAddr: "localhost", bindPort: 3000, destHost: "127.0.0.1", destPort: 5173, autoStart: false },
];
const temporary = new Map<number, Tunnel>();

function set(id: number, state: TunnelState) {
  states.set(id, state);
  listener?.({ kind: "state", id, state });
}

function tick() {
  const stats = [...states.entries()]
    .filter(([, s]) => s.kind === "running")
    .map(([id]) => {
      const t = traffic.get(id) ?? { rx: 0, tx: 0, active: 0, total: 0 };
      const burst = Math.random() > 0.5;
      t.rx += burst ? Math.round(Math.random() * 400_000) : 0;
      t.tx += burst ? Math.round(Math.random() * 30_000) : 0;
      t.active = burst ? 1 + Math.floor(Math.random() * 3) : 0;
      t.total += burst ? 1 : 0;
      traffic.set(id, t);
      return { id, stats: { ...t } };
    });
  if (stats.length) listener?.({ kind: "stats", stats });
}

function norm(i: TunnelInput, id: number): Tunnel {
  const kind = i.kind;
  if (kind !== "dynamic" && (!i.destHost?.trim() || !i.destPort)) throw { message: "destPort: 1–65535" };
  return {
    id,
    hostId: i.hostId,
    name: (i.name ?? "").trim(),
    kind,
    bindAddr: i.bindAddr?.trim() || (kind === "remote" ? "localhost" : "127.0.0.1"),
    bindPort: i.bindPort,
    destHost: kind === "dynamic" ? "" : (i.destHost ?? "").trim(),
    destPort: kind === "dynamic" ? 0 : (i.destPort ?? 0),
    autoStart: i.autoStart ?? false,
  };
}

function find(id: number): Tunnel | undefined {
  return id < 0 ? temporary.get(id) : tunnels.find((t) => t.id === id);
}

export const mockTunnels = {
  async subscribe(on: (e: TunnelEvent) => void): Promise<null> {
    listener = on;
    for (const [id, state] of states) on({ kind: "state", id, state });
    for (const t of tunnels) if (t.autoStart && !states.has(t.id)) void mockTunnels.start(t.id);
    if (timer) clearInterval(timer);
    timer = setInterval(tick, 1000);
    return null;
  },
  async list(): Promise<TunnelView[]> {
    const view = (t: Tunnel, temp: boolean): TunnelView => ({
      ...t,
      temporary: temp,
      spec: spec(t),
      public: isPublic(t.bindAddr),
      state: states.get(t.id) ?? { kind: "stopped" },
    });
    return [...tunnels.map((t) => view(t, false)), ...[...temporary.values()].map((t) => view(t, true))];
  },
  async create(i: TunnelInput, startNow: boolean): Promise<Tunnel> {
    const t = norm(i, nextId++);
    tunnels = [...tunnels, t];
    if (startNow) void mockTunnels.start(t.id);
    return t;
  },
  async update(id: number, i: TunnelInput): Promise<Tunnel> {
    const t = norm(i, id);
    tunnels = tunnels.map((x) => (x.id === id ? t : x));
    return t;
  },
  async remove(id: number): Promise<null> {
    states.delete(id);
    tunnels = tunnels.filter((t) => t.id !== id);
    if (temporary.delete(id)) listener?.({ kind: "changed" });
    return null;
  },
  async start(id: number): Promise<null> {
    const t = find(id);
    if (!t) throw { message: "tunnel not found" };
    set(id, { kind: "starting" });
    setTimeout(() => {
      if (t.hostId === 2) set(id, { kind: "failed", message: "server refused to listen on localhost:3000 (check AllowTcpForwarding / GatewayPorts)", needsLogin: false });
      else set(id, { kind: "running", boundPort: t.bindPort || 49152 + Math.floor(Math.random() * 1000) });
    }, 400);
    return null;
  },
  async stop(id: number): Promise<null> {
    set(id, { kind: "stopped" });
    traffic.delete(id);
    if (id < 0) {
      states.delete(id);
      temporary.delete(id);
      listener?.({ kind: "changed" });
    }
    return null;
  },
  async openTemporary(hostId: number, destHost: string, destPort: number): Promise<number> {
    const id = nextTemp--;
    const port = 50000 + Math.floor(Math.random() * 1000);
    temporary.set(id, { id, hostId, name: `${destHost}:${destPort}`, kind: "local", bindAddr: "127.0.0.1", bindPort: 0, destHost, destPort, autoStart: false });
    listener?.({ kind: "changed" });
    set(id, { kind: "running", boundPort: port });
    return port;
  },
};
