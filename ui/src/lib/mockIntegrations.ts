/**
 * Browser-only stand-in for integrations, with canned Plex, AMP and Uptime
 * Kuma statuses, plus a Pi-hole whose blocking, lists and records change in memory, for `pnpm dev`, tests and screenshots.
 */
import type {
  AmpAction,
  DnsAction_Deserialize as DnsAction,
  DnsStatus_Serialize as DnsStatus,
  IntegrationInput_Deserialize as IntegrationInput,
  IntegrationStatus_Serialize as IntegrationStatus,
  IntegrationView_Serialize as IntegrationView,
} from "./bindings";

let nextId = 1;
const secrets = new Map<number, string>();
let list: IntegrationView[] = [];

const add = (input: IntegrationInput, secret?: string) => {
  const id = nextId++;
  if (secret) secrets.set(id, secret);
  const v: IntegrationView = {
    integration: {
      id,
      kind: input.kind ?? "plex",
      name: input.name ?? "",
      baseUrl: (input.baseUrl ?? "").replace(/\/+$/, ""),
      username: input.username ?? "",
      slug: input.slug ?? "",
      endpointId: input.endpointId ?? 0,
      hostId: input.hostId ?? null,
      allowUntrusted: !!input.allowUntrusted,
    },
    hasSecret: secrets.has(id),
  };
  list = [...list, v];
  return v;
};

add({ kind: "plex", name: "Plex", baseUrl: "http://server01:32400" }, "token");
add({ kind: "amp", name: "Game servers", baseUrl: "http://server01:8080", username: "admin" }, "pw");
add({ kind: "kuma", name: "Uptime Kuma", baseUrl: "http://server02:3001", slug: "home" });
add({ kind: "pihole", name: "Pi-hole", baseUrl: "http://192.168.1.2" }, "pw");

const now = () => Date.now() / 1000;
const dns: DnsStatus & { pausedUntil: number | null } = {
  version: "v6.1",
  blocking: true,
  resumesIn: null,
  pausedUntil: null,
  queries: 48213,
  blocked: 6120,
  percentBlocked: 12.69,
  clients: 14,
  onLists: 167820,
  topBlocked: [
    { domain: "ads.example.net", count: 812 },
    { domain: "telemetry.example.org", count: 301 },
    { domain: "metrics.example.com", count: 122 },
  ],
  lists: [
    { url: "https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts", name: "Steven Black", enabled: true, allow: false, entries: 160213, updated: now() - 3600 * 20 },
    { url: "https://example.com/extra-block.txt", name: "Extra", enabled: false, allow: false, entries: 7607, updated: now() - 86400 * 3 },
    { url: "https://example.com/allow.txt", name: "Allowed", enabled: true, allow: true, entries: 12, updated: now() - 86400 },
  ],
  records: [
    { domain: "nas.lan", answer: "192.168.1.10" },
    { domain: "server01.lan", answer: "192.168.1.11" },
  ],
};

function dnsStatus(): DnsStatus {
  if (dns.pausedUntil && dns.pausedUntil <= now()) {
    dns.blocking = true;
    dns.pausedUntil = null;
  }
  const { pausedUntil, ...rest } = dns;
  return { ...rest, resumesIn: pausedUntil ? Math.ceil(pausedUntil - now()) : null, lists: dns.lists.map((l) => ({ ...l })), records: dns.records.map((r) => ({ ...r })) };
}

const amp = [
  { name: "Minecraft01", friendlyName: "Survival World", module: "Minecraft", running: true, state: "ready", users: 3, maxUsers: 20, cpuPercent: 12, memoryMb: 3072 },
  { name: "Valheim01", friendlyName: "Valheim", module: "Generic", running: false, state: "stopped", users: null, maxUsers: null, cpuPercent: null, memoryMb: null },
];

