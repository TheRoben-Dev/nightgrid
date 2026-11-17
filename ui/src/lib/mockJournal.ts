/** Browser-only stand-in for the journal (0.10.81): a few weeks of changes. */
import type { ChangeArea, ChangeEntry_Serialize as ChangeEntry, ChangeQuery_Deserialize as ChangeQuery } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
const H = 3600;
let nextId = 100;

const seed = (): ChangeEntry[] => {
  const t = now();
  const mk = (ago: number, hostId: number | null, host: string, area: ChangeArea, title: string, detail = "", diff: string | null = null, ok = true, manual = false): ChangeEntry => ({
    id: nextId++,
    at: t - ago,
    hostId,
    host,
    area,
    title,
    detail,
    diff,
    ok,
    manual,
  });
  return [
    mk(0.4 * H, 1, "server01", "stack", "Deployed media", "/srv/media: pulled 2 images, recreated jellyfin"),
    mk(1.2 * H, 1, "server01", "proxy", "Caddyfile: added photos.example.lan", "/etc/caddy/Caddyfile (on the host)", "@@ 12 unchanged\n }\n+\n+photos.example.lan {\n+\treverse_proxy 127.0.0.1:2342\n+}"),
    mk(3 * H, 2, "nas", "jobs", "Crontab: add (root's crontab)", "", "@@ 4 unchanged\n 0 2 * * * /usr/local/bin/snap.sh\n+30 3 * * 0 /usr/local/bin/scrub.sh"),
    mk(5 * H, 1, "server01", "firewall", "Firewall: allow 8443/tcp from 192.168.1.0/24", "applied with a 120 s rollback armed"),
    mk(5 * H - 60, 1, "server01", "firewall", "Firewall change kept"),
    mk(26 * H, null, "", "note", "Swapped the UPS battery", "Old one was 3 years old; runtime back to 22 min.", null, true, true),
    mk(27 * H, 1, "server01", "system", "Upgraded 14 packages", "apt: openssl, curl, linux-image… reboot needed"),
    mk(28 * H, 1, "server01", "power", "Reboot", "from Nightgrid"),
    mk(50 * H, 2, "nas", "file", "Saved /etc/exports", "", "@@ 2 unchanged\n-/tank/media 192.168.1.0/24(ro)\n+/tank/media 192.168.1.0/24(rw,no_subtree_check)"),
    mk(52 * H, 1, "server01", "secrets", ".env vault: changed DB_PASSWORD", "media"),
    mk(75 * H, 1, "server01", "stack", "Compose up failed: photos", "image pull failed: manifest unknown", null, false),
    mk(76 * H, 1, "server01", "container", "Restarted jellyfin", "from Nightgrid"),
    mk(9 * 24 * H, 3, "pve01", "proxmox", "Proxmox: snapshot before-upgrade of 101", "qemu 101 on pve01"),
    mk(20 * 24 * H, null, "", "integrations", "Added integration Pi-hole"),
  ];
};

let entries: ChangeEntry[] | null = null;
const all = () => (entries ??= seed());

function matches(e: ChangeEntry, q: ChangeQuery): boolean {
  if (q.hostId != null && e.hostId !== q.hostId) return false;
  if (q.since != null && e.at < q.since) return false;
  if (q.until != null && e.at > q.until) return false;
  if (q.failedOnly && e.ok) return false;
  if (q.areas?.length && !q.areas.includes(e.area)) return false;
  const hay = `${e.title} ${e.detail} ${e.host} ${e.diff ?? ""}`.toLowerCase();
  return (q.text ?? "").toLowerCase().split(/\s+/).filter(Boolean).every((w) => hay.includes(w));
}

export const mockJournal = {
  async query(q: ChangeQuery): Promise<ChangeEntry[]> {
    return all()
      .filter((e) => matches(e, q))
      .sort((a, b) => b.at - a.at)
      .slice(0, q.limit || 1000);
  },
  async stats() {
    const a = all();
    return { count: a.length, oldest: a.length ? Math.min(...a.map((e) => e.at)) : null };
  },
  async note(hostId: number | null, title: string, detail: string, at: number | null): Promise<ChangeEntry> {
    const e: ChangeEntry = { id: nextId++, at: at ?? now(), hostId, host: hostId ? `host ${hostId}` : "", area: "note", title, detail, diff: null, ok: true, manual: true };
    all().push(e);
    return e;
  },
  async edit(id: number, title: string, detail: string): Promise<null> {
    const e = all().find((x) => x.id === id && x.manual);
    if (!e) throw new Error("only entries you wrote can be edited");
    e.title = title;
    e.detail = detail;
    return null;
  },
  async delete(id: number): Promise<null> {
    entries = all().filter((x) => !(x.id === id && x.manual));
    return null;
  },
  async clear(): Promise<number> {
    const n = all().length;
    entries = [];
    return n;
  },
  async forDrift(hostIds: number[], since: number, max: number): Promise<string> {
    return all()
      .filter((e) => e.at >= since && (!hostIds.length || (e.hostId != null && hostIds.includes(e.hostId))))
      .sort((a, b) => b.at - a.at)
      .slice(0, max)
      .map((e) => `- [${new Date(e.at * 1000).toISOString().slice(0, 16).replace("T", " ")} UTC] ${e.area}${e.host ? ` on ${e.host}` : ""}: ${e.title}${e.detail ? `: ${e.detail}` : ""}${e.ok ? "" : " [FAILED]"}`)
      .join("\n");
  },
};
