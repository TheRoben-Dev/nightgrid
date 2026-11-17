/**
 * DRIFT's read-only checks on a host (D-151): which checks a question calls
 * for, how their results go into the prompt, and the `[[check …]]` lines the
 * model may end a reply with to ask for more. The commands themselves are
 * fixed on the Rust side (core/src/helper/lookup.rs); this only names kinds.
 * Pure, so it can be tested.
 */
import type { LookupKind, Lookup_Serialize as Lookup, LookupResult_Serialize as LookupResult } from "../bindings";

/** The names the model uses in `[[check …]]` lines, and what each shows. */
export const CHECKS: { name: string; kind: LookupKind; target?: "name" | "path"; what: string }[] = [
  { name: "overview", kind: "overview", what: "uptime, load, disk and memory" },
  { name: "system", kind: "system", what: "OS, kernel, pending reboot" },
  { name: "disk", kind: "disk", what: "disk and inode use" },
  { name: "storage", kind: "storage", what: "drives, RAID and ZFS health" },
  { name: "mounts", kind: "mounts", what: "mounted filesystems and shares" },
  { name: "memory", kind: "memory", what: "memory and the biggest processes" },
  { name: "processes", kind: "processes", what: "load and the busiest processes" },
  { name: "sensors", kind: "sensors", what: "temperatures, fans, GPU" },
  { name: "ports", kind: "ports", what: "listening ports" },
  { name: "network", kind: "network", what: "addresses and routes" },
  { name: "connections", kind: "connections", what: "open connections and top peers" },
  { name: "services", kind: "services", what: "running services" },
  { name: "failed-units", kind: "failedUnits", what: "failed systemd units" },
  { name: "service", kind: "service", target: "name", what: "one service's status" },
  { name: "service-logs", kind: "serviceLogs", target: "name", what: "one service's recent journal" },
  { name: "errors", kind: "errors", what: "error lines in the journal (24 h)" },
  { name: "kernel", kind: "kernelLog", what: "kernel warnings (OOM, disk, network)" },
  { name: "timers", kind: "timers", what: "systemd timers" },
  { name: "cron", kind: "cron", what: "cron jobs" },
  { name: "logins", kind: "logins", what: "who is logged in, recent logins" },
  { name: "updates", kind: "updates", what: "pending package updates" },
  { name: "containers", kind: "containers", what: "all containers and their status" },
  { name: "container", kind: "container", target: "name", what: "one container's state, restarts, ports, mounts" },
  { name: "container-logs", kind: "containerLogs", target: "name", what: "one container's recent logs" },
  { name: "docker-stats", kind: "dockerStats", what: "CPU and memory per container" },
  { name: "docker-events", kind: "dockerEvents", what: "container restarts, deaths, health (1 h)" },
  { name: "docker-disk", kind: "dockerDisk", what: "space used by images and volumes" },
  { name: "compose", kind: "composeProjects", what: "compose projects and their files" },
  { name: "file", kind: "file", target: "path", what: "a config or log file (secrets masked)" },
];

const byKind = new Map(CHECKS.map((c) => [c.kind, c]));
const byName = new Map(CHECKS.map((c) => [c.name, c]));

export const checkName = (k: LookupKind) => byKind.get(k)?.name ?? k;

const L = (kind: LookupKind, target: string | null = null): Lookup => ({ kind, target });

/** Words → checks. Each rule adds its kinds when any word matches. */
const RULES: [RegExp, LookupKind[]][] = [
  [/\b(disk|disks|space|full|storage|inode|inodes|df)\b/, ["disk", "storage"]],
  [/\b(raid|zfs|zpool|pool|mdadm|btrfs|drive|drives|nvme|ssd|hdd|smart|degraded)\b/, ["storage", "kernelLog"]],
  [/\b(mount|mounts|mounted|nfs|smb|cifs|share|shares)\b/, ["mounts"]],
  [/\b(memory|ram|oom|swap|swapping|leak)\b/, ["memory", "kernelLog"]],
  [/\b(cpu|load|slow|sluggish|busy|lag|lagging|process|processes|hog)\b/, ["processes"]],
  [/\b(temp|temps|temperature|hot|overheat\w*|fan|fans|thermal|gpu)\b/, ["sensors"]],
  [/\b(port|ports|listen\w*)\b/, ["ports"]],
  [/\b(network|ip|ips|route|routes|routing|interface|interfaces|gateway|dns)\b/, ["network"]],
  [/\b(connections?|traffic|bandwidth|peers?|ddos)\b/, ["connections"]],
  [/\b(docker|containers?)\b/, ["containers", "dockerStats"]],
  [/\b(restart\w*|crash\w*|died|dies|exited|killed|kill\w*|down|unhealthy)\b/, ["dockerEvents", "kernelLog", "failedUnits"]],
  [/\b(log|logs|errors?|fail\w*|warnings?|journal)\b/, ["errors", "failedUnits"]],
  [/\b(services?|systemd|units?|daemons?)\b/, ["services", "failedUnits"]],
  [/\b(timers?|cron\w*|schedul\w*|jobs?)\b/, ["timers", "cron"]],
  [/\b(login|logins|logged|ssh|users?|who|brute\w*|intrusion|hack\w*)\b/, ["logins"]],
  [/\b(updates?|upgrades?|upgradable|packages?|patch\w*|apt|dnf)\b/, ["updates", "system"]],
  [/\b(reboot\w*|kernel|os|version|distro|uptime)\b/, ["system"]],
  [/\b(compose|stacks?)\b/, ["composeProjects"]],
  [/\b(images?|volumes?|prune)\b/, ["dockerDisk"]],
];

