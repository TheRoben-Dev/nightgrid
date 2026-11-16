/**
 * Browser-only stand-in for the .env vault (M10w): variables per host and
 * stack kept in memory, plus a fake host `.env` for import and deploy.
 */
import type { EnvDeployed_Serialize as EnvDeployed, EnvDiff, EnvVar, EnvVarInput_Deserialize as EnvVarInput, Stack } from "./bindings";
import { looksSecret, validName } from "./envvault";

interface Row extends EnvVar {
  value: string;
}

let nextId = 1;
let rows: Row[] = [];
/** The host side: `${hostId}|${dir}` → .env contents. */
const hostFiles = new Map<string, Map<string, string>>();

const now = () => Math.floor(Date.now() / 1000);
const key = (hostId: number, s: Stack) => `${hostId}|${s.dir}`;
const strip = ({ value: _v, ...v }: Row): EnvVar => v;
const of = (hostId: number, stack: string) => rows.filter((r) => r.hostId === hostId && r.stack === stack).sort((a, b) => a.position - b.position);

function seedHost(hostId: number, s: Stack): Map<string, string> {
  const k = key(hostId, s);
  let f = hostFiles.get(k);
  if (!f) {
    f = new Map([
      ["TZ", "Europe/Stockholm"],
      ["PUID", "1000"],
      ["DB_PASSWORD", "correct-horse-battery"],
      ["API_TOKEN", "tok_example_123"],
    ]);
    hostFiles.set(k, f);
  }
  return f;
}

function set(hostId: number, stack: string, oldName: string | null, input: EnvVarInput): EnvVar {
  const name = input.name.trim();
  if (!validName(name)) throw new Error(`"${name}" isn't a valid variable name (letters, digits and _, not starting with a digit)`);
  const list = of(hostId, stack);
  const existing = list.find((r) => r.name === (oldName ?? name));
  if (list.some((r) => r.name === name && r !== existing)) throw new Error(`${name} is already in this stack's vault`);
  if (existing) {
    existing.name = name;
    existing.secret = input.secret;
    if (input.value != null) existing.value = input.value;
    existing.updatedAt = now();
    return strip(existing);
  }
  if (input.value == null) throw new Error("a new variable needs a value");
  const r: Row = {
    id: nextId++,
    hostId,
    stack,
    name,
    secret: input.secret,
    locked: false,
    keyId: "3f9a0c2e71b4d865",
    position: list.length,
    updatedAt: now(),
    value: input.value,
  };
  rows.push(r);
  return strip(r);
}

export const mockEnvVault = {
  list: async (hostId: number, stack: string): Promise<EnvVar[]> => of(hostId, stack).map(strip),
  stacks: async (): Promise<[number, string, number][]> => {
    const m = new Map<string, [number, string, number]>();
    for (const r of rows) {
      const k = `${r.hostId}|${r.stack}`;
      const e = m.get(k) ?? [r.hostId, r.stack, 0];
      e[2]++;
      m.set(k, e);
    }
    return [...m.values()];
  },
  set: async (hostId: number, stack: string, oldName: string | null, input: EnvVarInput): Promise<EnvVar> => set(hostId, stack, oldName, input),
  remove: async (id: number): Promise<null> => {
    rows = rows.filter((r) => r.id !== id);
    return null;
  },
  reveal: async (id: number): Promise<string> => {
    const r = rows.find((x) => x.id === id);
    if (!r) throw new Error("no such variable");
    return r.value;
  },
  order: async (hostId: number, stack: string, ids: number[]): Promise<null> => {
    ids.forEach((id, i) => {
      const r = rows.find((x) => x.id === id && x.hostId === hostId && x.stack === stack);
      if (r) r.position = i;
    });
    return null;
  },
  importHost: async (hostId: number, s: Stack): Promise<number> => {
    const f = seedHost(hostId, s);
    for (const [name, value] of f) {
      const old = of(hostId, s.name).find((r) => r.name === name);
      set(hostId, s.name, null, { name, value, secret: old ? old.secret : looksSecret(name) });
    }
    return f.size;
  },
  preview: async (hostId: number, s: Stack): Promise<EnvDiff> => {
    const exists = hostFiles.has(key(hostId, s));
    const cur = hostFiles.get(key(hostId, s)) ?? new Map<string, string>();
    const next = of(hostId, s.name);
    const names = new Set(next.map((r) => r.name));
    return {
      exists,
      added: next.filter((r) => !cur.has(r.name)).map((r) => r.name),
      changed: next.filter((r) => cur.has(r.name) && cur.get(r.name) !== r.value).map((r) => r.name),
      removed: [...cur.keys()].filter((n) => !names.has(n)),
      unchanged: next.filter((r) => cur.get(r.name) === r.value).length,
    };
  },
  deploy: async (hostId: number, s: Stack): Promise<EnvDeployed> => {
    hostFiles.set(key(hostId, s), new Map(of(hostId, s.name).map((r) => [r.name, r.value])));
    return { path: `${s.dir}/.env`, snapshot: true, snapshotError: null };
  },
};
