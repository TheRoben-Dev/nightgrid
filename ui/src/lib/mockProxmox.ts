/**
 * Browser-only stand-in for Proxmox (M10r): a two-node cluster like the
 * fixtures in core/src/proxmox/fixtures. Actions start a fake task that
 * finishes after a moment and change the guest's state.
 */
import type {
  PveAction,
  PveBackups_Serialize as PveBackups,
  PveBackupFile_Serialize as PveBackupFile,
  PveSnapshot_Serialize as PveSnapshot,
  PveAppliance,
  PveCloneSpec_Deserialize as PveCloneSpec,
  PveCreateInfo_Serialize as PveCreateInfo,
  PveCtSpec_Deserialize as PveCtSpec,
  PveVmSpec_Deserialize as PveVmSpec,
  PveVolume,
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

const day = 86400;
const snaps: Record<number, PveSnapshot[]> = {
  100: [
    { name: "clean_install", description: "", time: now() - 60 * day, vmstate: false, parent: null, current: false },
    { name: "jellyfin", description: "after jellyfin", time: now() - 40 * day, vmstate: false, parent: "clean_install", current: false },
    { name: "pre_upgrade", description: "before upgrade to 24.04", time: now() - 5 * day, vmstate: true, parent: "clean_install", current: true },
  ],
};
const bf = (vmid: number, kind: string, ago: number, size: number, extra: Partial<PveBackupFile> = {}): PveBackupFile => {
  const t = now() - ago;
  const d = new Date(t * 1000).toISOString().slice(0, 19).replace(/[-:]/g, "_").replace("T", "-");
  return { volid: `nas-backup:backup/vzdump-${kind}-${vmid}-${d}.${kind === "qemu" ? "vma" : "tar"}.zst`, storage: "nas-backup", vmid, kind, time: t, size, format: kind === "qemu" ? "vma.zst" : "tar.zst", notes: "", protected: false, verified: null, ...extra };
};
let files: PveBackupFile[] = [
  bf(100, "qemu", 0.3 * day, 20 * GiB, { protected: true, verified: "ok", notes: "media nightly" }),
  bf(200, "lxc", 0.3 * day, 0.7 * GiB, { notes: "pihole nightly" }),
  bf(100, "qemu", 1.3 * day, 19.6 * GiB, { notes: "media nightly" }),
  bf(200, "lxc", 1.3 * day, 0.7 * GiB, { notes: "pihole nightly" }),
  bf(201, "lxc", 55 * day, 1 * GiB),
];

function finishLater(t: PveTask, after: () => void, ms = 1800) {
  tasks = [t, ...tasks];
  logs[t.upid] = [`${t.kind} ${t.id}`];
  setTimeout(() => {
    tasks = tasks.map((x) => (x.upid === t.upid ? { ...x, end: now(), status: "OK" } : x));
    logs[t.upid].push("TASK OK");
    after();
  }, ms);
}
const task = (node: string, kind: string, id: string): PveTask => {
  const ts = now() + Math.floor(Math.random() * 1000);
  return { upid: UPID(node, kind, id, ts), node, kind, id, user: "root@pam", start: now(), end: null, status: null };
};

let isos: PveVolume[] = [
  { volid: "local:iso/Win11_24H2_English_x64.iso", content: "iso", size: 6.2 * GiB, name: "Win11_24H2_English_x64.iso" },
  { volid: "local:iso/ubuntu-24.04.1-live-server-amd64.iso", content: "iso", size: 2.6 * GiB, name: "ubuntu-24.04.1-live-server-amd64.iso" },
];
let tmpls: PveVolume[] = [{ volid: "local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst", content: "vztmpl", size: 0.12 * GiB, name: "debian-12-standard_12.7-1_amd64.tar.zst" }];
const nextFree = () => {
  let id = 100;
  while (guests.some((x) => x.vmid === id)) id++;
  return id;
};

export const mockProxmox = {
  createInfo: async (): Promise<PveCreateInfo> => ({
    nextId: nextFree(),
    bridges: [
      { name: "vmbr0", cidr: "192.168.1.20/24", vlanAware: false, ports: "enp3s0", comment: "LAN", active: true },
      { name: "vmbr1", cidr: null, vlanAware: true, ports: "", comment: "lab vlans", active: true },
    ],
    isos: [...isos],
    templates: [...tmpls],
    notes: [],
  }),
  vmCreate: async (v: PveVmSpec): Promise<string> => {
    if (guests.some((x) => x.vmid === v.vmid)) throw new Error(`unable to create VM ${v.vmid} - VM ${v.vmid} already exists on node '${v.node}'`);
    const t = task(v.node, "qmcreate", String(v.vmid));
    finishLater(t, () => (guests = [...guests, g(v.vmid, "qemu", v.node, v.name, v.start ? "running" : "stopped", { maxcpu: v.cores, maxmem: v.memoryMb * 1024 ** 2, maxdisk: v.diskGb * GiB })]));
    return t.upid;
  },
  ctCreate: async (c: PveCtSpec): Promise<string> => {
    if (guests.some((x) => x.vmid === c.vmid)) throw new Error(`CT ${c.vmid} already exists on node '${c.node}'`);
    const t = task(c.node, "vzcreate", String(c.vmid));
    finishLater(t, () => (guests = [...guests, g(c.vmid, "lxc", c.node, c.hostname, c.start ? "running" : "stopped", { maxcpu: c.cores, maxmem: c.memoryMb * 1024 ** 2, maxdisk: c.diskGb * GiB })]));
    return t.upid;
  },
  clone: async (c: PveCloneSpec): Promise<string> => {
    const src = guests.find((x) => x.vmid === c.vmid);
    if (!src) throw new Error("source not found");
    const t = task(c.node, c.kind === "qemu" ? "qmclone" : "vzclone", String(c.vmid));
    finishLater(t, () => (guests = [...guests, { ...src, vmid: c.newid, name: c.name, template: false, status: "stopped", uptime: 0, cpu: 0, mem: 0, tags: [] }]), 2500);
    return t.upid;
  },
  template: async (node: string, vmid: number): Promise<string> => {
    const t = task(node, "qmtemplate", String(vmid));
    finishLater(t, () => (guests = guests.map((x) => (x.vmid === vmid ? { ...x, template: true } : x))));
    return t.upid;
  },
  destroy: async (node: string, vmid: number): Promise<string> => {
    const x = guests.find((y) => y.vmid === vmid);
    if (x?.status === "running") throw new Error(`VM ${vmid} is running - destroy failed`);
    const t = task(node, "qmdestroy", String(vmid));
    finishLater(t, () => (guests = guests.filter((y) => y.vmid !== vmid)));
    return t.upid;
  },
  isoDownload: async (node: string, storage: string, filename: string): Promise<string> => {
    const t = task(node, "download", filename);
    finishLater(t, () => (isos = [...isos, { volid: `${storage}:iso/${filename}`, content: "iso", size: 3 * GiB, name: filename }]), 3000);
    return t.upid;
  },
  appliances: async (): Promise<PveAppliance[]> => [
    { template: "debian-12-standard_12.7-1_amd64.tar.zst", headline: "Debian 12 Bookworm (standard)", os: "debian-12", section: "system", version: "12.7-1" },
    { template: "ubuntu-24.04-standard_24.04-2_amd64.tar.zst", headline: "Ubuntu 24.04 Noble (standard)", os: "ubuntu-24.04", section: "system", version: "24.04-2" },
    { template: "alpine-3.20-default_20240908_amd64.tar.xz", headline: "Alpine Linux 3.20", os: "alpine-3.20", section: "system", version: "3.20-0" },
    { template: "debian-12-turnkey-wordpress_18.0-1_amd64.tar.gz", headline: "TurnKey WordPress", os: "debian-12", section: "turnkeylinux", version: "18.0-1" },
  ],
  applianceDownload: async (node: string, storage: string, template: string): Promise<string> => {
    const t = task(node, "download", template);
    finishLater(t, () => (tmpls = [...tmpls, { volid: `${storage}:vztmpl/${template}`, content: "vztmpl", size: 0.1 * GiB, name: template }]));
    return t.upid;
  },
  snapshots: async (_node: string, vmid: number): Promise<PveSnapshot[]> => snaps[vmid] ?? [],
  snapshotCreate: async (node: string, vmid: number, name: string, description: string, vmstate: boolean): Promise<string> => {
    if ((snaps[vmid] ?? []).some((x) => x.name === name)) throw new Error(`snapshot name '${name}' already used`);
    const t = task(node, "qmsnapshot", String(vmid));
    finishLater(t, () => {
      const cur = (snaps[vmid] ?? []).find((x) => x.current);
      snaps[vmid] = [...(snaps[vmid] ?? []).map((x) => ({ ...x, current: false })), { name, description, time: now(), vmstate, parent: cur?.name ?? null, current: true }];
    });
    return t.upid;
  },
  snapshotRollback: async (node: string, vmid: number, name: string): Promise<string> => {
    const t = task(node, "qmrollback", String(vmid));
    finishLater(t, () => (snaps[vmid] = (snaps[vmid] ?? []).map((x) => ({ ...x, current: x.name === name }))));
    return t.upid;
  },
  snapshotDelete: async (node: string, vmid: number, name: string): Promise<string> => {
    const t = task(node, "qmdelsnapshot", String(vmid));
    finishLater(t, () => (snaps[vmid] = (snaps[vmid] ?? []).filter((x) => x.name !== name)));
    return t.upid;
  },
  backups: async (): Promise<PveBackups> => {
    await new Promise((r) => setTimeout(r, 150));
    return {
      jobs: [
        { id: "backup-1a2b3c4d-5e6f", enabled: true, schedule: "02:00", storage: "nas-backup", mode: "snapshot", guests: "100,200,201", exclude: "", comment: "", nextRun: now() + 15 * 3600 },
        { id: "backup-weekly-all", enabled: false, schedule: "sun 03:00", storage: "nas-backup", mode: "stop", guests: "all", exclude: "9000", comment: "weekly full", nextRun: null },
      ],
      files: [...files].sort((a, b) => (b.time ?? 0) - (a.time ?? 0)),
      uncovered: [
        { vmid: 101, name: "win11", kind: "qemu" },
        { vmid: 102, name: "homeassistant", kind: "qemu" },
      ],
      notes: [],
    };
  },
  backupNow: async (node: string, vmid: number, storage: string): Promise<string> => {
    const g0 = guests.find((x) => x.vmid === vmid);
    const t = task(node, "vzdump", String(vmid));
    finishLater(t, () => (files = [bf(vmid, g0?.kind ?? "qemu", 0, (g0?.maxdisk ?? GiB) / 3, { storage, notes: "manual" }), ...files]), 2500);
    return t.upid;
  },
  restore: async (node: string, kind: string, vmid: number, volid: string): Promise<string> => {
    const t = task(node, kind === "qemu" ? "qmrestore" : "vzrestore", String(vmid));
    const src = files.find((f0) => f0.volid === volid);
    finishLater(t, () => {
      if (!guests.some((x) => x.vmid === vmid)) guests = [...guests, g(vmid, kind, node, `restored-${src?.vmid ?? vmid}`, "stopped")];
    }, 2500);
    return t.upid;
  },
  protect: async (volid: string, protect: boolean): Promise<null> => {
    files = files.map((f0) => (f0.volid === volid ? { ...f0, protected: protect } : f0));
    return null;
  },
  deleteBackup: async (volid: string): Promise<null> => {
    if (files.find((f0) => f0.volid === volid)?.protected) throw new Error("can't remove protected volume");
    files = files.filter((f0) => f0.volid !== volid);
    return null;
  },
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