/** What a broad "how is it doing" question gets. */
const GENERAL: LookupKind[] = ["overview", "failedUnits", "errors", "containers", "dockerEvents", "kernelLog"];

const PATH = /(?:^|[\s"'`(])((?:\/|~\/)[A-Za-z0-9_./@+~-]*[A-Za-z0-9_@+~-])/g;

/** File paths named in a question. */
export function pathsIn(q: string): string[] {
  const out: string[] = [];
  for (const m of q.matchAll(PATH)) if (!out.includes(m[1]) && m[1].length > 2) out.push(m[1]);
  return out.slice(0, 2);
}

const HOWTO = /\b(how (do|can|to|should) i|where (is|do|can)|in nightgrid|nightgrid's|which view|what does .* (button|view|tab))\b/;
const TROUBLE = /\b(why|wrong|broken|down|fail\w*|error\w*|slow|full|crash\w*|status|health\w*|going on|happen\w*|check|look|running)\b/;

/** Whether a question about a host wants live checks (not a how-to about Nightgrid). */
export function wantsLookups(q: string): boolean {
  const t = q.toLowerCase();
  return !HOWTO.test(t) || TROUBLE.test(t.replace(HOWTO, ""));
}

/** The checks a question calls for (the Rust side adds named containers and services). */
export function planLookups(q: string): Lookup[] {
  const text = q.toLowerCase();
  const kinds: LookupKind[] = [];
  for (const [re, ks] of RULES) if (re.test(text)) for (const k of ks) if (!kinds.includes(k)) kinds.push(k);
  const out = pathsIn(q).map((p) => L("file", p));
  // Nothing specific: a broad look at what's going on.
  for (const k of kinds.length ? kinds : GENERAL) out.push(L(k));
  return out.slice(0, 8);
}

const CHECK_LINE = /^\s*\[\[\s*check\s+([a-z-]+)(?:\s+([^\]\s]+))?\s*\]\]\s*$/i;

/**
 * Splits `[[check KIND target]]` lines off a reply: what the model wants to
 * look at next (at most three), and the reply without them.
 */
export function takeCheckRequests(reply: string): { text: string; asked: Lookup[] } {
  const asked: Lookup[] = [];
  const keep: string[] = [];
  for (const line of reply.split("\n")) {
    const m = CHECK_LINE.exec(line);
    if (!m) {
      keep.push(line);
      continue;
    }
    const c = byName.get(m[1].toLowerCase());
    if (!c || asked.length >= 3) continue;
    const target = m[2] ?? null;
    if (c.target && !target) continue;
    const l = L(c.kind, c.target ? target : null);
    if (!asked.some((a) => a.kind === l.kind && a.target === l.target)) asked.push(l);
  }
  return { text: keep.join("\n").trim(), asked };
}

/** Results already in hand, so a follow-up doesn't run the same check twice. */
export function notYetRun(asked: Lookup[], done: LookupResult[]): Lookup[] {
  return asked.filter((a) => !done.some((d) => d.lookup.kind === a.kind && (d.lookup.target ?? null) === (a.target ?? null)));
}

/** A one-line label: `container-logs web`. */
export function lookupLabel(l: Lookup): string {
  return `${checkName(l.kind)}${l.target ? ` ${l.target}` : ""}`;
}

const TAIL_KINDS: LookupKind[] = ["containerLogs", "serviceLogs", "errors", "kernelLog", "dockerEvents"];

/** Characters of check output one host's block may hold, shared between its checks. */
export const BLOCK_CHARS = 5400;

/** Cuts one output to `max` characters at a line break (logs keep their newest lines). */
function fitOutput(r: LookupResult, max: number): string {
  if (r.output.length <= max) return r.output;
  if (TAIL_KINDS.includes(r.lookup.kind)) {
    const s = r.output.slice(-max);
    const nl = s.indexOf("\n");
    return `[older lines cut]\n${nl >= 0 ? s.slice(nl + 1) : s}`;
  }
  const s = r.output.slice(0, max);
  const nl = s.lastIndexOf("\n");
  return `${nl > 0 ? s.slice(0, nl) : s}\n[cut to fit]`;
}

/** The prompt block for one host's check results; every check gets a fair share of the space. */
export function lookupBlock(host: string, results: LookupResult[]): string {
  const share = Math.max(300, Math.floor(BLOCK_CHARS / Math.max(1, results.length)));
  const parts = results.map((r) => {
    const head = `#### ${lookupLabel(r.lookup)}${r.command ? ` (\`${r.command.length > 120 ? `${r.command.slice(0, 117)}…` : r.command}\`)` : ""}`;
    if (!r.ok) return `${head}\nCouldn't check: ${r.output}`;
    return `${head}${r.clipped ? " [cut to fit]" : ""}\n\`\`\`\n${fitOutput(r, share)}\n\`\`\``;
  });
  return `### ${host}\n${parts.join("\n")}`;
}

/** How to ask for more, added on the first round only. */
export function followUpHint(): string {
  return `If these don't answer the question, you may end your reply with up to 2 lines like [[check KIND]] or [[check KIND NAME]] (NAME is a container, a service or a file path) and I will run them and you answer again. Checks: ${CHECKS.map((c) => `${c.name}${c.target === "name" ? " NAME" : c.target === "path" ? " PATH" : ""}`).join(", ")}. Only ask when you need it.`;
}
