/**
 * Browser-only stand-in for host metrics: plausible, slowly drifting samples
 * for every mock host, plus generated history. Used by `pnpm dev`, tests and
 * screenshots; never talks to a host.
 */
import type {
  MetricsEvent_Serialize as MetricsEvent,
  Point_Serialize as Point,
  Sample_Serialize as Sample,
} from "./bindings";
import { mockBackend } from "./mock";

const GiB = 1024 ** 3;
let timer: ReturnType<typeof setInterval> | null = null;
let listener: ((e: MetricsEvent) => void) | null = null;
let visible = new Set<number>();
let tick = 0;

const wave = (seed: number, t: number, lo: number, hi: number) =>
  lo + ((Math.sin(t / 7 + seed) + Math.sin(t / 3.1 + seed * 2) * 0.4 + 1.4) / 2.8) * (hi - lo);

/** Deterministic per-host profile. */
function profile(id: number) {
  const cores = [4, 8, 16, 4][id % 4];
  return { cores, mem: [8, 32, 64, 4][id % 4] * GiB, pi: id % 4 === 3 };
}

export function mockSample(id: number, t: number): Sample {
  const p = profile(id);
  const cores = Array.from({ length: p.cores }, (_, i) => Math.round(wave(id * 3 + i, t + i, 2, i === 0 ? 85 : 45) * 10) / 10);
  const cpu = cores.reduce((a, b) => a + b, 0) / cores.length;
  const memUsed = p.mem * wave(id + 9, t / 5, 0.35, 0.62);
  const rx = wave(id + 1, t, 20e3, 9e6);
  return {
    ts: Date.now(),
    hostname: p.pi ? "pi" : `node${id}`,
    kernel: p.pi ? "Linux 6.6.51+rpt-rpi-v8" : "Linux 6.8.0-49-generic",
    uptimeSecs: 86400 * 12 + 3600 * 5 + id * 777 + t,
    cpu,
    cores,
    load: [cpu / 25, cpu / 28, cpu / 30],
    memTotal: p.mem,
    memUsed,
    swapTotal: 4 * GiB,
    swapUsed: 0.4 * GiB,
    mounts: [
      { device: "/dev/nvme0n1p2", mount: "/", fstype: "ext4", total: 480 * GiB, used: 131 * GiB, avail: 325 * GiB },
      { device: "/dev/sda1", mount: "/mnt/media", fstype: "xfs", total: 7450 * GiB, used: 6630 * GiB, avail: 820 * GiB },
      { device: "/dev/nvme0n1p1", mount: "/boot/efi", fstype: "vfat", total: 0.5 * GiB, used: 0.01 * GiB, avail: 0.49 * GiB },
    ],
    net: [
      { name: "eth0", rx, tx: rx / 6 },
      { name: "docker0", rx: rx / 20, tx: rx / 25 },
    ],
    netRx: rx,
    netTx: rx / 6,
    disks: [
      { name: "nvme0n1", rx: wave(id + 4, t, 0, 40e6), tx: wave(id + 5, t, 0, 12e6) },
      { name: "sda", rx: wave(id + 6, t, 0, 90e6), tx: 0 },
    ],
    diskRead: wave(id + 4, t, 0, 40e6) + wave(id + 6, t, 0, 90e6),
    diskWrite: wave(id + 5, t, 0, 12e6),
    temps: p.pi
      ? [{ chip: "cpu_thermal", label: "", celsius: Math.round(wave(id, t, 46, 58) * 10) / 10 }]
      : [
          { chip: "coretemp", label: "Package id 0", celsius: Math.round(wave(id, t, 41, 67) * 10) / 10 },
          { chip: "nvme", label: "Composite", celsius: 38.9 },
        ],
    cpuTemp: Math.round(wave(id, t, p.pi ? 46 : 41, p.pi ? 58 : 67) * 10) / 10,
    gpus:
      id % 4 === 1
        ? [{ name: "NVIDIA GeForce RTX 3060", util: Math.round(wave(id + 7, t, 0, 92)), memUsed: 3.1 * GiB, memTotal: 12 * GiB, temp: Math.round(wave(id + 7, t, 38, 71)), power: Math.round(wave(id + 7, t, 14, 160)) }]
        : [],
  };
}

async function emit() {
  if (!listener) return;
  tick++;
  const hosts = await mockBackend.hostsList();
  for (const h of hosts) {
    if (h.proxyJump) {
      if (tick === 1) listener({ kind: "state", hostId: h.id, state: "needsLogin", message: "password required", retrySecs: null });
      continue;
    }
    if (!visible.has(h.id) && tick % 10 !== 1) continue;
    if (tick === 1) listener({ kind: "state", hostId: h.id, state: "ok", message: null, retrySecs: null });
    listener({ kind: "sample", hostId: h.id, sample: mockSample(h.id, tick) });
  }
}

export const mockMetrics = {
  subscribe(on: (e: MetricsEvent) => void): Promise<null> {
    listener = on;
    tick = 0;
    if (timer) clearInterval(timer);
    void emit();
    timer = setInterval(() => void emit(), 2000);
    return Promise.resolve(null);
  },
  sync: (): Promise<null> => Promise.resolve(null),
  setVisible(ids: number[]): Promise<null> {
    visible = new Set(ids);
    return Promise.resolve(null);
  },
  history(hostId: number, from: number, to: number): Promise<Point[]> {
    const step = to - from <= 2 * 86400 ? 60 : to - from <= 31 * 86400 ? 900 : 3600;
    const out: Point[] = [];
    for (let ts = Math.ceil(from / step) * step; ts <= to; ts += step) {
      const t = ts / 120;
      out.push({
        ts,
        cpu: wave(hostId, t, 3, 55),
        mem: wave(hostId + 9, t / 5, 35, 62),
        swap: 10,
        load1: wave(hostId, t, 0.1, 2.5),
        // Fills slowly (about 0.35 % a day) so the forecast has something to say.
        disk: 84 - ((to - ts) / 86400) * 0.35,
        netRx: wave(hostId + 1, t, 20e3, 9e6),
        netTx: wave(hostId + 1, t, 20e3, 9e6) / 6,
        diskRead: wave(hostId + 4, t, 0, 40e6),
        diskWrite: wave(hostId + 5, t, 0, 12e6),
        temp: wave(hostId, t, 41, 67),
        gpu: hostId % 4 === 1 ? wave(hostId + 7, t, 0, 80) : null,
        gpuMem: hostId % 4 === 1 ? wave(hostId + 8, t / 4, 20, 45) : null,
        gpuTemp: hostId % 4 === 1 ? wave(hostId + 7, t, 38, 71) : null,
        gpuPower: hostId % 4 === 1 ? wave(hostId + 7, t, 14, 160) : null,
      });
    }
    return Promise.resolve(out);
  },
};
