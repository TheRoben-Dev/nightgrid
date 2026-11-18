/** Browser-only stand-in for Security → SECRETS (1.1). */
import type { KeyHost_Serialize as KeyHost, ManualSecret_Deserialize as ManualSecret, SecretItem_Serialize as SecretItem } from "./bindings";
import { mockBackend } from "./mock";

const DAY = 86_400;
const now = () => Math.floor(Date.now() / 1000);
const stamps = new Map<string, number>();
const policies = new Map<string, number | null>();
let manual: (ManualSecret & { id: number })[] = [{ id: 1, name: "Router admin password", place: "router web UI", hostId: null, rotatedAt: now() - 420 * DAY, note: "" }];

function item(key: string, kind: SecretItem["kind"], name: string, place: string, age: number | null, defDays: number | null, note = "", hostId: number | null = null): SecretItem {
  const rotatedAt = stamps.get(key) ?? (age == null ? null : now() - age * DAY);
  const p = policies.has(key) ? policies.get(key)! : defDays;
  const policyDays = p === 0 ? null : p;
  const dueAt = policyDays && rotatedAt ? rotatedAt + policyDays * DAY : null;
  const state: SecretItem["state"] = policyDays == null ? "noPolicy" : dueAt == null ? "unknown" : dueAt <= now() ? "overdue" : dueAt - now() <= 30 * DAY ? "dueSoon" : "ok";
  return { key, kind, name, place, hostId, rotatedAt, policyDays, dueAt, state, note };
}

const rank: Record<SecretItem["state"], number> = { overdue: 0, dueSoon: 1, unknown: 2, ok: 3, noPolicy: 4 };

export const mockSecrets = {
  async list(): Promise<SecretItem[]> {
    const hs = await mockBackend.hostsList();
    const first = hs[0]?.name ?? "server01";
    const v: SecretItem[] = [
      item("ssh:SHA256:mockA", "sshKey", "id_ed25519", hs.slice(0, 3).map((h) => h.name).join(", "), 500, 365, "ssh-ed25519 · passphrase"),
      item("ssh:SHA256:mockB", "sshKey", "nightgrid_ed25519", first, 40, 365, "ssh-ed25519 · no passphrase"),
      item("vault:1", "vault", "POSTGRES_PASSWORD", `immich on ${first}`, 350, 365, "", hs[0]?.id ?? null),
      item("integration:1", "integration", "Pi-hole", "pihole", 90, 180),
      item("app:mcp", "appToken", "MCP server token", "Settings → MCP", null, 180),
      ...manual.map((m) => item(`manual:${m.id}`, "manual", m.name, m.place, m.rotatedAt == null ? null : Math.round((now() - m.rotatedAt) / DAY), 365, m.note, m.hostId)),
      { key: "expiry:1", kind: "expiry", name: "example.org", place: first, hostId: null, rotatedAt: null, policyDays: null, dueAt: now() + 12 * DAY, state: "dueSoon", note: "certificate" },
    ];
    return v.sort((a, b) => rank[a.state] - rank[b.state] || (a.dueAt ?? Infinity) - (b.dueAt ?? Infinity));
  },
  async markRotated(key: string): Promise<null> {
    stamps.set(key, now());
    return null;
  },
  async policy(key: string, days: number | null): Promise<null> {
    if (days == null) policies.delete(key);
    else policies.set(key, days);
    return null;
  },
  async save(m: ManualSecret): Promise<number> {
    if (m.id != null) {
      manual = manual.map((x) => (x.id === m.id ? { ...m, id: x.id } : x));
      return m.id;
    }
    const id = Math.max(0, ...manual.map((x) => x.id)) + 1;
    manual.push({ ...m, id });
    return id;
  },
  async remove(id: number): Promise<null> {
    manual = manual.filter((x) => x.id !== id);
    return null;
  },
  async keyHosts(fingerprint: string): Promise<KeyHost[]> {
    const hs = await mockBackend.hostsList();
    return hs.filter((h) => h.role !== "client").map((h, i) => ({ hostId: h.id, name: h.name, authorized: fingerprint.endsWith("A") ? i < 3 : i === 0, error: null }));
  },
};
