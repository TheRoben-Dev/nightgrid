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
  PveSandboxSpec_Deserialize as PveSandboxSpec,
  PveSandboxDone_Serialize as PveSandboxDone,
  PveCreateInfo_Serialize as PveCreateInfo,
  PveCtSpec_Deserialize as PveCtSpec,
  PveVmSpec_Deserialize as PveVmSpec,
  PveVolume,
  PveGuest_Serialize as PveGuest,
  PveOverview_Serialize as PveOverview,
  PveTask_Serialize as PveTask,
  PveTaskDetail_Serialize as PveTaskDetail,
  PveConfig_Serialize as PveConfig,
  PveConfigEntry_Serialize as PveConfigEntry,
  PveHostDevices,
  PveSetting,
  PveIface_Serialize as PveIface,
  PveIfaceSpec_Deserialize as PveIfaceSpec,
  PveNetwork_Serialize as PveNetwork,
  PveNetApply_Serialize as PveNetApply,
  PveFirewall_Serialize as PveFirewall,
  PveFwNamed,
  PveFwOptions,
  PveFwRule,
  PveFwRuleSpec,
  PveFwRun,
  PveFwScope,
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
  g(9100, "qemu", "pve1", "media-sandbox", "running", { tags: ["media", "nightgrid-sandbox", "ng-sbx-of-100", `ng-sbx-until-${Math.floor(Date.now() / 1000) + 5 * 3600 + 1200}`] }),
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

// ---- part 4: configs ----
const configs: Record<number, Record<string, string>> = {};
const pendingCfg: Record<number, Record<string, string | null>> = {};
let digestSeq = 1;
function cfgOf(vmid: number): Record<string, string> {
  if (configs[vmid]) return configs[vmid];
  const x = guests.find((q) => q.vmid === vmid);
  const mem = String(Math.round((x?.maxmem ?? 2 * GiB) / 1024 ** 2));
  const mac = `BC:24:11:00:${String(vmid % 100).padStart(2, "0")}:01`;
  const c: Record<string, string> =
    x?.kind === "lxc"
      ? { hostname: x.name, cores: String(x.maxcpu), memory: mem, swap: "512", rootfs: `local-lvm:vm-${vmid}-disk-0,size=8G`, net0: `name=eth0,bridge=vmbr0,firewall=1,hwaddr=${mac},ip=dhcp,type=veth`, onboot: "1", features: "nesting=1", unprivileged: "1" }
      : {
          name: x?.name ?? String(vmid),
          cores: String(x?.maxcpu ?? 2),
          sockets: "1",
          memory: mem,
          cpu: "x86-64-v2-AES",
          machine: "q35",
          scsihw: "virtio-scsi-single",
          scsi0: `local-lvm:vm-${vmid}-disk-0,discard=on,iothread=1,size=32G`,
          ide2: "none,media=cdrom",
          net0: `virtio=${mac},bridge=vmbr0,firewall=1`,
          boot: "order=scsi0;ide2;net0",
          agent: "1",
          onboot: vmid === 100 ? "1" : "0",
        };
  if (vmid === 100) Object.assign(c, { hostpci0: "0000:01:00,pcie=1", usb0: "host=1050:0407", unused0: "local-lvm:vm-100-disk-2", startup: "order=2,up=30", tags: "media;prod" });
  configs[vmid] = c;
  return c;
}
function opts(v: string) {
  return v.split(",").map((p) => (p.includes("=") ? (p.split("=", 2) as [string, string]) : (["", p] as [string, string])));
}
function configView(vmid: number, kind: string): PveConfig {
  const c = cfgOf(vmid);
  const p = pendingCfg[vmid] ?? {};
  const keys = [...new Set([...Object.keys(c), ...Object.keys(p)])].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  const entries: PveConfigEntry[] = keys.map((key) => ({ key, value: c[key] ?? null, pending: p[key] ?? null, delete: key in p && p[key] === null }));
  entries.push({ key: "digest", value: `d${digestSeq}`, pending: null, delete: false });
  const cur = (e: PveConfigEntry) => e.pending ?? e.value ?? "";
  const isDisk = (k: string) => /^(scsi|virtio|sata|ide|mp)\d+$|^rootfs$|^efidisk0$|^tpmstate0$/.test(k);
  return {
    kind,
    digest: `d${digestSeq}`,
    entries,
    disks: entries.filter((e) => isDisk(e.key)).map((e) => {
      const o = opts(cur(e));
      const f = (k: string) => o.find(([a]) => a === k)?.[1] ?? "";
      return { key: e.key, volid: o[0][1] && !o[0][0] ? o[0][1] : `${o[0][0]}=${o[0][1]}`, size: f("size"), cdrom: f("media") === "cdrom", mountpoint: f("mp"), value: cur(e), pending: !!e.pending || e.delete };
    }),
    nics: entries.filter((e) => /^net\d+$/.test(e.key)).map((e) => {
      const o = opts(cur(e));
      const f = (k: string) => o.find(([a]) => a === k)?.[1] ?? "";
      return { key: e.key, model: kind === "lxc" ? "veth" : o[0][0], mac: kind === "lxc" ? f("hwaddr") : o[0][1], bridge: f("bridge"), vlan: f("tag") ? Number(f("tag")) : null, firewall: f("firewall") === "1", ip: f("ip"), value: cur(e), pending: !!e.pending, delete: e.delete };
    }),
    devices: entries.filter((e) => /^(hostpci|usb)\d+$/.test(e.key)),
    unused: entries.filter((e) => /^unused\d+$/.test(e.key)),
    pending: entries.some((e) => e.pending !== null || e.delete),
  };
}

