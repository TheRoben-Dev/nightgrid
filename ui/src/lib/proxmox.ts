/** Pure helpers for the Proxmox view (M10r). Data comes from `pvesh` via core/src/proxmox. */
import type {
  PveAction,
  PveGuest_Serialize as PveGuest,
  PveNode_Serialize as PveNode,
  PveOverview_Serialize as PveOverview,
  PveStorage,
  PveTask_Serialize as PveTask,
} from "./bindings";

export type { PveAction, PveGuest, PveNode, PveOverview, PveStorage, PveTask };
import type { PveBackupFile_Serialize as PveBackupFile, PveBackups_Serialize as PveBackups, PveSnapshot_Serialize as PveSnapshot, PveStorageRef } from "./bindings";


export const ACTION_WORD: Record<PveAction, string> = {
  start: "Start",
  shutdown: "Shut down",
  reboot: "Reboot",
  stop: "Force stop",
  suspend: "Pause",
  resume: "Resume",
};

export const ACTION_CONSEQUENCE: Partial<Record<PveAction, string>> = {
  shutdown: "The guest is asked to shut down cleanly (ACPI for VMs, init for containers). Proxmox gives up after its timeout.",
  reboot: "The guest restarts; its open connections are dropped.",
  stop: "Pulls the plug: the guest stops at once without shutting down. Unsaved data in it is lost.",
  suspend: "The VM is frozen in memory; it keeps its RAM but uses no CPU until resumed.",
};

export const isDestructive = (a: PveAction): boolean => a !== "start" && a !== "resume";

/** What makes sense for a guest right now. */
export function guestActions(g: PveGuest): PveAction[] {
  if (g.template) return [];
  if (g.status === "running") return g.kind === "qemu" ? ["shutdown", "reboot", "suspend", "stop"] : ["shutdown", "reboot", "stop"];
  if (g.status === "paused" || g.status === "suspended") return ["resume", "stop"];
  if (g.status === "stopped") return ["start"];
  return ["stop"];
}

/** The everyday action as a button; the rest go in the "⋯" menu. */
export function splitActions(g: PveGuest): { main: PveAction[]; more: PveAction[] } {
  const all = guestActions(g);
  const main: PveAction[] = all.filter((a) => a === "start" || a === "resume" || a === "shutdown");
  return { main, more: all.filter((a) => !main.includes(a)) };
}

export type Tone = "ok" | "warn" | "bad" | "off";

export function guestTone(g: PveGuest): Tone {
  if (g.template) return "off";
  if (g.lock) return "warn";
  if (g.status === "running") return "ok";
  if (g.status === "stopped") return "off";
  return "warn";
}

export const pct = (used: number | null, total: number | null): number | null => (total && total > 0 ? Math.min(100, Math.max(0, ((used ?? 0) / total) * 100)) : null);

export function usageTone(p: number | null): Tone {
  if (p == null) return "off";
  return p >= 90 ? "bad" : p >= 75 ? "warn" : "ok";
}

