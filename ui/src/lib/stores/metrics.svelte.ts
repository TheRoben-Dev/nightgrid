import { api, type MetricsEvent, type PollState, type Sample } from "../api";

/** Points kept per host for sparklines (≈5 min at 2 s). */
export const SPARK_POINTS = 150;

export interface HostMetrics {
  state: PollState;
  message: string | null;
  retrySecs: number | null;
  sample: Sample | null;
  /** Recent values for sparklines, oldest first. */
  cpu: number[];
  mem: number[];
  net: number[];
  temp: number[];
}

const empty = (): HostMetrics => ({
  state: "connecting",
  message: null,
  retrySecs: null,
  sample: null,
  cpu: [],
  mem: [],
  net: [],
  temp: [],
});

const push = (arr: number[], v: number | null | undefined): number[] => {
  if (v == null || Number.isNaN(v)) return arr;
  const next = arr.length >= SPARK_POINTS ? arr.slice(arr.length - SPARK_POINTS + 1) : arr.slice();
  next.push(v);
  return next;
};

class MetricsStore {
  byHost = $state<Record<number, HostMetrics>>({});
  /** Host whose monitor panel is open. */
  detailId = $state<number | null>(null);
  private started = false;
  /** Visible-host requests by owner (overview, monitor panel), merged. */
  private owners = new Map<string, number[]>();
  private hidden = false;

  get(id: number): HostMetrics {
    return this.byHost[id] ?? empty();
  }

  start() {
    if (this.started) return;
    this.started = true;
    void api.metricsSubscribe((e) => this.onEvent(e));
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", () => {
        this.hidden = document.visibilityState === "hidden";
        this.pushVisible();
      });
    }
  }

  onEvent(e: MetricsEvent) {
    const cur = this.byHost[e.hostId] ?? empty();
    if (e.kind === "state") {
      this.byHost[e.hostId] = { ...cur, state: e.state, message: e.message, retrySecs: e.retrySecs };
      return;
    }
    const s = e.sample;
    const memPct = s.memTotal ? (s.memUsed / s.memTotal) * 100 : null;
    this.byHost[e.hostId] = {
      ...cur,
      state: "ok",
      message: null,
      retrySecs: null,
      sample: s,
      cpu: push(cur.cpu, s.cpu),
      mem: push(cur.mem, memPct),
      net: push(cur.net, s.netRx + s.netTx),
      temp: push(cur.temp, s.cpuTemp),
    };
  }

  /** Marks hosts as on screen for `owner` (sampled fast); [] releases. */
  setVisible(owner: string, ids: number[]) {
    if (ids.length) this.owners.set(owner, ids);
    else this.owners.delete(owner);
    this.pushVisible();
  }

  private pushVisible() {
    const ids = this.hidden ? [] : [...new Set([...this.owners.values()].flat())];
    void api.metricsSetVisible(ids).catch(() => {});
  }

  /** After hosts or metric settings changed. */
  sync() {
    if (this.started) void api.metricsSync().catch(() => {});
  }

  retry(id: number) {
    void api.metricsRetry(id);
  }
}

export const metrics = new MetricsStore();

/** 1536 → "1.5 KB/s" style rates. */
export function formatRate(bps: number): string {
  const u = ["B/s", "KB/s", "MB/s", "GB/s"];
  let v = bps;
  let i = 0;
  while (v >= 1000 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${u[i]}`;
}

/** 93784 → "1d 2h", 3700 → "1h 1m". */
export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return d ? `${d}d ${h}h` : h ? `${h}h ${m}m` : `${m}m`;
}
