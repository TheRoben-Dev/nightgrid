/** Starter blueprints and runbooks (D-177): form values, wording and runbook notes. */
import type { AlertKind, Note, NoteInput, StarterBlueprint, StarterKind, StarterRunbook, StarterValue, StarterVar } from "./api";

/** Library topic ids used by starters, as shown on the cards. */
export const CATEGORY_LABEL: Record<string, string> = {
  start: "Getting started",
  docker: "Docker",
  media: "Media",
  network: "Network",
  storage: "Storage",
  monitoring: "Monitoring",
  security: "Security",
  linux: "Linux",
  hardware: "Hardware",
  dev: "Development",
  smarthome: "Smart home",
  gaming: "Gaming",
  productivity: "Productivity",
  alerts: "Alerts",
  logs: "Logs",
};

export const categoryLabel = (c: string): string => CATEGORY_LABEL[c] ?? c;

/** `stack/NAME`, the key a value is sent under. */
export const valueKey = (stack: string, v: StarterVar): string => `${stack}/${v.name}`;

/** The form's starting values: defaults, and empty for generated secrets. */
export function initialValues(s: StarterBlueprint): Record<string, string> {
  const out: Record<string, string> = {};
  for (const st of s.stacks) for (const v of st.env) out[valueKey(st.name, v)] = v.default;
  for (const v of s.vars) out[valueKey(VARS, v)] = v.default;
  return out;
}

/** The key group of a starter's `{{vars}}` (timers, firewall). */
export const VARS = "vars";

/** A `{{var}}` value: one word, since it goes on a command line. */
export const wordOk = (v: string): boolean => /^[A-Za-z0-9/._:@+-]+$/.test(v);

/** Gallery shelves, in order. */
export const KINDS: { id: StarterKind; label: string }[] = [
  { id: "setup", label: "SETUPS" },
  { id: "app", label: "APPS" },
  { id: "jobs", label: "JOBS" },
  { id: "firewall", label: "FIREWALL" },
];

/** What still needs filling in, in form order (null when ready). Mirrors the app's checks. */
export function valuesProblem(s: StarterBlueprint, values: Record<string, string>, parent: string): string | null {
  if (s.stacks.length && !/^\/[^'"`$\\]*$/.test(parent.trim())) return "Pick an absolute folder for the stacks, such as /opt/stacks.";
  for (const st of s.stacks) {
    for (const v of st.env) {
      const val = (values[valueKey(st.name, v)] ?? "").trim();
      if (!val && !v.default && !v.generate) return `Fill in “${v.label}”.`;
      if ([...val].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127)) return `“${v.label}” must be one line.`;
      if (val && v.path && !val.startsWith("/")) return `“${v.label}” must be an absolute folder such as /srv/data.`;
    }
  }
  for (const v of s.vars) {
    const val = (values[valueKey(VARS, v)] ?? "").trim();
    if (!val && !v.default && !v.generate) return `Fill in “${v.label}”.`;
    if (val && !wordOk(val)) return `“${v.label}” must be one word: letters, digits and / . _ : @ + -`;
  }
  return null;
}

/** Values to send: only what was typed (empty ones fall back to the default or are generated). */
export function toValues(values: Record<string, string>): StarterValue[] {
  return Object.entries(values)
    .map(([key, value]) => ({ key, value: value.trim() }))
    .filter((v) => v.value !== "");
}

/** The firewall rules a starter adds, as words. */
export function firewallWords(s: StarterBlueprint): string {
  const f = s.firewall;
  if (!f) return "";
  return [...f.allowFrom.map((a) => `allow from ${a}`), ...f.allow.map((p) => `allow ${p}`), ...f.limit.map((p) => `limit ${p}`)].join(", ") || "no rules";
}

/** The card's last line: containers, firewall, timers. */
export function cardMeta(s: StarterBlueprint): string {
  const n = s.stacks.flatMap((st) => images(st.compose)).length;
  const parts: string[] = [];
  if (n) parts.push(`${n} container${n === 1 ? "" : "s"}`);
  if (s.firewall) parts.push("firewall");
  if (s.timers.length) parts.push(`${s.timers.length} timer${s.timers.length === 1 ? "" : "s"}`);
  return parts.join(" · ");
}

/** Ports a starter opens, split by who can reach them. */
export function portsOf(s: StarterBlueprint): { lan: string[]; local: string[] } {
  const lan: string[] = [];
  const local: string[] = [];
  for (const st of s.stacks) for (const p of st.ports) (p.local ? local : lan).push(`${p.what} ${p.port}`);
  return { lan, local };
}

/** The image names in a stack's compose file, for the card. */
export function images(compose: string): string[] {
  return [...compose.matchAll(/^\s*image:\s*["']?([^\s"'#]+)/gm)].map((m) => m[1]);
}

/** The confirmation sentence shown before anything is applied. */
export function confirmLine(count: number, host: string): string {
  return `Apply ${count} step${count === 1 ? "" : "s"} to ${host}? Files are written and stacks started there; nothing on the host is deleted.`;
}

/** The note a starter runbook becomes. */
export function runbookInput(r: StarterRunbook): NoteInput {
  return {
    title: r.title,
    body: r.body,
    hostId: null,
    tags: r.tags,
    pinned: false,
    hideFromDrift: false,
    runbook: true,
    alertKinds: r.alertKinds,
    serviceId: null,
  };
}

/** A starter runbook already added (a note with its title). */
export const runbookAdded = (notes: Note[], r: StarterRunbook): Note | undefined => notes.find((n) => n.title.trim() === r.title.trim());

/** Starter runbooks for an alert kind that aren't added yet. */
export function startersFor(kind: AlertKind, runbooks: StarterRunbook[], notes: Note[]): StarterRunbook[] {
  return runbooks.filter((r) => r.alertKinds.includes(kind) && !runbookAdded(notes, r));
}
