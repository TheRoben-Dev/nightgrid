import { api, errorText, type CheckPoint, type Service, type ServiceEvent, type ServiceInput, type ServiceStatus, type TlsInfo } from "../api";
import { hosts } from "./hosts.svelte";

const DAY = 86_400;
/** Points kept per service for the latency trend and 24 h uptime. */
const HISTORY_CAP = 1500;
const VIEW_KEY = "nightgrid.services.v1";
/** Certificates expiring sooner than this are flagged. */
export const CERT_WARN_DAYS = 14;

export type GroupBy = "group" | "host";
export type Health = "up" | "down" | "warn" | "pending" | "off";

export interface ServiceGroup {
  key: string;
  label: string;
  services: Service[];
  up: number;
  checked: number;
}

export const nowSecs = () => Math.floor(Date.now() / 1000);

export function certDays(tls: TlsInfo | null | undefined, now = nowSecs()): number | null {
  return tls?.expires != null ? Math.floor((tls.expires - now) / DAY) : null;
}

/** Up/down plus "warn" for up-but-attention (untrusted cert allowed, cert expiring soon). */
export function healthOf(svc: Service, st: ServiceStatus | undefined, now = nowSecs()): Health {
  if (!svc.checkEnabled) return "off";
  if (!st) return "pending";
  if (!st.result.up) return "down";
  const tls = st.result.tls;
  const days = certDays(tls, now);
  if (tls && (!tls.trusted || (days !== null && days < CERT_WARN_DAYS))) return "warn";
  return "up";
}

/** Share of up checks, 0–100, or null with no data. */
export function uptime(points: CheckPoint[]): number | null {
  if (!points.length) return null;
  return (points.filter((p) => p.up).length / points.length) * 100;
}

export function hostOfUrl(url: string): string {
  try {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- parsed once, never mutated
    const u = new URL(url);
    return u.port ? `${u.hostname}:${u.port}` : u.hostname;
  } catch {
    return url;
  }
}

export const emptyInput = (): ServiceInput => ({
  name: "",
  url: "",
  hostId: null,
  group: "",
  description: "",
  icon: null,
  checkEnabled: true,
  expectStatus: "200-399",
  allowUntrusted: false,
});

export const toInput = (s: Service): ServiceInput => ({
  name: s.name,
  url: s.url,
  hostId: s.hostId,
  group: s.group,
  description: s.description,
  icon: s.icon,
  checkEnabled: s.checkEnabled,
  expectStatus: s.expectStatus,
  allowUntrusted: s.allowUntrusted,
});

function loadView(): { groupBy: GroupBy } {
  try {
    const v = JSON.parse(localStorage.getItem(VIEW_KEY) ?? "{}");
    return { groupBy: v.groupBy === "host" ? "host" : "group" };
  } catch {
    return { groupBy: "group" };
  }
}

class ServicesStore {
  services = $state<Service[]>([]);
  status = $state<Record<number, ServiceStatus>>({});
  history = $state<Record<number, CheckPoint[]>>({});
  loaded = $state(false);
  error = $state<string | null>(null);
  filter = $state("");
  groupBy = $state<GroupBy>(loadView().groupBy);
  /** Editor: null closed, 0 new, else the service id. */
  editing = $state<number | null>(null);
  /** Fields to start a new service with (from the network map). */
  draft = $state<Partial<ServiceInput> | null>(null);
  importOpen = $state(false);
  /** Services with a manual check in flight. */
  checking = $state<number[]>([]);

  private started = false;

  start() {
    if (this.started) return;
    this.started = true;
    void this.refresh().then(() => this.loadHistory());
    void api.servicesSubscribe((e) => this.onEvent(e)).catch((e) => (this.error = errorText(e)));
  }

  async refresh() {
    try {
      this.services = await api.servicesList();
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loaded = true;
    }
  }

  async loadHistory() {
    const from = nowSecs() - DAY;
    const entries = await Promise.all(
      this.services.map(async (s) => [s.id, await api.serviceHistory(s.id, from).catch(() => [])] as const),
    );
    const next: Record<number, CheckPoint[]> = {};
    for (const [id, pts] of entries) next[id] = pts;
    this.history = next;
  }

