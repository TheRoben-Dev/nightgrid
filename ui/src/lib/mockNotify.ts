/** Browser-only stand-in for the notifications inbox and ntfy (1.1). */
import type { Notice_Serialize as Notice, NtfyInfo_Serialize as NtfyInfo, NotifyTest_Serialize as NotifyTest } from "./bindings";

let next = 4;
const now = () => Math.floor(Date.now() / 1000);
let list: Notice[] = [
  { id: 3, at: now() - 600, source: "drill", level: "critical", title: "Restore drill failed · server01", body: "restic: no snapshot newer than 8 days", hostId: null, view: "fleet/drills", read: false },
  { id: 2, at: now() - 5400, source: "posture", level: "warn", title: "Security score fell on nas: 82 → 64", body: "Firewall is off; 2 database ports listen on every address.", hostId: null, view: "security/posture", read: false },
  { id: 1, at: now() - 86400, source: "alert", level: "ok", title: "Resolved: server01 · disk almost full", body: "/ is at 71%", hostId: null, view: "overview", read: true },
];
let ntfy: NtfyInfo = { configured: false, url: null, hasToken: false, public: false };

export const mockNotify = {
  async list(limit: number): Promise<Notice[]> {
    return list.slice(0, limit);
  },
  async read(ids: number[] | null): Promise<null> {
    list = list.map((n) => (ids === null || ids.includes(n.id) ? { ...n, read: true } : n));
    return null;
  },
  async clear(): Promise<null> {
    list = [];
    return null;
  },
  async ntfyGet(): Promise<NtfyInfo> {
    return ntfy;
  },
  async ntfySet(url: string | null, token: string | null): Promise<NtfyInfo> {
    if (url === null) ntfy = { configured: false, url: null, hasToken: false, public: false };
    else {
      if (url) ntfy = { ...ntfy, configured: true, url };
      if (token !== null) ntfy = { ...ntfy, hasToken: token !== "" };
    }
    return ntfy;
  },
  async test(): Promise<NotifyTest> {
    list = [{ id: next++, at: now(), source: "system", level: "info", title: "Test", body: "Notifications from Nightgrid reach you here.", hostId: null, view: "settings", read: false }, ...list];
    return { native: "sent", ntfy: ntfy.configured ? "delivered (HTTP 200)" : null };
  },
};