// ---- part 5: node network ----
const nic = (iface: string, kind: string, x: Partial<PveIface> = {}): PveIface => ({
  iface, kind, active: true, autostart: true, exists: kind === "eth", cidr: "", gateway: "", cidr6: "", gateway6: "", ports: "", vlanAware: false, bondMode: "", vlanId: null, vlanRawDevice: "", mtu: null, comments: "", ...x,
});
let liveNet: PveIface[] = [
  nic("vmbr0", "bridge", { cidr: "192.168.1.20/24", gateway: "192.168.1.1", ports: "bond0", comments: "LAN" }),
  nic("vmbr1", "bridge", { vlanAware: true, comments: "lab vlans" }),
  nic("bond0", "bond", { ports: "enp3s0 enp4s0", bondMode: "active-backup" }),
  nic("vmbr0.20", "vlan", { cidr: "10.20.0.2/24", vlanId: 20, vlanRawDevice: "vmbr0", comments: "storage net" }),
  nic("enp3s0", "eth"),
  nic("enp4s0", "eth"),
  nic("wlp5s0", "eth", { active: false, autostart: false }),
];
let stagedNet: PveIface[] | null = null;
let netArmed = false;
let prevNet: PveIface[] = [];
const ifaceText = (i: PveIface) =>
  [`auto ${i.iface}`, `iface ${i.iface} inet ${i.cidr ? "static" : "manual"}`, ...(i.cidr ? [`\taddress ${i.cidr}`] : []), ...(i.gateway ? [`\tgateway ${i.gateway}`] : []), ...(i.kind === "bridge" ? [`\tbridge-ports ${i.ports || "none"}`] : []), ...(i.vlanAware ? ["\tbridge-vlan-aware yes"] : []), ...(i.comments ? [`#${i.comments}`] : [])];
function netDiff(): string {
  if (!stagedNet) return "";
  const out = ["--- /etc/network/interfaces", "+++ /etc/network/interfaces.new"];
  const names = [...new Set([...liveNet, ...stagedNet].map((i) => i.iface))];
  for (const n of names) {
    const a = liveNet.find((i) => i.iface === n);
    const b = stagedNet.find((i) => i.iface === n);
    const at = a ? ifaceText(a) : [];
    const bt = b ? ifaceText(b) : [];
    if (at.join("\n") === bt.join("\n")) continue;
    out.push(`@@ ${n} @@`, ...at.filter((l) => !bt.includes(l)).map((l) => `-${l}`), ...bt.filter((l) => !at.includes(l)).map((l) => `+${l}`));
  }
  return out.length > 2 ? out.join("\n") : "";
}
const fromSpec = (s: PveIfaceSpec): PveIface =>
  nic(s.iface, s.kind, { cidr: s.cidr, gateway: s.gateway, cidr6: s.cidr6, gateway6: s.gateway6, ports: s.ports, vlanAware: s.vlanAware, bondMode: s.bondMode, vlanId: s.vlanId ?? null, vlanRawDevice: s.vlanRawDevice, mtu: s.mtu ?? null, autostart: s.autostart, comments: s.comments, active: false });

