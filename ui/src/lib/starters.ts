/** Starter blueprints and runbooks (D-175): form values, wording and runbook notes. */
import type { AlertKind, Note, NoteInput, StarterBlueprint, StarterRunbook, StarterValue, StarterVar } from "./api";

/** Library topic ids used by starters, as shown on the cards. */
export const CATEGORY_LABEL: Record<string, string> = {
  start: "Getting started",
  docker: "Docker",
  media: "Media",
  network: "Network",
  storage: "Storage",
  monitoring: "Monitoring",
  security: "Security",
  smarthome: "Smart home",
  gaming: "Gaming",
  productivity: "Productivity",
};

export const categoryLabel = (c: string): string => CATEGORY_LABEL[c] ?? c;

/** `stack/NAME`, the key a value is sent under. */
export const valueKey = (stack: string, v: StarterVar): string => `${stack}/${v.name}`;

/** The form's starting values: defaults, and empty for generated secrets. */
export function initialValues(s: StarterBlueprint): Record<string, string> {
  const out: Record<string, string> = {};
  for (const st of s.stacks) for (const v of st.env) out[valueKey(st.name, v)] = v.default;
  return out;
}

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
  return null;
}

/** Values to send: only what was typed (empty ones fall back to the default or are generated). */
export function toValues(values: Record<string, string>): StarterValue[] {
  return Object.entries(values)
    .map(([key, value]) => ({ key, value: value.trim() }))
    .filter((v) => v.value !== "");
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
