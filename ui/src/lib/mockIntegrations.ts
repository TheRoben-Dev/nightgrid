/**
 * Browser-only stand-in for integrations, with canned Plex, AMP and Uptime
 * Kuma statuses for `pnpm dev`, tests and screenshots.
 */
import type {
  AmpAction,
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
  },
};
