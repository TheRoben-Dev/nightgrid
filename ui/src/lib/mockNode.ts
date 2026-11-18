/** Browser-only stand-in for Nightgrid Node pairing (M12f). */
import type { NodeInstallResult_Serialize as NodeInstallResult, NodeView_Serialize as NodeView, Release, SyncReport } from "./bindings";
import { mockBackend } from "./mock";

const states = new Map<number, NodeView>();
const RELEASE: Release = { tag: "v0.23.0", published: "2026-11-16T10:00:00Z", assets: [] };

function base(hostId: number, i: number): NodeView {
  const ready = i === 0;
  return {
    hostId,
    state: ready ? "ready" : i === 1 ? "notInstalled" : "unknown",
    message: null,
    useNode: true,
    version: ready ? "0.22.0" : null,
    nodeId: ready ? "5f0c2a91" : null,
    proto: ready ? 2 : 0,
    caps: ready ? ["alerts", "containers", "disk", "events", "metrics", "metrics.sync", "node", "uptime", "summary"] : [],
    eventsLive: ready,
    metricsLive: ready,
    syncing: false,
    syncedAt: ready ? Date.now() / 1000 - 120 : null,
    lastSync: ready ? { points: 1302, diskRows: 4, hours: 21, events: 3, reset: false, pruned: false } : null,
    rulesNote: null,
    user: "alice",
    arch: "x86_64",
    info: null,
  };
}

async function all(): Promise<NodeView[]> {
  const hs = await mockBackend.hostsList();
  return hs.map((h, i) => {
    if (!states.has(h.id)) states.set(h.id, base(h.id, i));
    return states.get(h.id)!;
  });
}

export const mockNode = {
  list: all,
  async check(hostId: number): Promise<NodeView> {
    await all();
    const v = states.get(hostId)!;
    if (v.state === "unknown") v.state = "notInstalled";
    if (v.state !== "ready") return { ...v };
    const now = Date.now() / 1000;
    return {
      ...v,
      info: {
        started: now - 5 * 86400,
        intervalSecs: 10,
        rssBytes: 2_633_728,
        dbBytes: 41_943_040,
        coverage: [
          { res: 60, points: 20160, oldest: now - 14 * 86400, newest: now - 60 },
          { res: 900, points: 8640, oldest: now - 90 * 86400, newest: now - 900 },
          { res: 3600, points: 2160, oldest: now - 90 * 86400, newest: now - 3600 },
        ],
        access: [
          { name: "docker", state: "ok" },
          { name: "journal", state: "denied" },
          { name: "health", state: "ok" },
        ],
        alerts: [],
      },
    };
  },
  async syncNow(hostId: number): Promise<SyncReport> {
    const r: SyncReport = { points: 12, diskRows: 0, hours: 0, events: 1, reset: false, pruned: false };
    const v = states.get(hostId);
    if (v) {
      v.lastSync = r;
      v.syncedAt = Date.now() / 1000;
    }
    return r;
  },
  async setUse(hostId: number, on: boolean): Promise<NodeView> {
    await all();
    const v = states.get(hostId)!;
    v.useNode = on;
    v.state = on ? "unknown" : "off";
    return { ...v };
  },
  release: async (): Promise<Release> => RELEASE,
  async install(hostId: number): Promise<NodeInstallResult> {
    await all();
    const v = { ...base(hostId, 0), version: "0.23.0", lastSync: null, syncedAt: null };
    states.set(hostId, v);
    return { report: { action: "install", version: "0.23.0", steps: ["Created account nightgrid-node", "Installed /usr/local/bin/nightgrid-node", "Started nightgrid-node.service"], users: ["alice"], warnings: [] }, view: v };
  },
  async uninstall(hostId: number): Promise<NodeInstallResult> {
    await all();
    const v = { ...base(hostId, 1) };
    states.set(hostId, v);
    return { report: { action: "uninstall", version: "0.23.0", steps: ["Stopped nightgrid-node.service", "Removed /usr/local/bin/nightgrid-node"], users: [], warnings: [] }, view: v };
  },
};