  onEvent(e: ServiceEvent) {
    if (e.kind === "cleared") {
      const { [e.serviceId]: _, ...rest } = this.status;
      this.status = rest;
      return;
    }
    const st: ServiceStatus = { serviceId: e.serviceId, ts: e.ts, result: e.result };
    this.status = { ...this.status, [e.serviceId]: st };
    const pts = this.history[e.serviceId] ?? [];
    if (pts.at(-1)?.ts === e.ts) return;
    const cutoff = e.ts - DAY;
    const next = [...pts.filter((p) => p.ts >= cutoff), { ts: e.ts, up: e.result.up, status: e.result.status, latencyMs: e.result.latencyMs }];
    this.history = { ...this.history, [e.serviceId]: next.slice(-HISTORY_CAP) };
  }

  setGroupBy(g: GroupBy) {
    this.groupBy = g;
    try {
      localStorage.setItem(VIEW_KEY, JSON.stringify({ groupBy: g }));
    } catch {
      /* storage unavailable */
    }
  }

  /** Opens the editor for a new service with some fields filled in. */
  addFrom(d: Partial<ServiceInput>) {
    this.draft = d;
    this.editing = 0;
  }

  byId(id: number): Service | undefined {
    return this.services.find((s) => s.id === id);
  }

  get visible(): Service[] {
    const q = this.filter.trim().toLowerCase();
    if (!q) return this.services;
    return this.services.filter((s) =>
      [s.name, s.url, s.group, s.description, s.hostId != null ? (hosts.byId(s.hostId)?.name ?? "") : ""].some((v) =>
        v.toLowerCase().includes(q),
      ),
    );
  }

  /** Groups in first-seen order (services come sorted by their board order). */
  get groups(): ServiceGroup[] {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- local, rebuilt on each read
    const by = new Map<string, ServiceGroup>();
    for (const s of this.visible) {
      const key = this.groupBy === "host" ? String(s.hostId ?? "") : s.group;
      const label =
        this.groupBy === "host"
          ? s.hostId != null
            ? (hosts.byId(s.hostId)?.name ?? `host ${s.hostId}`)
            : "no host"
          : s.group || "ungrouped";
      let g = by.get(key);
      if (!g) {
        g = { key, label, services: [], up: 0, checked: 0 };
        by.set(key, g);
      }
      g.services.push(s);
      const st = this.status[s.id];
      if (s.checkEnabled && st) {
        g.checked++;
        if (st.result.up) g.up++;
      }
    }
    // Empty keys ("no host"/"ungrouped") go last.
    return [...by.values()].sort((a, b) => Number(a.key === "") - Number(b.key === ""));
  }

  get groupNames(): string[] {
    return [...new Set(this.services.map((s) => s.group).filter(Boolean))].sort((a, b) => a.localeCompare(b));
  }

  /** Counts for the summary strip. */
  get summary() {
    const now = nowSecs();
    let up = 0;
    let down = 0;
    let warn = 0;
    let off = 0;
    for (const s of this.services) {
      const h = healthOf(s, this.status[s.id], now);
      if (h === "up") up++;
      else if (h === "down") down++;
      else if (h === "warn") warn++;
      else if (h === "off") off++;
    }
    return { up, down, warn, off, total: this.services.length };
  }

  async save(id: number, input: ServiceInput): Promise<Service> {
    const s = id ? await api.serviceUpdate(id, input) : await api.serviceCreate(input);
    await this.refresh();
    return s;
  }

  async remove(id: number) {
    await api.serviceDelete(id);
    const { [id]: _h, ...hist } = this.history;
    this.history = hist;
    await this.refresh();
  }

  async checkNow(id: number) {
    if (this.checking.includes(id)) return;
    this.checking = [...this.checking, id];
    try {
      this.onEvent({ kind: "status", ...(await api.serviceCheck(id)) });
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.checking = this.checking.filter((x) => x !== id);
    }
  }

  async checkAll() {
    await api.servicesCheckAll();
  }
}

export const services = new ServicesStore();
