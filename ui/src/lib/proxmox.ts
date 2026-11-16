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
