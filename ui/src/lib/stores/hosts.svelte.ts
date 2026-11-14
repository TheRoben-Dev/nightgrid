import { api, errorText, type Group, type Host, type HostPing } from "../api";
import { fuzzyMatch } from "../palette/fuzzy";

const PING_EVERY_MS = 30_000;

export interface HostGroupView {
  id: number | null;
  name: string;
  color: string | null;
  hosts: Host[];
}

/** Matches a host against a search query (name, hostname, user, tags). */
export function hostMatches(h: Host, q: string): boolean {
  const query = q.trim();
  if (!query) return true;
  if (query.startsWith("#")) return h.tags.some((t) => t.toLowerCase().startsWith(query.slice(1).toLowerCase()));
  const hay = [h.name, h.hostname, h.username ?? "", ...h.tags].join(" ");
  return fuzzyMatch(query, hay) !== null;
}

/** Groups hosts for the rail: favorites first, then user groups, then ungrouped. */
export function groupHosts(hosts: Host[], groups: Group[], q: string): HostGroupView[] {
  const visible = hosts.filter((h) => hostMatches(h, q));
  const out: HostGroupView[] = [];
  const favs = visible.filter((h) => h.favorite);
  if (favs.length) out.push({ id: -1, name: "Favorites", color: null, hosts: favs });
  const rest = visible.filter((h) => !h.favorite);
  for (const g of [...groups].sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name))) {
    const hs = rest.filter((h) => h.groupId === g.id);
    if (hs.length) out.push({ id: g.id, name: g.name, color: g.color, hosts: hs });
  }
  const loose = rest.filter((h) => h.groupId === null || !groups.some((g) => g.id === h.groupId));
  if (loose.length) out.push({ id: null, name: out.length ? "Ungrouped" : "All hosts", color: null, hosts: loose });
  return out;
}

class HostsStore {
  hosts = $state<Host[]>([]);
  groups = $state<Group[]>([]);
  pings = $state<Record<number, HostPing>>({});
  query = $state("");
  error = $state<string | null>(null);
  loaded = $state(false);
  /** Host being edited (`0` = new host), or null when the editor is closed. */
  editing = $state<number | null>(null);
  importOpen = $state(false);
  private timer: ReturnType<typeof setInterval> | null = null;

  get grouped(): HostGroupView[] {
    return groupHosts(this.hosts, this.groups, this.query);
  }

  byId(id: number): Host | undefined {
    return this.hosts.find((h) => h.id === id);
  }

  async refresh() {
    try {
      const [hosts, groups] = await Promise.all([api.hostsList(), api.groupsList()]);
      this.hosts = hosts;
      this.groups = groups;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loaded = true;
    }
    void this.ping();
  }

  async ping() {
    if (!this.hosts.length) return;
    try {
      const res = await api.hostsPing(this.hosts.map((h) => h.id));
      const next: Record<number, HostPing> = {};
      for (const p of res) next[p.id] = p;
      this.pings = next;
    } catch {
      /* reachability is best-effort */
    }
  }

  /** Starts periodic pings while the window is visible. Returns a stop function. */
  start(): () => void {
    void this.refresh();
    const tick = () => {
      if (typeof document === "undefined" || document.visibilityState === "visible") void this.ping();
    };
    this.timer = setInterval(tick, PING_EVERY_MS);
    return () => {
      if (this.timer) clearInterval(this.timer);
      this.timer = null;
    };
  }
}

export const hosts = new HostsStore();
