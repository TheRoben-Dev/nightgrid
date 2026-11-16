/** Browser-only stand-in for network tests (M10x): canned history, new runs made up. */
import type { NetTest_Serialize as NetTest, NetTestResult_Serialize as NetTestResult } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
let nextId = 10;
let tests: NetTest[] = [];
const mk = (id: number, from: number, to: number, ago: number, up: number, down: number, rtt: number): NetTest => ({
  id,
  fromHost: from,
  toHost: to,
  target: `192.168.1.${10 + to}`,
  at: now() - ago,
  upBps: up,
  downBps: down,
  retransmits: Math.round(up / 1e8),
  rttMs: rtt,
  jitterMs: rtt / 5,
  lossPct: 0,
  error: null,
});
tests = [
  mk(1, 1, 2, 86400 * 6, 941e6, 936e6, 0.41),
  mk(2, 1, 2, 86400 * 4, 938e6, 934e6, 0.39),
  mk(3, 1, 2, 86400 * 2, 940e6, 931e6, 0.44),
  mk(4, 1, 2, 3600 * 3, 94.1e6, 93.8e6, 0.52),
].reverse();

export const mockNetperf = {
  list: async (): Promise<NetTest[]> => tests,
  run: async (fromHost: number, toHost: number, target: string | null, secs: number): Promise<NetTestResult> => {
    if (fromHost === toHost) throw new Error("pick two different hosts");
    await new Promise((r) => setTimeout(r, Math.min(secs, 3) * 200));
    const t: NetTest = { ...mk(nextId++, fromHost, toHost, 0, 930e6 + Math.random() * 10e6, 920e6 + Math.random() * 10e6, 0.4), target: target || `192.168.1.${10 + toHost}` };
    tests = [t, ...tests];
    return { test: t, finding: null };
  },
  clear: async (fromHost: number, toHost: number): Promise<number> => {
    const n = tests.length;
    tests = tests.filter((t) => !(t.fromHost === fromHost && t.toHost === toHost));
    return n - tests.length;
  },
};
