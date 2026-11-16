/**
 * Browser-only stand-in for Proxmox (M10r): a two-node cluster like the
 * fixtures in core/src/proxmox/fixtures. Actions start a fake task that
 * finishes after a moment and change the guest's state.
 */
import type {
  PveAction,
  PveGuest_Serialize as PveGuest,
  PveOverview_Serialize as PveOverview,
  PveTask_Serialize as PveTask,
  PveTaskDetail_Serialize as PveTaskDetail,
} from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
const GiB = 1024 ** 3;
const g = (vmid: number, kind: string, node: string, name: string, status: string, extra: Partial<PveGuest> = {}): PveGuest => ({
  vmid,
  kind,
  node,
  name,
  status,
  template: false,
  cpu: status === "running" ? 0.04 : 0,
  maxcpu: 2,
  mem: status === "running" ? 1.5 * GiB : 0,
  maxmem: 4 * GiB,
  disk: 0,
  maxdisk: 32 * GiB,
  uptime: status === "running" ? 86400 * 3 : 0,
  tags: [],
  lock: null,
  hastate: null,
  ...extra,
});

let guests: PveGuest[] = [
  g(100, "qemu", "pve1", "media", "running", { maxcpu: 4, mem: 5 * GiB, maxmem: 8 * GiB, maxdisk: 64 * GiB, tags: ["media", "prod"], uptime: 1203000 }),
  g(101, "qemu", "pve1", "win11", "stopped"),
  g(102, "qemu", "pve1", "homeassistant", "running", { tags: ["home"] }),
  g(200, "lxc", "pve2", "pihole", "running", { maxcpu: 1, mem: 100 * 1024 ** 2, maxmem: 512 * 1024 ** 2, disk: 1.2 * GiB, maxdisk: 8 * GiB, tags: ["dns"], hastate: "started" }),
  g(201, "lxc", "pve2", "gitea", "stopped", { lock: "backup" }),
  g(9000, "qemu", "pve1", "debian12-cloud", "stopped", { template: true }),
];

const UPID = (node: string, type: string, id: string, ts: number) =>
  `UPID:${node}:0000${(ts % 0xffff).toString(16).toUpperCase().padStart(4, "0")}:00ABCDEF:${ts.toString(16).toUpperCase()}:${type}:${id}:root@pam:`;

let tasks: PveTask[] = [
  { upid: UPID("pve1", "qmstart", "100", now() - 3600), node: "pve1", kind: "qmstart", id: "100", user: "root@pam", start: now() - 3600, end: now() - 3590, status: "OK" },
  { upid: UPID("pve2", "vzdump", "", now() - 7200), node: "pve2", kind: "vzdump", id: "", user: "root@pam", start: now() - 7200, end: now() - 6000, status: "job errors" },
  { upid: UPID("pve1", "aptupdate", "", now() - 86400), node: "pve1", kind: "aptupdate", id: "", user: "root@pam", start: now() - 86400, end: now() - 86380, status: "OK" },
];
const logs: Record<string, string[]> = {
  [tasks[1].upid]: [
    "INFO: starting new backup job: vzdump 201 --storage nas-backup --mode snapshot",
    "INFO: Starting Backup of VM 201 (lxc)",
    "ERROR: Backup of VM 201 failed - CT is locked (backup)",
    "INFO: Failed at 2026-11-16 06:40:00",
    "TASK ERROR: job errors",
  ],
};

const TYPE: Record<PveAction, [string, string]> = {
  start: ["qmstart", "vzstart"],
  shutdown: ["qmshutdown", "vzshutdown"],
  reboot: ["qmreboot", "vzreboot"],
  stop: ["qmstop", "vzstop"],
  suspend: ["qmsuspend", "qmsuspend"],
  resume: ["qmresume", "qmresume"],
};
const AFTER: Record<PveAction, string> = { start: "running", shutdown: "stopped", reboot: "running", stop: "stopped", suspend: "paused", resume: "running" };

