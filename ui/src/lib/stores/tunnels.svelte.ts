import { api, errorText, type TunnelEvent, type TunnelInput, type TunnelKind, type TunnelState, type TunnelTraffic, type TunnelView } from "../api";
import { openExternal } from "../clipboard";

/** Byte rate history kept per tunnel (seconds). */
const RATE_POINTS = 60;

export const KIND_LABEL: Record<TunnelKind, string> = { local: "LOCAL -L", remote: "REMOTE -R", dynamic: "SOCKS -D" };

export const emptyTunnel = (hostId: number, kind: TunnelKind = "local"): TunnelInput => ({
  hostId,
  name: "",
  kind,
  bindAddr: "",
  bindPort: kind === "dynamic" ? 1080 : 8080,
  destHost: kind === "remote" ? "127.0.0.1" : "localhost",
  destPort: 80,
  autoStart: false,
});

export const toTunnelInput = (t: TunnelView): TunnelInput => ({
  hostId: t.hostId,
  name: t.name,
  kind: t.kind,
  bindAddr: t.bindAddr,
  bindPort: t.bindPort,
  destHost: t.destHost,
  destPort: t.destPort,
  autoStart: t.autoStart,
});

export const isActive = (s: TunnelState | undefined): boolean =>
  !!s && (s.kind === "running" || s.kind === "starting" || s.kind === "retrying");

/** Where to point a browser/client at, for a running local or SOCKS tunnel. */
export function localEndpoint(t: TunnelView, s: TunnelState | undefined): string | null {
  if (s?.kind !== "running" || t.kind === "remote") return null;
  const addr = t.bindAddr === "0.0.0.0" || t.bindAddr === "::" ? "127.0.0.1" : t.bindAddr;
  return `${addr.includes(":") ? `[${addr}]` : addr}:${s.boundPort}`;
}

/** Splits a service URL into what to forward and how to rebuild it locally. */
export function tunnelTarget(url: string): { host: string; port: number; rebuild: (localPort: number) => string } | null {
  try {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- parsed once, never mutated
    const u = new URL(url);
    if (u.protocol !== "http:" && u.protocol !== "https:") return null;
    const port = u.port ? Number(u.port) : u.protocol === "https:" ? 443 : 80;
    const host = u.hostname.replace(/^\[|\]$/g, "");
    return { host, port, rebuild: (p) => `${u.protocol}//127.0.0.1:${p}${u.pathname}${u.search}${u.hash}` };
  } catch {
    return null;
  }
}

class TunnelsStore {
  tunnels = $state<TunnelView[]>([]);
  states = $state<Record<number, TunnelState>>({});
  traffic = $state<Record<number, TunnelTraffic>>({});
  /** Bytes/s (rx+tx) per tunnel, newest last. */
  rates = $state<Record<number, number[]>>({});
  loaded = $state(false);
  error = $state<string | null>(null);
  /** Editor: null closed, 0 new, else the tunnel id. */
  editing = $state<number | null>(null);
  /** Pre-filled input for a new tunnel (host picked etc.). */
  draft = $state<TunnelInput | null>(null);
  busy = $state<number[]>([]);

  private started = false;

  start() {
    if (this.started) return;
    this.started = true;
    void this.refresh();
    void api.tunnelsSubscribe((e) => this.onEvent(e)).catch((e) => (this.error = errorText(e)));
  }

  async refresh() {
    try {
      const list = await api.tunnelsList();
      this.tunnels = list;
      const states = { ...this.states };
      for (const t of list) if (!(t.id in states)) states[t.id] = t.state;
      this.states = states;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loaded = true;
    }
  }

  onEvent(e: TunnelEvent) {
    if (e.kind === "changed") {
      void this.refresh();
    } else if (e.kind === "state") {
      this.states = { ...this.states, [e.id]: e.state };
      if (e.state.kind === "stopped") {
        const { [e.id]: _r, ...rates } = this.rates;
        this.rates = rates;
      }
    } else {
      const traffic = { ...this.traffic };
      const rates = { ...this.rates };
      for (const { id, stats } of e.stats) {
        const prev = traffic[id];
        const delta = prev ? Math.max(0, stats.rx + stats.tx - prev.rx - prev.tx) : 0;
        rates[id] = [...(rates[id] ?? []), delta].slice(-RATE_POINTS);
        traffic[id] = stats;
      }
      this.traffic = traffic;
      this.rates = rates;
    }
  }

  stateOf(id: number): TunnelState {
    return this.states[id] ?? { kind: "stopped" };
  }

  byId(id: number): TunnelView | undefined {
    return this.tunnels.find((t) => t.id === id);
  }

  get activeCount(): number {
    return Object.values(this.states).filter((s) => s.kind === "running").length;
  }

  private async withBusy(id: number, f: () => Promise<unknown>) {
    this.busy = [...this.busy, id];
    try {
      await f();
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = this.busy.filter((x) => x !== id);
    }
  }

  toggle(id: number) {
    return this.withBusy(id, () => (isActive(this.states[id]) ? api.tunnelStop(id) : api.tunnelStart(id)));
  }

  async save(id: number, input: TunnelInput, startNow: boolean) {
    if (id) await api.tunnelUpdate(id, input);
    else await api.tunnelCreate(input, startNow);
    await this.refresh();
  }

  async remove(id: number) {
    await api.tunnelDelete(id);
    const { [id]: _s, ...states } = this.states;
    this.states = states;
    await this.refresh();
  }

  openNew(hostId: number, kind: TunnelKind = "local") {
    this.draft = emptyTunnel(hostId, kind);
    this.editing = 0;
  }

  /**
   * "Open through tunnel": forwards the URL's host:port as seen from `hostId`
   * to a free local port and opens the rewritten URL in the browser.
   */
  async openThrough(hostId: number, url: string): Promise<string> {
    const target = tunnelTarget(url);
    if (!target) throw new Error("Only http(s) URLs can be opened through a tunnel.");
    const port = await api.tunnelOpenTemporary(hostId, target.host, target.port);
    const local = target.rebuild(port);
    await openExternal(local);
    return local;
  }
}

export const tunnels = new TunnelsStore();