export function formatBytes(n: number | null): string {
  if (!n || n < 0) return "0 B";
  const u = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let i = 0;
  let v = n ?? 0;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${u[i]}`;
}

export function formatUptime(s: number | null): string {
  if (!s || s < 0) return "--";
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  return d ? `${d}d ${h}h` : h ? `${h}h ${m}m` : `${m}m`;
}

const TASK_WORDS: Record<string, string> = {
  qmstart: "VM start",
  qmstop: "VM stop",
  qmshutdown: "VM shutdown",
  qmreboot: "VM reboot",
  qmsuspend: "VM pause",
  qmresume: "VM resume",
  qmclone: "VM clone",
  qmcreate: "VM create",
  qmdestroy: "VM destroy",
  qmigrate: "VM migrate",
  qmsnapshot: "VM snapshot",
  qmdelsnapshot: "Delete snapshot",
  qmrollback: "Snapshot rollback",
  qmtemplate: "Convert to template",
  vzstart: "CT start",
  vzstop: "CT stop",
  vzshutdown: "CT shutdown",
  vzreboot: "CT reboot",
  vzcreate: "CT create",
  vzdestroy: "CT destroy",
  vzdump: "Backup",
  vncproxy: "Console",
  vncshell: "Node shell",
  termproxy: "Terminal",
  spiceproxy: "SPICE console",
  imgcopy: "Copy disk",
  download: "Download",
  aptupdate: "Package lists",
  startall: "Start all guests",
  stopall: "Stop all guests",
  srvreload: "Reload service",
  srvrestart: "Restart service",
};

export const taskLabel = (t: PveTask): string => `${TASK_WORDS[t.kind] ?? t.kind}${t.id ? ` ${t.id}` : ""}`;

export function taskTone(t: PveTask): Tone {
  if (t.end == null) return "warn";
  if (t.status === "OK") return "ok";
  if (t.status?.startsWith("WARNINGS")) return "warn";
  return "bad";
}

export interface Counts {
  running: number;
  stopped: number;
  templates: number;
  vms: number;
  cts: number;
}

export function counts(gs: PveGuest[]): Counts {
  const real = gs.filter((g) => !g.template);
  return {
    running: real.filter((g) => g.status === "running").length,
    stopped: real.filter((g) => g.status === "stopped").length,
    templates: gs.length - real.length,
    vms: real.filter((g) => g.kind === "qemu").length,
    cts: real.filter((g) => g.kind === "lxc").length,
  };
}

/** Free-text filter over name, id, node and tags; `#tag` matches a tag exactly. */
export function filterGuests(gs: PveGuest[], q: string, node: string | null): PveGuest[] {
  const terms = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return gs.filter(
    (g) =>
      (node == null || g.node === node) &&
      terms.every((t) =>
        t.startsWith("#") ? g.tags.some((x) => x.toLowerCase() === t.slice(1)) : `${g.vmid} ${g.name} ${g.node} ${g.kind === "qemu" ? "vm" : "ct"} ${g.tags.join(" ")} ${g.status}`.toLowerCase().includes(t),
      ),
  );
}

/** Storages grouped by name; shared ones appear once. */
export function storageRows(ss: PveStorage[]): { key: string; s: PveStorage; nodes: string[] }[] {
  const out: { key: string; s: PveStorage; nodes: string[] }[] = [];
  for (const s of ss) {
    const prev = s.shared ? out.find((r) => r.s.shared && r.s.storage === s.storage) : undefined;
    if (prev) prev.nodes.push(s.node);
    else out.push({ key: `${s.storage}@${s.shared ? "*" : s.node}`, s, nodes: [s.node] });
  }
  return out;
}

/** A guest's screen/console only works on the node this host is. */
export const isLocal = (o: PveOverview | null, node: string): boolean => !!o?.nodes.find((n) => n.name === node)?.local;

// ---------------------------------------------------------------- part 2


export type { PveBackupFile, PveBackups, PveSnapshot, PveStorageRef };

/** Proxmox snapshot names: a letter, then letters, digits, `_` or `-`; 2–40 long. */
export const validSnapname = (n: string): boolean => /^[A-Za-z][A-Za-z0-9_-]{1,39}$/.test(n) && n !== "current";

/** A default snapshot name: `snap_2026_11_16_0930`. */
export function snapName(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `snap_${d.getFullYear()}_${p(d.getMonth() + 1)}_${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}`;
}

/** Snapshots as a tree, oldest root first, with each one's depth. */
export function snapTree(snaps: PveSnapshot[]): { s: PveSnapshot; depth: number }[] {
  const names = new Set(snaps.map((s) => s.name));
  const kids = (parent: string | null) => snaps.filter((s) => (s.parent && names.has(s.parent) ? s.parent : null) === parent).sort((a, b) => (a.time ?? 0) - (b.time ?? 0));
  const out: { s: PveSnapshot; depth: number }[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const s of kids(parent)) {
      out.push({ s, depth });
      walk(s.name, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

/** Storages (node + id) that hold backups and can be read now. */
export function backupTargets(o: PveOverview | null): PveStorageRef[] {
  return (o?.storages ?? []).filter((s) => s.content.includes("backup") && s.status === "available").map((s) => ({ node: s.node, storage: s.storage }));
}

/** Backup storages to offer for "Back up now" from `node`. */
export const backupStorages = (o: PveOverview | null, node: string): string[] =>
  [...new Set((o?.storages ?? []).filter((s) => s.node === node && s.content.includes("backup") && s.status === "available").map((s) => s.storage))];

/** Storages on `node` that can hold a restored guest's disks. */
export const restoreStorages = (o: PveOverview | null, node: string, kind: string): string[] =>
  [...new Set((o?.storages ?? []).filter((s) => s.node === node && s.content.includes(kind === "qemu" ? "images" : "rootdir") && s.status === "available").map((s) => s.storage))];

/** The lowest free guest id at or above 100. */
export function nextVmid(o: PveOverview | null): number {
  const used = new Set((o?.guests ?? []).map((g) => g.vmid));
  let id = 100;
  while (used.has(id)) id++;
  return id;
}

export interface GuestBackups {
  vmid: number;
  name: string;
  kind: string;
  files: PveBackupFile[];
  latest: PveBackupFile | null;
  size: number;
  tone: Tone;
  covered: boolean;
}

/** Backups per guest (every guest, even ones without any), worst first. */
export function backupsByGuest(o: PveOverview | null, b: PveBackups | null, now = Date.now() / 1000): GuestBackups[] {
  if (!b) return [];
  const uncovered = new Set(b.uncovered.map((u) => u.vmid));
  const ids = new Set([...(o?.guests ?? []).filter((g) => !g.template).map((g) => g.vmid), ...b.files.map((f) => f.vmid)]);
  const rows = [...ids].map((vmid) => {
    const g = o?.guests.find((x) => x.vmid === vmid);
    const files = b.files.filter((f) => f.vmid === vmid).sort((a, c) => (c.time ?? 0) - (a.time ?? 0));
    const latest = files[0] ?? null;
    const age = latest ? now - (latest.time ?? 0) : Infinity;
    const tone: Tone = !latest ? (g ? "bad" : "off") : age > 7 * 86400 ? "bad" : age > 2 * 86400 ? "warn" : "ok";
    return { vmid, name: g?.name ?? "(guest gone)", kind: g?.kind ?? latest?.kind ?? "qemu", files, latest, size: files.reduce((n, f) => n + (f.size ?? 0), 0), tone, covered: !uncovered.has(vmid) };
  });
  const rank: Record<Tone, number> = { bad: 0, warn: 1, ok: 2, off: 3 };
  return rows.sort((a, c) => rank[a.tone] - rank[c.tone] || a.vmid - c.vmid);
}

/** "daily 02:00", "sun 03:00" as Proxmox writes it, made a little friendlier. */
export function scheduleText(s: string): string {
  if (/^\d{1,2}:\d{2}$/.test(s)) return `daily ${s}`;
  return s;
}

// ---- part 3: create, clone, templates, ISOs ----

/** A guest name Proxmox accepts (a DNS name). */
export const validGuestName = (n: string): boolean => n.length > 0 && n.length <= 63 && /^[A-Za-z0-9.-]+$/.test(n) && !/^[-.]|[-.]$/.test(n);

const ipv4 = (a: string): boolean => {
  const p = a.split(".");
  return p.length === 4 && p.every((x) => /^\d{1,3}$/.test(x) && Number(x) <= 255);
};
export const validIpv4 = ipv4;

/** "dhcp" or an IPv4 address with a prefix. */
export function validCtIp(ip: string): boolean {
  if (ip === "dhcp") return true;
  const [a, n, ...rest] = ip.split("/");
  return !rest.length && ipv4(a) && /^\d{1,2}$/.test(n ?? "") && Number(n) >= 1 && Number(n) <= 32;
}

/** OpenSSH public keys, one per line, at least one. */
export function validSshKeys(k: string): boolean {
  const lines = k
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
  return lines.length > 0 && k.length <= 16384 && lines.every((l) => /^(ssh-|ecdsa-|sk-)\S+\s+[A-Za-z0-9+/=]{16,}(\s|$)/.test(l));
}

export const validUrl = (u: string): boolean => /^https?:\/\/[^\s"'`]{3,}$/.test(u) && u.length <= 2048;

/** The ISO file name a download URL suggests ("" when it doesn't end in .iso). */
export function isoNameFromUrl(u: string): string {
  const last = u.split(/[?#]/)[0].split("/").pop() ?? "";
  let name = last;
  try {
    name = decodeURIComponent(last);
  } catch {
    /* keep it as is */
  }
  name = name.replace(/[^A-Za-z0-9._+-]/g, "_");
  return /\.iso$/i.test(name) && !name.startsWith(".") ? name : "";
}

export const validIsoName = (n: string): boolean => /^[A-Za-z0-9_+-][A-Za-z0-9._+-]*\.iso$/i.test(n) && n.length <= 255;
export const validSha256 = (s: string): boolean => /^[0-9a-fA-F]{64}$/.test(s);

export const OSTYPES: { id: string; label: string }[] = [
  { id: "l26", label: "Linux" },
  { id: "win11", label: "Windows 11/2022/2025" },
  { id: "win10", label: "Windows 10/2016/2019" },
  { id: "other", label: "Other" },
];

/** Whether a vmid is free in the cluster. */
export const vmidFree = (o: PveOverview | null, id: number): boolean => Number.isInteger(id) && id >= 100 && id <= 999999999 && !o?.guests.some((g) => g.vmid === id);

// ---- part 4: hardware and options ----

import type {
  PveConfig_Serialize as PveConfig,
  PveConfigEntry_Serialize as PveConfigEntry,
  PveDisk,
  PveHostDevices,
  PveNic_Serialize as PveNic,
  PvePci,
  PveSetting,
  PveUsb,
} from "./bindings";
export type { PveConfig, PveConfigEntry, PveDisk, PveHostDevices, PveNic, PvePci, PveSetting, PveUsb };

/** `a=b,c,d=e` as pairs; a bare item has an empty key. */
export function optionsOf(v: string): [string, string][] {
  return v
    .split(",")
    .filter(Boolean)
    .map((p) => {
      const i = p.indexOf("=");
      return i < 0 ? (["", p] as [string, string]) : ([p.slice(0, i), p.slice(i + 1)] as [string, string]);
    });
}

export const optionsText = (o: [string, string][]): string => o.map(([k, v]) => (k ? `${k}=${v}` : v)).join(",");

/** Replaces or removes options, keeping the rest (MAC addresses, sizes…) as they are. */
export function withOptions(v: string, changes: Record<string, string | null>): string {
  const o = optionsOf(v).filter(([k]) => !(k in changes && changes[k] === null));
  for (const [k, val] of Object.entries(changes)) {
    if (val === null) continue;
    const i = o.findIndex(([a]) => a === k);
    if (i >= 0) o[i] = [k, val];
    else o.push([k, val]);
  }
  return optionsText(o);
}

/** What a key will be after a restart (or is now). */
export function cfgValue(c: PveConfig | null, key: string): string | null {
  const e = c?.entries.find((x) => x.key === key);
  if (!e || e.delete) return null;
  return e.pending ?? e.value ?? null;
}

/** The lowest free index for `prefix` (net, scsi, mp, hostpci, usb…). */
export function freeKey(c: PveConfig | null, prefix: string, max: number): string | null {
  const used = new Set((c?.entries ?? []).map((e) => e.key));
  for (let i = 0; i <= max; i++) if (!used.has(`${prefix}${i}`)) return `${prefix}${i}`;
  return null;
}

export const NIC_MODELS = ["virtio", "e1000", "e1000e", "rtl8139", "vmxnet3"];
export const CPU_TYPES = ["host", "x86-64-v2-AES", "x86-64-v3", "x86-64-v4", "kvm64", "qemu64"];
export const DISK_BUSES: { id: string; max: number }[] = [
  { id: "scsi", max: 30 },
  { id: "virtio", max: 15 },
  { id: "sata", max: 5 },
];

export interface NicForm {
  model: string;
  bridge: string;
  vlan: number | null;
  firewall: boolean;
  /** Containers: dhcp or CIDR. */
  ip: string;
  gateway: string;
}

/** A NIC value: an edit keeps the MAC and any option the form doesn't show. */
export function nicValue(kind: string, key: string, existing: string | null, f: NicForm): string {
  const common: Record<string, string | null> = { bridge: f.bridge, tag: f.vlan ? String(f.vlan) : null, firewall: f.firewall ? "1" : null };
  if (kind === "lxc") {
    const base = existing ?? `name=eth${key.replace(/\D/g, "")}`;
    return withOptions(base, { ...common, ip: f.ip || "dhcp", gw: f.ip && f.ip !== "dhcp" && f.gateway ? f.gateway : null });
  }
  if (!existing) return withOptions(f.model, common);
  const o = optionsOf(existing);
  const first = o[0];
  // virtio=AA:BB:… → e1000=AA:BB:… keeps the MAC with a new model.
  if (first && NIC_MODELS.includes(first[0])) o[0] = [f.model, first[1]];
  return withOptions(optionsText(o), common);
}

export function nicForm(n: PveNic | null, bridge = "vmbr0"): NicForm {
  if (!n) return { model: "virtio", bridge, vlan: null, firewall: true, ip: "dhcp", gateway: "" };
  const gw = optionsOf(n.value).find(([k]) => k === "gw")?.[1] ?? "";
  return { model: n.model || "virtio", bridge: n.bridge, vlan: n.vlan, firewall: n.firewall, ip: n.ip || "dhcp", gateway: gw };
}

/** A new disk: `storage:GB` plus sensible options. */
export function diskValue(kind: string, bus: string, storage: string, gb: number, mountpoint = ""): string {
  if (kind === "lxc") return `${storage}:${gb},mp=${mountpoint}`;
  return bus === "scsi" ? `${storage}:${gb},discard=on,iothread=1` : bus === "virtio" ? `${storage}:${gb},discard=on,iothread=1` : `${storage}:${gb},discard=on`;
}

export const cdromValue = (volid: string | null): string => `${volid || "none"},media=cdrom`;

export const validMountPath = (p: string): boolean => /^\/[A-Za-z0-9._/-]+$/.test(p) && !/(^|\/)\.\.(\/|$)/.test(p) && p !== "/";

/** A PCI device for hostpciN; all functions drops the `.0`, PCIe only on q35. */
export function pciValue(id: string, allFunctions: boolean, pcie: boolean): string {
  const dev = allFunctions ? id.replace(/\.\d$/, "") : id;
  return pcie ? `${dev},pcie=1` : dev;
}

export const isQ35 = (c: PveConfig | null): boolean => (cfgValue(c, "machine") ?? "").includes("q35");

/** Other devices in the same IOMMU group go to the guest too. */
export function groupMates(pci: PvePci[], id: string): PvePci[] {
  const me = pci.find((p) => p.id === id);
  if (!me || me.iommuGroup < 0) return [];
  return pci.filter((p) => p.iommuGroup === me.iommuGroup && p.id !== id && p.id.replace(/\.\d$/, "") !== id.replace(/\.\d$/, ""));
}

/** A device entry as words: hostpci0 → the PCI device's name if the node has it. */
export function deviceLabel(e: PveConfigEntry, d: PveHostDevices | null): string {
  const v = e.pending ?? e.value ?? "";
  const first = optionsOf(v)[0]?.[1] ?? v;
  if (e.key.startsWith("hostpci")) {
    const dev = d?.pci.find((p) => p.id === first || p.id.replace(/\.\d$/, "") === first);
    return dev ? `${first} · ${dev.device || dev.vendor}` : first;
  }
  const host = optionsOf(v).find(([k]) => k === "host")?.[1] ?? v;
  const u = d?.usb.find((x) => x.id === host || x.path === host);
  return u ? `${host} · ${u.product || u.manufacturer}` : host;
}

/** `order=2,up=30` → { order: 2, up: 30 }. */
export function startupOf(v: string | null): { order: number | null; up: number | null } {
  const o = optionsOf(v ?? "");
  const n = (k: string) => {
    const x = o.find(([a]) => a === k)?.[1];
    return x != null && /^\d+$/.test(x) ? Number(x) : null;
  };
  return { order: n("order"), up: n("up") };
}

export function startupText(order: number | null, up: number | null): string | null {
  const p = [order != null ? `order=${order}` : "", up != null ? `up=${up}` : ""].filter(Boolean);
  return p.length ? p.join(",") : null;
}

/** Settings that changed: only those are sent. */
export function changed(c: PveConfig | null, want: Record<string, string | null>): { set: PveSetting[]; remove: string[] } {
  const set: PveSetting[] = [];
  const remove: string[] = [];
  for (const [key, v] of Object.entries(want)) {
    const now = cfgValue(c, key);
    if (v === null || v === "") {
      if (now !== null) remove.push(key);
    } else if (v !== now) set.push({ key, value: v });
  }
  return { set, remove };
}

// ---- part 5: node network ----

import type { PveIface_Serialize as PveIface, PveIfaceSpec_Deserialize as PveIfaceSpec, PveNetwork_Serialize as PveNetwork } from "./bindings";
export type { PveIface, PveIfaceSpec, PveNetwork };

export const BOND_MODES = ["active-backup", "802.3ad", "balance-rr", "balance-xor", "broadcast", "balance-tlb", "balance-alb"];

export function ifaceSpec(i: PveIface): PveIfaceSpec {
  return {
    iface: i.iface,
    kind: i.kind,
    cidr: i.cidr,
    gateway: i.gateway,
    cidr6: i.cidr6,
    gateway6: i.gateway6,
    ports: i.ports,
    vlanAware: i.vlanAware,
    bondMode: i.bondMode,
    vlanId: i.vlanId,
    vlanRawDevice: i.vlanRawDevice,
    mtu: i.mtu,
    autostart: i.autostart,
    comments: i.comments,
  };
}

/** A new bridge, bond or VLAN with the next free name. */
export function newIface(kind: "bridge" | "bond" | "vlan", ifaces: PveIface[]): PveIfaceSpec {
  const used = new Set(ifaces.map((i) => i.iface));
  const free = (p: string) => {
    for (let n = p === "vmbr" ? 1 : 0; n < 10000; n++) if (!used.has(`${p}${n}`)) return `${p}${n}`;
    return p;
  };
  const vlanBase = ifaces.find((i) => i.kind === "bridge")?.iface ?? "vmbr0";
  return {
    iface: kind === "bridge" ? free("vmbr") : kind === "bond" ? free("bond") : `${vlanBase}.10`,
    kind,
    cidr: "",
    gateway: "",
    cidr6: "",
    gateway6: "",
    ports: "",
    vlanAware: false,
    bondMode: kind === "bond" ? "active-backup" : "",
    vlanId: null,
    vlanRawDevice: "",
    mtu: null,
    autostart: true,
    comments: "",
  };
}

export const validCidr4 = (c: string): boolean => c !== "dhcp" && validCtIp(c);

/** What's wrong with a spec, or null. Mirrors the backend's checks. */
export function ifaceProblem(s: PveIfaceSpec): string | null {
  if (!/^[A-Za-z][A-Za-z0-9_.-]{1,19}$/.test(s.iface)) return "Name: a letter, then letters, digits, . _ -";
  if (s.kind === "bridge" && !/^vmbr\d{1,4}$/.test(s.iface)) return "A Linux bridge is named vmbr plus a number (vmbr1).";
  if (s.cidr && !validCidr4(s.cidr)) return "IPv4: an address with prefix, like 192.168.1.20/24.";
  if (s.gateway && (!validIpv4(s.gateway) || !s.cidr)) return "Gateway: an IPv4 address, only with an address set.";
  if (s.kind === "bond" && !s.ports.trim()) return "A bond needs at least one slave.";
  if (s.kind === "vlan" && !/\.\d{1,4}$/.test(s.iface) && !(s.vlanId && s.vlanRawDevice)) return "Name a VLAN device.tag (vmbr0.20).";
  if (s.ports && !s.ports.split(/\s+/).filter(Boolean).every((p) => /^[A-Za-z][A-Za-z0-9_.-]{1,19}$/.test(p))) return "Ports: interface names separated by spaces.";
  if (s.mtu != null && (s.mtu < 1280 || s.mtu > 65520)) return "MTU: 1280 to 65520.";
  return null;
}

// ---- part 6: firewall ----

import type { PveFwNamed, PveFwRule, PveFwRuleSpec, PveFwScope } from "./bindings";
export type { PveFwNamed, PveFwRule, PveFwRuleSpec, PveFwScope };

export const FW_ACTIONS = ["ACCEPT", "DROP", "REJECT"];
export const FW_LOGS = ["", "nolog", "info", "notice", "warning", "err", "debug"];

export const scopeOf = (level: string, x: Partial<PveFwScope> = {}): PveFwScope => ({ level, node: "", kind: "", vmid: 0, group: "", ...x });

export function blankRule(): PveFwRuleSpec {
  return { kind: "in", action: "ACCEPT", macro: "", source: "", dest: "", proto: "", sport: "", dport: "", iface: "", log: "", enable: true, comment: "" };
}

export function ruleSpec(r: PveFwRule): PveFwRuleSpec {
  const { pos: _pos, ...rest } = r;
  void _pos;
  return rest;
}

/** One line for a rule: "IN ACCEPT SSH from +management". */
export function ruleText(r: PveFwRuleSpec): string {
  if (r.kind === "group") return `group ${r.action}`;
  const what = r.macro || [r.proto, r.dport && `port ${r.dport}`].filter(Boolean).join(" ") || "any";
  return [r.kind.toUpperCase(), r.action, what, r.source && `from ${r.source}`, r.dest && `to ${r.dest}`, r.iface && `on ${r.iface}`].filter(Boolean).join(" ");
}

/** Mirrors the backend's rule checks for inline feedback. */
export function ruleProblem(r: PveFwRuleSpec): string | null {
  if (r.kind === "group" ? !/^[A-Za-z][A-Za-z0-9_-]{1,17}$/.test(r.action) : !FW_ACTIONS.includes(r.action)) return "Pick an action or a security group.";
  const addr = /^[A-Za-z0-9.:/,+\-_!]*$/;
  if (!addr.test(r.source) || !addr.test(r.dest)) return "Source and destination: addresses, CIDRs, ranges, aliases or +ipsets.";
  if (!/^[A-Za-z0-9:,-]*$/.test(r.dport) || !/^[A-Za-z0-9:,-]*$/.test(r.sport)) return "Ports: numbers, ranges (1000:2000), lists or service names.";
  if ((r.dport || r.sport) && !r.proto && !r.macro) return "Ports need a protocol (tcp or udp).";
  if (r.comment.length > 200) return "Comment: up to 200 characters.";
  return null;
}