// ---- part 6: firewall ----
interface FwState {
  rules: PveFwRule[];
  options: PveFwOptions | null;
}
const fwr = (pos: number, kind: string, action: string, x: Partial<PveFwRule> = {}): PveFwRule => ({
  pos, kind, action, macro: "", source: "", dest: "", proto: "", sport: "", dport: "", iface: "", log: "", enable: true, comment: "", ...x,
});
let fwSeq = 1;
const fwOpts = (enable: boolean, pin = "DROP", pout = "ACCEPT"): PveFwOptions => ({ enable, policyIn: pin, policyOut: pout, digest: "0" });
let fwStore: Record<string, FwState> = {
  cluster: { rules: [fwr(0, "in", "ACCEPT", { macro: "SSH", source: "+management", comment: "admin ssh" }), fwr(1, "in", "ACCEPT", { proto: "tcp", dport: "8006", source: "lan" }), fwr(2, "group", "webservers", { enable: false })], options: fwOpts(false) },
  "group:webservers": { rules: [fwr(0, "in", "ACCEPT", { macro: "HTTP" }), fwr(1, "in", "ACCEPT", { macro: "HTTPS" })], options: null },
  "group:monitoring": { rules: [fwr(0, "in", "ACCEPT", { proto: "tcp", dport: "9100", source: "192.168.1.10" })], options: null },
};
let fwGroups: PveFwNamed[] = [{ name: "monitoring", cidr: "", comment: "" }, { name: "webservers", cidr: "", comment: "80/443 in" }];
let fwAliases: PveFwNamed[] = [{ name: "lan", cidr: "192.168.1.0/24", comment: "home LAN" }];
let fwPrev: string | null = null;
const fwKey = (s: PveFwScope) => (s.level === "cluster" ? "cluster" : s.level === "node" ? `node:${s.node}` : s.level === "guest" ? `guest:${s.vmid}` : `group:${s.group}`);
function fwOf(s: PveFwScope): FwState {
  return (fwStore[fwKey(s)] ??= { rules: [], options: s.level === "group" ? null : s.level === "node" ? fwOpts(true, "", "") : fwOpts(false, "ACCEPT", "ACCEPT") });
}
function fwTouch(s: PveFwScope, token: string | null): PveFwRun {
  const guarded = s.level !== "guest";
  if (guarded && !token) fwPrev = JSON.stringify({ fwStore, fwGroups, fwAliases });
  fwSeq++;
  return { armed: guarded, token: token ?? String(now()), secs: 90 };
}
const renumber = (r: PveFwRule[]) => r.map((x, i) => ({ ...x, pos: i }));
const fromRule = (r: PveFwRuleSpec, pos: number): PveFwRule => ({ ...r, pos });

