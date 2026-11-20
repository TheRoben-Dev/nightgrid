/** Browser-only stand-in for Nightgrid Node pairing (M12f). */
import type {
  ConfigSaveReport_Serialize as ConfigSaveReport,
  HubJoinResult,
  HubStepResult_Serialize as HubStepResult,
  NodeConfigFile,
  NodeInstallResult_Serialize as NodeInstallResult,
  NodePart,
  NodeSetup_Serialize as NodeSetup,
  NodeView_Serialize as NodeView,
  Release,
  SyncReport,
} from "./bindings";
import { mockBackend } from "./mock";

const states = new Map<number, NodeView>();
const RELEASE: Release = { tag: "v0.26.0", published: "2026-11-16T10:00:00Z", assets: [] };

function base(hostId: number, i: number): NodeView {
  const ready = i === 0 || i === 2;
  return {
    hostId,
    state: ready ? "ready" : i === 1 ? "notInstalled" : "unknown",
    message: null,
    useNode: true,
    version: ready ? "0.26.0" : null,
    nodeId: ready ? "5f0c2a91" : null,
    proto: ready ? 2 : 0,
    caps: ready ? ["alerts", "containers", "disk", "events", "hub.member", "metrics", "metrics.sync", "node", "peers", "uptime", "summary"] : [],
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

// ---- hub and node setup (D-175) ----

let hubHost: number | null = null;
const members = new Set<number>();
const NODE_CONF = `# Nightgrid Node configuration (root-owned).\n[peers]\nlisten = "tailnet"\nport = 7419\nnetworks = ["100.64.0.0/10", "fd7a:115c:a1e0::/48"]\nserve = ["ping"]\n`;
const HUB_CONF = `# Nightgrid Hub configuration (root-owned).\n[collect]\nlisten = "tailnet"\nport = 7420\nnetworks = ["100.64.0.0/10", "fd7a:115c:a1e0::/48"]\ninterval_secs = 60\n`;
const confs = new Map<string, string>();

function nodeId(hostId: number) {
  return `${hostId.toString(16).padStart(4, "0")}5f0c2a91d3e4b6a7c8d9e0f1a2b3c4`.slice(0, 32);
}

/** Mock members with an odd id are LAN-only (no tailnet), so the hub's collector must also listen on its LAN address. */
const lanOnly = (id: number) => id % 2 === 1;
const hubListen = new Set<string>();
const linked = new Set<number>();

export const mockHub = {
  async setup(hostId: number): Promise<NodeSetup> {
    const v = (await all()).find((x) => x.hostId === hostId);
    const ready = v?.state === "ready";
    const now = Date.now() / 1000;
    const hubId = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
    return {
      hostId,
      nodeId: ready ? nodeId(hostId) : null,
      link: ready
        ? {
            access: members.has(hostId) || hubHost === hostId || linked.has(hostId) ? "ok" : "off",
            fingerprint: "9f".repeat(32),
            listening: `100.64.0.${hostId}:7419`,
            serve: members.has(hostId) ? ["ping", "sync"] : ["ping"],
            peers: members.has(hostId) ? [{ id: hubId, name: "hub", address: lanOnly(hostId) ? "192.168.0.1:7420" : "100.64.0.1:7420", fingerprint: "ab".repeat(32) }] : [],
            readings: members.has(hostId)
              ? [
                  lanOnly(hostId) && !hubListen.has("192.168.0.1:7420")
                    ? { id: hubId, at: now - 30, ok: false, rttMs: null, version: null, hostname: null, error: "connection refused (is the link on there?)" }
                    : { id: hubId, at: now - 30, ok: true, rttMs: 3, version: "0.26.0", hostname: "hub", error: null },
                ]
              : [],
            hubId: members.has(hostId) ? hubId : null,
            canJoin: true,
          }
        : null,
      hub:
        hubHost === hostId
          ? {
              probe: { installed: true, socket: true, writable: true, member: true, unit: "active", collect: "active", version: "0.26.0" },
              overview: {
                hubId,
                fingerprint: "ab".repeat(32),
                listening: "100.64.0.1:7420",
                listenAll: ["100.64.0.1:7420", ...hubListen],
                collector: "ok",
                storeBytes: 9_437_184,
                members: [...members].map((m) => ({ id: nodeId(m), name: `host ${m}`, state: "ok", lastContact: now - 40, version: "0.26.0", hostname: `host${m}`, alerts: 0, error: null })),
              },
              events: [{ at: now - 7200, level: "ok", title: "host 2 back online" }],
              error: null,
            }
          : null,
      units: ready ? [{ unit: "nightgrid-node.service", state: "active" }, ...(hubHost === hostId ? [{ unit: "nightgrid-hub.service", state: "active" }, { unit: "nightgrid-hub-collect.service", state: "active" }] : [])] : [],
      doctor: ready ? [{ check: "config", level: "ok", detail: "/etc/nightgrid-node/config.toml is valid" }, { check: "service", level: "ok", detail: "active" }] : [],
      notes: [],
    };
  },
  async configRead(hostId: number, part: NodePart): Promise<NodeConfigFile> {
    const path = part === "hub" ? "/etc/nightgrid-hub/config.toml" : "/etc/nightgrid-node/config.toml";
    return { path, text: confs.get(`${hostId}:${part}`) ?? (part === "hub" ? HUB_CONF : NODE_CONF), exists: true };
  },
  async configSave(hostId: number, part: NodePart, text: string): Promise<ConfigSaveReport> {
    confs.set(`${hostId}:${part}`, text);
    return { ok: true, steps: ["saved the config (the old one is config.toml.bak)", "restarted what was running"], warnings: [], failure: null };
  },
  async logs(part: NodePart): Promise<string> {
    return `2026-11-18T10:00:00+0000 host ${part === "hub" ? "nightgrid-hub" : "nightgrid-node"}[812]: started\n2026-11-18T10:00:02+0000 host ${part === "hub" ? "nightgrid-hub" : "nightgrid-node"}[812]: listening\n`;
  },
  async install(hostId: number): Promise<HubStepResult> {
    hubHost = hostId;
    members.add(hostId);
    return {
      report: { action: "install", version: "0.26.0", steps: ["installed nightgrid-hub", "collector started"], users: ["alice"], warnings: [], hubId: "a1b2", fingerprint: "ab".repeat(32), listening: "100.64.0.1:7420" },
      setup: await mockHub.setup(hostId),
    };
  },
  async uninstall(hostId: number): Promise<HubStepResult> {
    hubHost = null;
    members.clear();
    return { report: { action: "uninstall", version: "0.26.0", steps: ["removed nightgrid-hub"], users: [], warnings: [], hubId: null, fingerprint: null, listening: null }, setup: await mockHub.setup(hostId) };
  },
  async join(memberId: number, hub: number): Promise<HubJoinResult> {
    members.add(memberId);
    return { steps: ["turned on the node-to-node link", `paired with the hub on host ${hub}`, "added to the hub's members"], warnings: [], hubNetworks: lanOnly(memberId) ? [`192.168.0.${memberId}/32`] : [], hubListen: lanOnly(memberId) ? ["192.168.0.1:7420"] : [] };
  },
  async leave(memberId: number): Promise<HubJoinResult> {
    members.delete(memberId);
    return { steps: ["removed from the hub's members", "unpaired from the hub"], warnings: [], hubNetworks: [], hubListen: [] };
  },
  async allowNetworks(networks: string[], listen: string[]): Promise<ConfigSaveReport> {
    for (const l of listen) hubListen.add(l);
    return { ok: true, steps: [...listen.map((l) => `the collector now also answers on ${l}`), `the collector may reach ${networks.join(", ")}`, "ran allow collect again for its networks"], warnings: [], failure: null };
  },
  async linkEnable(hostId: number): Promise<ConfigSaveReport> {
    linked.add(hostId);
    return { ok: true, steps: ["turned on the node-to-node link (allow peers)", `the link is on, listening on 100.64.0.${hostId}:7419`], warnings: [], failure: null };
  },
  async repairLink(memberId: number): Promise<ConfigSaveReport> {
    hubListen.add("192.168.0.1:7420");
    return { ok: true, steps: ["updated the hub from 0.26.0 to 0.32.0", "the collector now also answers on 192.168.0.1:7420", `the collector may reach 192.168.0.${memberId}/32`], warnings: [], failure: null };
  },
};