export const mockProxmox = {
  overview: async (_hostId: number): Promise<PveOverview> => {
    await new Promise((r) => setTimeout(r, 200));
    return {
      present: true,
      version: "8.2.4",
      cluster: "lab",
      quorate: true,
      nodes: [
        { name: "pve1", online: true, local: true, ip: "192.168.1.20", cpu: 0.06 + Math.random() * 0.05, maxcpu: 16, mem: 22.5 * GiB, maxmem: 62.7 * GiB, disk: 5 * GiB, maxdisk: 93.9 * GiB, uptime: 1204811 },
        { name: "pve2", online: true, local: false, ip: "192.168.1.21", cpu: 0.025, maxcpu: 8, mem: 9 * GiB, maxmem: 31.4 * GiB, disk: 4 * GiB, maxdisk: 93.9 * GiB, uptime: 604800 },
      ],
      guests: [...guests].sort((a, b) => a.vmid - b.vmid),
      storages: [
        { storage: "local", node: "pve1", plugin: "dir", content: ["iso", "vztmpl", "backup"], used: 20 * GiB, total: 93.9 * GiB, shared: false, status: "available" },
        { storage: "local-lvm", node: "pve1", plugin: "lvmthin", content: ["images", "rootdir"], used: 200 * GiB, total: 800 * GiB, shared: false, status: "available" },
        { storage: "local-lvm", node: "pve2", plugin: "lvmthin", content: ["images", "rootdir"], used: 0, total: 0, shared: false, status: "unknown" },
        { storage: "nas-backup", node: "pve1", plugin: "nfs", content: ["backup"], used: 1024 * GiB, total: 3072 * GiB, shared: true, status: "available" },
        { storage: "nas-backup", node: "pve2", plugin: "nfs", content: ["backup"], used: 1024 * GiB, total: 3072 * GiB, shared: true, status: "available" },
      ],
      tasks: [...tasks].sort((a, b) => Number(a.end != null) - Number(b.end != null) || (b.start ?? 0) - (a.start ?? 0)),
      local: { pveVersion: "8.2.4", kernel: "6.8.12-1-pve", cpuModel: "AMD Ryzen 7 5700G with Radeon Graphics", sockets: 1, cores: 8, threads: 16, loadavg: [0.52, 0.61, 0.58], swapUsed: 0, swapTotal: 8 * GiB, bootMode: "efi" },
      notes: [],
    };
  },
  action: async (_hostId: number, node: string, kind: string, vmid: number, action: PveAction): Promise<string> => {
    const guest = guests.find((x) => x.vmid === vmid && x.node === node);
    if (!guest) throw new Error(`Configuration file 'nodes/${node}/${kind}-server/${vmid}.conf' does not exist`);
    if (action === "start" && guest.status === "running") throw new Error(`VM ${vmid} already running`);
    const ts = now();
    const type = TYPE[action][kind === "qemu" ? 0 : 1];
    const t: PveTask = { upid: UPID(node, type, String(vmid), ts), node, kind: type, id: String(vmid), user: "root@pam", start: ts, end: null, status: null };
    tasks = [t, ...tasks];
    logs[t.upid] = [`${action} ${kind} ${vmid}`];
    setTimeout(() => {
      tasks = tasks.map((x) => (x.upid === t.upid ? { ...x, end: now(), status: "OK" } : x));
      logs[t.upid].push("TASK OK");
      guests = guests.map((x) =>
        x.vmid === vmid ? { ...x, status: AFTER[action], uptime: AFTER[action] === "running" ? 1 : 0, cpu: AFTER[action] === "running" ? 0.05 : 0, mem: AFTER[action] === "stopped" ? 0 : (x.maxmem ?? 0) / 3 } : x,
      );
    }, 2200);
    return t.upid;
  },
  task: async (_hostId: number, _node: string, upid: string): Promise<PveTaskDetail> => {
    const t = tasks.find((x) => x.upid === upid);
    if (!t) throw new Error("no such task");
    return { running: t.end == null, exitstatus: t.status, log: logs[upid] ?? ["no content"] };
  },
};