function statusFor(kind: string): IntegrationStatus {
  switch (kind) {
    case "plex":
      return {
        kind: "plex",
        server: "media01",
        version: "1.41.2",
        transcodes: 1,
        streams: [
          { user: "alice", title: "Example Show · S2E5 · The Fifth One", media: "episode", player: "Living Room TV", state: "playing", progress: 25, transcode: true, quality: "1080p" },
          { user: "bob", title: "A Film (2019)", media: "movie", player: "Plex Web", state: "paused", progress: 61, transcode: false, quality: "4K" },
        ],
      };
    case "amp":
      return { kind: "amp", instances: amp.map((i) => ({ ...i })) };
    case "kuma":
      return {
        kind: "kuma",
        title: "Home Lab",
        up: 3,
        down: 1,
        monitors: [
          { id: 1, name: "Plex", group: "Media", state: "up", pingMs: 25, message: "200 - OK", uptime24h: 99.6 },
          { id: 2, name: "Jellyfin", group: "Media", state: "down", pingMs: null, message: "connect ECONNREFUSED", uptime24h: 50 },
          { id: 5, name: "Router", group: "Infra", state: "up", pingMs: 2, message: "", uptime24h: 100 },
          { id: 6, name: "NAS", group: "Infra", state: "up", pingMs: 4, message: "", uptime24h: 100 },
        ],
      };
    case "pihole":
      return { kind: "pihole", ...dnsStatus() };
    case "adguard":
      return { kind: "adguard", ...dnsStatus(), version: "0.107.57" };
    default:
      return { kind: "portainer", version: "2.21.0" };
  }
}

export const mockIntegrations = {
  async list(): Promise<IntegrationView[]> {
    return list.map((v) => ({ ...v, hasSecret: secrets.has(v.integration.id) }));
  },
  async create(input: IntegrationInput, secret: string | null): Promise<IntegrationView> {
    if (!/^https?:\/\/[^/\s@]+/i.test(input.baseUrl ?? "")) throw { message: "baseUrl: must start with http:// or https://" };
    return add(input, secret ?? undefined);
  },
  async update(id: number, input: IntegrationInput, secret: string | null): Promise<IntegrationView> {
    if (secret === "") secrets.delete(id);
    else if (secret) secrets.set(id, secret);
    list = list.map((v) =>
      v.integration.id === id
        ? { ...v, integration: { ...v.integration, ...input, kind: v.integration.kind, baseUrl: (input.baseUrl ?? "").replace(/\/+$/, "") } as IntegrationView["integration"] }
        : v,
    );
    return { ...list.find((v) => v.integration.id === id)!, hasSecret: secrets.has(id) };
  },
  async remove(id: number): Promise<null> {
    list = list.filter((v) => v.integration.id !== id);
    secrets.delete(id);
    return null;
  },
  async reveal(id: number): Promise<string | null> {
    return secrets.get(id) ?? null;
  },
  async status(id: number): Promise<IntegrationStatus> {
    const v = list.find((x) => x.integration.id === id);
    if (!v) throw { message: "integration not found" };
    return statusFor(v.integration.kind);
  },
  async test(input: IntegrationInput): Promise<IntegrationStatus> {
    return statusFor(input.kind ?? "portainer");
  },
  async ampAction(_id: number, instance: string, action: AmpAction): Promise<null> {
    const i = amp.find((x) => x.name === instance);
    if (i) {
      i.running = action !== "stop";
      i.state = i.running ? "ready" : "stopped";
    }
    return null;
  },  async dnsAction(_id: number, a: DnsAction): Promise<null> {
    await new Promise((r) => setTimeout(r, 250));
    switch (a.op) {
      case "setBlocking":
        dns.blocking = a.enabled;
        dns.pausedUntil = !a.enabled && a.seconds ? now() + a.seconds : null;
        break;
      case "addList":
        if (!/^https?:\/\//.test(a.url)) throw { message: `"${a.url}" isn't a list URL (http or https)` };
        dns.lists = [...dns.lists, { url: a.url, name: a.name, enabled: true, allow: a.allow, entries: null, updated: null }];
        break;
      case "removeList":
        dns.lists = dns.lists.filter((l) => !(l.url === a.url && l.allow === a.allow));
        break;
      case "setListEnabled":
        dns.lists = dns.lists.map((l) => (l.url === a.url && l.allow === a.allow ? { ...l, enabled: a.enabled } : l));
        break;
      case "updateLists":
        dns.lists = dns.lists.map((l) => ({ ...l, updated: now(), entries: l.entries ?? 1000 }));
        break;
      case "addRecord":
        if (!/^[\d.:a-f]+$/i.test(a.answer)) throw { message: `"${a.answer}" isn't an IP address` };
        dns.records = [...dns.records, { domain: a.domain, answer: a.answer }];
        break;
      case "removeRecord":
        dns.records = dns.records.filter((r) => !(r.domain === a.domain && r.answer === a.answer));
        break;
    }
    return null;
  },
};
