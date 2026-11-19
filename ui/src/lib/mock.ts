/**
 * Browser-only stand-in for the Rust backend, used by `pnpm dev` without
 * Tauri, by tests and for screenshots. It keeps hosts in localStorage and
 * fakes terminals with a tiny echo shell. It never touches the network.
 */
import { normalizeIcon } from "./hosts/icons";
import type {
  Group_Serialize as Group,
  GroupInput_Serialize as GroupInput,
  Host_Serialize as Host,
  HostInput_Serialize as HostInput,
  HostPing_Serialize as HostPing,
  ImportResult,
  SshConfigCandidate_Serialize as SshConfigCandidate,
  TermMessage_Serialize as TermMessage,
  TermOpen,
} from "./bindings";

const KEY = "nightgrid.mock.hosts";

interface Db {
  hosts: Host[];
  groups: Group[];
  next: number;
  trusted: string[];
  secrets: string[];
}

function load(): Db {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return JSON.parse(raw) as Db;
  } catch {
    /* ignore */
  }
  return { hosts: [], groups: [], next: 1, trusted: [], secrets: [] };
}

let db = load();
const save = () => {
  try {
    localStorage.setItem(KEY, JSON.stringify(db));
  } catch {
    /* memory only */
  }
};

const toHost = (id: number, i: HostInput): Host => ({
  id,
  name: i.name.trim(),
  hostname: i.hostname.trim(),
  port: i.port ?? 22,
  username: i.username,
  groupId: i.groupId,
  color: i.color,
  icon: normalizeIcon(i.icon),
  favorite: i.favorite,
  notes: i.notes,
  authMethod: i.authMethod,
  identityFile: i.identityFile,
  proxyJump: i.proxyJump,
  rememberSecret: i.rememberSecret,
  sshAlias: i.sshAlias,
  tailnetAddress: i.tailnetAddress?.trim() || null,
  driftHistory: i.driftHistory,
  driftLookups: i.driftLookups ?? false,
  alertOffline: i.alertOffline ?? false,
  driftPaths: i.driftPaths ?? null,
  macAddress: i.macAddress?.trim() || null,
  wakeRelayId: i.wakeRelayId,
  sleepWindow: i.sleepWindow?.trim() || null,
  role: i.role === "client" ? "client" : "server",
  os: "",
  tags: [...new Set(i.tags.map((t) => t.trim()).filter(Boolean))],
});

const sorted = (hs: Host[]) =>
  [...hs].sort((a, b) => Number(b.favorite) - Number(a.favorite) || a.name.localeCompare(b.name));

// A fixed sample, so the import dialog has something to show in the browser.
const SAMPLE_CONFIG: Omit<SshConfigCandidate, "existingId">[] = [
  { alias: "lumen", hostname: "lumen.lan", user: "admin", port: null, identityFile: null, proxyJump: null },
  { alias: "nas", hostname: "nas.lan", user: "admin", port: 2222, identityFile: "~/.ssh/id_ed25519", proxyJump: null },
  { alias: "edge", hostname: "10.0.0.2", user: null, port: null, identityFile: null, proxyJump: "lumen" },
];

interface MockTerm {
  on: (m: TermMessage) => void;
  line: string;
  label: string;
}
const terms = new Map<number, MockTerm>();
let nextTerm = 1;
const enc = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));
const PROMPT = "\x1b[38;2;0;229;255m❯\x1b[0m ";

function startTerm(on: (m: TermMessage) => void, label: string, banner: string): number {
  const id = nextTerm++;
  terms.set(id, { on, line: "", label });
  queueMicrotask(() => {
    on({ kind: "status", state: "connected", attempt: 0, message: label });
    on({ kind: "data", b64: enc(banner + PROMPT) });
  });
  return id;
}

const fg = new Map<number, string | null>();

