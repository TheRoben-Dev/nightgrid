/** Browser-only stand-in for quest checks (M12e): a believable half-done host. */
import type { Quest_Serialize as Quest, QuestResult } from "./bindings";

const PASS = new Set(["ssh-key", "docker", "docker-group", "time-sync", "swap", "container:caddy", "port:443", "port:53", "no-reboot-pending", "tailscale"]);
const UNKNOWN = new Set(["ssh-no-root-password"]);

export const mockQuest = {
  async check(checks: string[]): Promise<QuestResult[]> {
    await new Promise((r) => setTimeout(r, 400));
    return [...new Set(checks)].map((id) =>
      PASS.has(id)
        ? { id, state: "pass", detail: id.startsWith("container:") ? `${id.slice(10)} running` : "ok" }
        : UNKNOWN.has(id)
          ? { id, state: "unknown", detail: "/etc/ssh/sshd_config.d/50-cloud-init.conf needs root to read" }
          : { id, state: "fail", detail: id === "ssh-no-password" ? "passwordauthentication yes (default)" : "not set up" },
    );
  },
};

/** Browser-only quest log (1.4): kept in localStorage like the real one is kept in the database. */
const LOG_KEY = "nightgrid.mock.quests";
function load(): Quest[] {
  try {
    const v = JSON.parse(localStorage.getItem(LOG_KEY) ?? "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}
function store(qs: Quest[]) {
  try {
    localStorage.setItem(LOG_KEY, JSON.stringify(qs));
  } catch {
    /* session only */
  }
}
const nowS = () => Math.floor(Date.now() / 1000);
const order = (qs: Quest[]) => [...qs].sort((a, b) => Number(a.completedAt != null) - Number(b.completedAt != null) || (b.updatedAt ?? 0) - (a.updatedAt ?? 0));

export const mockQuestLog = {
  async list(): Promise<Quest[]> {
    return order(load());
  },
  async start(articleId: string, hostId: number | null, title: string, ticks: string[]): Promise<Quest> {
    const qs = load();
    const hit = qs.find((q) => q.articleId === articleId && q.hostId === hostId);
    if (hit) {
      hit.completedAt = null;
      hit.updatedAt = nowS();
      store(qs);
      return hit;
    }
    const q: Quest = { id: Math.max(0, ...qs.map((x) => x.id)) + 1, articleId, hostId, title, ticks: [...new Set(ticks)].sort(), startedAt: nowS(), updatedAt: nowS(), completedAt: null };
    store([...qs, q]);
    return q;
  },
  async ticks(id: number, ticks: string[], complete: boolean): Promise<Quest> {
    const qs = load();
    const q = qs.find((x) => x.id === id);
    if (!q) throw new Error("no such quest");
    q.ticks = [...new Set(ticks)].sort();
    q.updatedAt = nowS();
    q.completedAt = complete ? (q.completedAt ?? nowS()) : null;
    store(qs);
    return q;
  },
  async abandon(id: number): Promise<boolean> {
    const qs = load();
    store(qs.filter((q) => q.id !== id));
    return qs.some((q) => q.id === id);
  },
};
