/**
 * Browser-only stand-in for cross-host log search (M10u): a fixed set of
 * journal and container lines per host, matched like the backend does.
 */
import type { LogHit, LogSearchQuery_Serialize as LogSearchQuery, LogSearchResult, SavedSearch_Serialize as SavedSearch, SavedSearchInput_Deserialize as SavedSearchInput } from "./bindings";

const now = () => Math.floor(Date.now() / 1000);
const LINES: [number, string, number, string][] = [
  [1, "journal/sshd", 40, "Failed password for invalid user admin from 203.0.113.7 port 52144 ssh2"],
  [1, "journal/sshd", 300, "Accepted publickey for roben from 192.168.1.20 port 50122 ssh2"],
  [1, "container/proxy", 90, '192.168.1.20 - - "GET /photos HTTP/1.1" 502 157 "upstream timed out"'],
  [1, "container/proxy", 400, '192.168.1.21 - - "GET / HTTP/1.1" 200 5120'],
  [2, "journal/kernel", 120, "nvme0: I/O 412 QID 3 timeout, aborting"],
  [2, "container/photos", 95, "ERROR database connection refused: dial tcp 172.18.0.4:5432"],
  [2, "container/photos", 600, "INFO started in 2.1s"],
  [2, "journal/systemd", 800, "backup.service: Failed with result 'exit-code'."],
  [3, "journal/smartd", 1500, "Device: /dev/sda [SAT], 8 Currently unreadable (pending) sectors"],
];
let saved: SavedSearch[] = [
  { id: 1, name: "Upstream errors", query: { text: "upstream", regex: false, caseSensitive: false, hosts: [], journal: true, containers: true, unit: null, container: null, priority: null, sinceMins: 60 }, alert: true },
];
let nextId = 2;

function matcher(q: LogSearchQuery): (s: string) => boolean {
  const t = q.text.trim();
  if (!t) return () => true;
  if (q.regex) {
    const re = new RegExp(t, q.caseSensitive ? "" : "i");
    return (s) => re.test(s);
  }
  return q.caseSensitive ? (s) => s.includes(t) : (s) => s.toLowerCase().includes(t.toLowerCase());
}

const full = (q: Partial<LogSearchQuery>): LogSearchQuery => ({ text: "", regex: false, caseSensitive: false, hosts: [], journal: true, containers: true, unit: null, container: null, priority: null, sinceMins: 60, ...q });

export const mockLogs = {
  async search(q0: Partial<LogSearchQuery>): Promise<LogSearchResult> {
    const q = full(q0);
    let m: (s: string) => boolean;
    try {
      m = matcher(q);
    } catch {
      throw new Error("not a valid regular expression");
    }
    await new Promise((r) => setTimeout(r, 250));
    const hosts = q.hosts.length ? q.hosts : [1, 2, 3];
    const t = now();
    const hits: LogHit[] = LINES.filter(([h, src, ago, text]) => hosts.includes(h) && ago <= q.sinceMins * 60 && (src.startsWith("journal/") ? q.journal : q.containers) && m(text))
      .map(([h, src, ago, text]) => ({ hostId: h, source: src, ts: t - ago, text }))
      .sort((a, b) => b.ts - a.ts);
    return { hits, truncated: false, errors: hosts.includes(3) ? [{ hostId: 3, message: "connection timed out" }] : [], hosts };
  },
  async list(): Promise<SavedSearch[]> {
    return [...saved];
  },
  async save(i: SavedSearchInput): Promise<SavedSearch> {
    const s: SavedSearch = { id: i.id ?? nextId++, name: i.name.trim(), query: full(i.query as Partial<LogSearchQuery>), alert: i.alert };
    saved = saved.some((x) => x.id === s.id) ? saved.map((x) => (x.id === s.id ? s : x)) : [...saved, s];
    return s;
  },
  async delete(id: number): Promise<null> {
    saved = saved.filter((s) => s.id !== id);
    return null;
  },
};