export const mockProxmox = {
  firewall: async (s: PveFwScope): Promise<PveFirewall> => {
    const f = fwOf(s);
    return {
      rules: structuredClone(f.rules),
      options: f.options ? { ...f.options, digest: String(fwSeq) } : null,
      digest: String(fwSeq),
      groups: [...fwGroups],
      aliases: [...fwAliases],
      ipsets: [{ name: "management", cidr: "", comment: "hosts allowed to the web UI and SSH" }],
      macros: [
        { name: "DNS", cidr: "", comment: "Domain Name System traffic" },
        { name: "HTTP", cidr: "", comment: "Hypertext Transfer Protocol (WWW)" },
        { name: "HTTPS", cidr: "", comment: "HTTP over SSL" },
        { name: "Ping", cidr: "", comment: "ICMP echo request" },
        { name: "SSH", cidr: "", comment: "Secure shell traffic" },
      ],
      rollbackArmed: false,
      notes: [],
    };
  },
  fwRuleSave: async (s: PveFwScope, token: string | null, pos: number | null, r: PveFwRuleSpec): Promise<PveFwRun> => {
    const run = fwTouch(s, token);
    const f = fwOf(s);
    f.rules = pos === null ? renumber([fromRule(r, 0), ...f.rules]) : f.rules.map((x) => (x.pos === pos ? fromRule(r, pos) : x));
    return run;
  },
  fwRuleMove: async (s: PveFwScope, token: string | null, pos: number, to: number): Promise<PveFwRun> => {
    const run = fwTouch(s, token);
    const f = fwOf(s);
    const r = [...f.rules];
    const [x] = r.splice(pos, 1);
    r.splice(to, 0, x);
    f.rules = renumber(r);
    return run;
  },
  fwRuleDelete: async (s: PveFwScope, token: string | null, pos: number): Promise<PveFwRun> => {
    const run = fwTouch(s, token);
    const f = fwOf(s);
    f.rules = renumber(f.rules.filter((x) => x.pos !== pos));
    return run;
  },
  fwOptions: async (s: PveFwScope, token: string | null, enable: boolean, pin: string, pout: string): Promise<PveFwRun> => {
    const run = fwTouch(s, token);
    const f = fwOf(s);
    f.options = { enable, policyIn: pin, policyOut: pout, digest: "0" };
    return run;
  },
  fwObject: async (token: string | null, op: string, name: string, cidr: string, comment: string): Promise<PveFwRun> => {
    const run = fwTouch({ level: "cluster", node: "", kind: "", vmid: 0, group: "" }, token);
    if (op === "group-add") fwGroups = [...fwGroups, { name, cidr: "", comment }].sort((a, b) => a.name.localeCompare(b.name));
    if (op === "group-delete") {
      if (fwOf({ level: "group", node: "", kind: "", vmid: 0, group: name }).rules.length) throw new Error(`security group '${name}' is not empty`);
      fwGroups = fwGroups.filter((g) => g.name !== name);
    }
    if (op === "alias-add") fwAliases = [...fwAliases, { name, cidr, comment }];
    if (op === "alias-delete") fwAliases = fwAliases.filter((a) => a.name !== name);
    return run;
  },
  fwSettle: async (keep: boolean): Promise<null> => {
    if (!keep && fwPrev) ({ fwStore, fwGroups, fwAliases } = JSON.parse(fwPrev));
    fwPrev = null;
    return null;
  },
  network: async (): Promise<PveNetwork> => ({ ifaces: structuredClone(stagedNet ?? liveNet), diff: netDiff(), rollbackArmed: netArmed, notes: [] }),
  ifaceSave: async (spec: PveIfaceSpec, create: boolean): Promise<null> => {
    const cur = structuredClone(stagedNet ?? liveNet);
    if (create && cur.some((i) => i.iface === spec.iface)) throw new Error(`interface '${spec.iface}' already exists`);
    if (spec.kind === "bridge" && !/^vmbr\d+$/.test(spec.iface)) throw new Error("a Linux bridge is named vmbr plus a number (vmbr1)");
    const i = cur.findIndex((x) => x.iface === spec.iface);
    const next = { ...fromSpec(spec), active: i >= 0 ? cur[i].active : false, exists: i >= 0 ? cur[i].exists : false };
    if (i >= 0) cur[i] = next;
    else cur.push(next);
    stagedNet = cur;
    return null;
  },
  ifaceDelete: async (iface: string): Promise<null> => {
    stagedNet = structuredClone(stagedNet ?? liveNet).filter((i) => i.iface !== iface);
    return null;
  },
  networkRevert: async (): Promise<null> => {
    stagedNet = null;
    return null;
  },
  networkApply: async (node: string): Promise<PveNetApply> => {
    if (!stagedNet) throw new Error("no staged changes to apply");
    const t = task(node, "srvreload", "networking");
    finishLater(t, () => {
      liveNet = (stagedNet ?? liveNet).map((i) => ({ ...i, active: i.autostart }));
      stagedNet = null;
    }, 1200);
    netArmed = true;
    prevNet = structuredClone(liveNet);
    return { token: String(now()), upid: t.upid, secs: 90 };
  },
  networkSettle: async (keep: boolean): Promise<null> => {
    netArmed = false;
    if (!keep) liveNet = prevNet;
    return null;
  },
  config: async (vmid: number, kind: string): Promise<PveConfig> => configView(vmid, kind),
  configSet: async (vmid: number, digest: string, set: PveSetting[], remove: string[]): Promise<null> => {
    if (digest && digest !== `d${digestSeq}`) throw new Error("detected modified configuration - file changed by other user? Try again.");
    const c = cfgOf(vmid);
    const running = guests.find((x) => x.vmid === vmid)?.status === "running";
    const cold = (k: string) => running && /^(cores|sockets|memory|cpu|hostpci\d+|machine|swap)$/.test(k);
    for (const { key, value } of set) {
      if (cold(key)) (pendingCfg[vmid] ??= {})[key] = value;
      else c[key] = value;
    }
    for (const k of remove) {
      if (/^(scsi|virtio|sata)\d+$/.test(k) && !c[k].includes("media=cdrom")) {
        let i = 0;
        while (c[`unused${i}`]) i++;
        c[`unused${i}`] = c[k].split(",")[0];
      }
      if (cold(k)) (pendingCfg[vmid] ??= {})[k] = null;
      else delete c[k];
    }
    digestSeq++;
    return null;
  },
  diskResize: async (vmid: number, disk: string, gb: number): Promise<string | null> => {
    const c = cfgOf(vmid);
    c[disk] = c[disk].replace(/size=(\d+)G/, (_, n) => `size=${Number(n) + gb}G`);
    digestSeq++;
    return null;
  },
  hostDevices: async (): Promise<PveHostDevices> => ({
    pci: [
      { id: "0000:01:00.0", vendor: "NVIDIA Corporation", device: "AD102 [GeForce RTX 4090]", class: "0x030000", iommuGroup: 14 },
      { id: "0000:01:00.1", vendor: "NVIDIA Corporation", device: "AD102 High Definition Audio Controller", class: "0x040300", iommuGroup: 14 },
      { id: "0000:05:00.0", vendor: "Intel Corporation", device: "Ethernet Controller I226-V", class: "0x020000", iommuGroup: 18 },
    ],
    usb: [
      { id: "1050:0407", path: "1-2", manufacturer: "Yubico", product: "YubiKey OTP+FIDO+CCID" },
      { id: "10c4:ea60", path: "2-1", manufacturer: "Silicon Labs", product: "Zigbee Dongle" },
    ],
    iommuOff: false,
    notes: [],
  }),
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
  sandboxCreate: async (c: PveSandboxSpec): Promise<string> => {
    const src = guests.find((x) => x.vmid === c.vmid);
    if (!src) throw new Error("source not found");
    if (guests.some((x) => x.vmid === c.newid)) throw new Error(`unable to create ${c.newid}: config file already exists`);
    const t = task(c.node, c.kind === "qemu" ? "qmclone" : "vzclone", String(c.vmid));
    finishLater(t, () => (guests = [...guests, { ...src, vmid: c.newid, name: c.name, template: false, status: "stopped", uptime: 0, cpu: 0, mem: 0 }]), 2500);
    return t.upid;
  },
  sandboxIsolate: async (c: PveSandboxSpec): Promise<PveSandboxDone> => {
    const until = c.hours ? Math.floor(Date.now() / 1000) + c.hours * 3600 : null;
    const own = (guests.find((x) => x.vmid === c.newid)?.tags ?? []).filter((t) => !t.startsWith("ng-sbx-") && t !== "nightgrid-sandbox");
    const tags = [...own, "nightgrid-sandbox", `ng-sbx-of-${c.vmid}`, ...(until ? [`ng-sbx-until-${until}`] : [])];
    guests = guests.map((x) => (x.vmid === c.newid ? { ...x, tags, status: c.start ? "running" : "stopped" } : x));
    const notes = [c.isolation === "cut" ? "net0: cable pulled (link down)" : `net0: moved from vmbr0 to ${c.bridge}`];
    if (c.vmid === 100) notes.push("hostpci0: passthrough removed", "usb0: passthrough removed");
    return { notes, until, upid: c.start ? task(c.node, "qmstart", String(c.newid)).upid : null };
  },
  sandboxExtend: async (vmid: number, hours: number): Promise<number | null> => {
    const until = hours ? Math.floor(Date.now() / 1000) + hours * 3600 : null;
    guests = guests.map((x) => (x.vmid === vmid ? { ...x, tags: [...x.tags.filter((t) => !t.startsWith("ng-sbx-until-")), ...(until ? [`ng-sbx-until-${until}`] : [])] } : x));
    return until;
  },
  sandboxKeep: async (vmid: number): Promise<null> => {
    guests = guests.map((x) => (x.vmid === vmid ? { ...x, tags: x.tags.filter((t) => !t.startsWith("ng-sbx-") && t !== "nightgrid-sandbox") } : x));
    return null;
  },
  sandboxDestroy: async (node: string, vmid: number): Promise<string> => {
    const t = task(node, "qmdestroy", String(vmid));
    finishLater(t, () => (guests = guests.filter((y) => y.vmid !== vmid)));
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