export const mockBackend = {
  async hostsList(): Promise<Host[]> {
    return sorted(db.hosts);
  },
  async hostCreate(i: HostInput): Promise<Host> {
    const h = toHost(db.next++, i);
    db.hosts.push(h);
    save();
    return h;
  },
  async hostUpdate(id: number, i: HostInput): Promise<Host> {
    const h = toHost(id, i);
    db.hosts = db.hosts.map((x) => (x.id === id ? h : x));
    save();
    return h;
  },
  async hostDelete(id: number): Promise<null> {
    db.hosts = db.hosts.filter((h) => h.id !== id);
    save();
    return null;
  },
  async groupsList(): Promise<Group[]> {
    return db.groups;
  },
  async groupCreate(i: GroupInput): Promise<Group> {
    const g: Group = { id: db.next++, name: i.name, color: i.color, icon: i.icon, sortOrder: i.sortOrder };
    db.groups.push(g);
    save();
    return g;
  },
  async groupDelete(id: number): Promise<null> {
    db.groups = db.groups.filter((g) => g.id !== id);
    db.hosts = db.hosts.map((h) => (h.groupId === id ? { ...h, groupId: null } : h));
    save();
    return null;
  },
  async sshConfigScan(): Promise<SshConfigCandidate[]> {
    return SAMPLE_CONFIG.map((c) => ({ ...c, existingId: db.hosts.find((h) => h.sshAlias === c.alias)?.id ?? null }));
  },
  async sshConfigImport(aliases: string[]): Promise<ImportResult> {
    let created = 0;
    let updated = 0;
    for (const c of SAMPLE_CONFIG.filter((c) => aliases.includes(c.alias))) {
      const existing = db.hosts.find((h) => h.sshAlias === c.alias);
      const fields = { hostname: c.hostname, port: c.port ?? 22, username: c.user, proxyJump: c.proxyJump };
      if (existing) {
        Object.assign(existing, fields);
        updated++;
      } else {
        db.hosts.push({
          ...toHost(db.next++, {
            name: c.alias,
            hostname: c.hostname,
            port: c.port,
            username: c.user,
            groupId: null,
            color: null,
            icon: null,
            favorite: false,
            notes: null,
            authMethod: c.identityFile ? "key" : "agent",
            identityFile: c.identityFile,
            proxyJump: c.proxyJump,
            rememberSecret: false,
            sshAlias: c.alias,
            tailnetAddress: null,
            driftHistory: false,
            alertOffline: false,
            driftLookups: false,
            driftPaths: null,
            macAddress: null,
            wakeRelayId: null,
            sleepWindow: null,
            role: "server",
            tags: ["ssh-config"],
          }),
        });
        created++;
      }
    }
    save();
    return { created, updated };
  },
  async hostsPing(ids: number[]): Promise<HostPing[]> {
    return ids.map((id) => {
      const h = db.hosts.find((x) => x.id === id);
      const off = h?.hostname.includes("offline") ?? false;
      const jump = !!h?.proxyJump;
      return {
        id,
        state: jump ? "unknown" : off ? "offline" : "online",
        latencyMs: jump || off ? null : 3 + (id % 9),
        connected: false,
      };
    });
  },

  async termOpenLocal(on: (m: TermMessage) => void): Promise<number> {
    return startTerm(on, "mock-shell", "NIGHTGRID browser preview: this is a mock shell (no real commands run).\r\n");
  },
  async termOpenSsh(hostId: number, on: (m: TermMessage) => void): Promise<TermOpen> {
    const h = db.hosts.find((x) => x.id === hostId);
    if (!h) return { kind: "needsUser", error: { kind: "other", message: "host not found" } };
    const key = {
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:q3Vn0mX7c1d2PZ9yKfW8bT5rL4hJ6sA0eG2uI1oN3xM",
      openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMockKeyForBrowserPreviewOnly",
    };
    if (h.hostname.includes("changed") && !db.trusted.includes(`!${h.hostname}`)) {
      return {
        kind: "needsUser",
        error: {
          kind: "hostKeyChanged",
          host: h.hostname,
          port: h.port,
          key,
          previous: [{ ...key, fingerprint: "SHA256:q3Vn0mX7c1d2PZ9yKfW8bT5rL4hJ6sA0eG2uI1oN3xQ" }],
        },
      };
    }
    if (!db.trusted.includes(h.hostname) && !h.hostname.includes("changed")) {
      return { kind: "needsUser", error: { kind: "hostKeyUnknown", host: h.hostname, port: h.port, key } };
    }
    const label = `${h.username ?? "you"}@${h.hostname}`;
    if (h.authMethod === "password" && !db.secrets.includes(label)) {
      return {
        kind: "needsUser",
        error: { kind: "passwordRequired", target: label, host: h.hostname, user: h.username ?? "you", retry: false },
      };
    }
    return { kind: "opened", id: startTerm(on, label, `Connected to ${h.name} (mock).\r\n`) };
  },
  async termForeground(id: number): Promise<string | null> {
    return fg.get(id) ?? null;
  },
  async termWrite(id: number, data: string): Promise<null> {
    const t = terms.get(id);
    if (!t) return null;
    let out = "";
    for (const ch of data) {
      if (ch === "\r") {
        const cmd = t.line.trim();
        // The last command "stays in front" so key hints can be previewed.
        fg.set(id, cmd || null);
        out += "\r\n" + (cmd ? `mock: ${cmd}\r\n` : "") + PROMPT;
        t.line = "";
      } else if (ch === "\x7f") {
        if (t.line) {
          t.line = t.line.slice(0, -1);
          out += "\b \b";
        }
      } else if (ch >= " ") {
        t.line += ch;
        out += ch;
      }
    }
    if (out) t.on({ kind: "data", b64: enc(out) });
    return null;
  },
  async termClose(id: number): Promise<null> {
    terms.delete(id);
    return null;
  },
  async trust(host: string): Promise<null> {
    db.trusted.push(host, `!${host}`);
    save();
    return null;
  },
  async provideSecret(target: string): Promise<null> {
    db.secrets.push(target);
    return null;
  },
  /** Test helper. */
  reset() {
    db = { hosts: [], groups: [], next: 1, trusted: [], secrets: [] };
    terms.clear();
  },
};
