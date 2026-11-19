/**
 * Browser-only stand-in for the services board: a few example services with
 * simulated checks, generated history and a fake Homepage config. Used by
 * `pnpm dev`, tests and screenshots; never makes a network request.
 */
import type {
  CheckPoint_Serialize as CheckPoint,
  CheckResult_Serialize as CheckResult,
  HomepageCandidate,
  HomepagePreview_Serialize as HomepagePreview,
  ImportSummary,
  Service_Serialize as Service,
  ServiceEvent_Serialize as ServiceEvent,
  ServiceInput_Deserialize as ServiceInput,
  ServiceStatus_Serialize as ServiceStatus,
  Side,
} from "./bindings";

const DAY = 86_400;
const now = () => Math.floor(Date.now() / 1000);

let nextId = 1;
const mk = (s: Partial<Service> & Pick<Service, "name" | "url">): Service => ({
  id: nextId++,
  hostId: 1,
  group: "Apps",
  description: "",
  icon: null,
  checkEnabled: true,
  expectStatus: "200-399",
  allowUntrusted: false,
  sortOrder: nextId,
  source: "manual",
  ...s,
});

let services: Service[] = [
  mk({ name: "Media server", url: "https://media.example.lan/", group: "Media", description: "Movies and shows" }),
  mk({ name: "Requests", url: "http://192.0.2.10:5055", group: "Media", description: "Request queue" }),
  mk({ name: "Photos", url: "https://photos.example.lan", group: "Media", allowUntrusted: true }),
  mk({ name: "Git", url: "https://git.example.lan", group: "Dev", description: "Self-hosted forge", hostId: 2 }),
  mk({ name: "CI runner", url: "http://192.0.2.11:8080/health", group: "Dev", hostId: 2 }),
  mk({ name: "Router", url: "https://192.0.2.1", group: "Network", hostId: null, checkEnabled: false }),
];

let listener: ((e: ServiceEvent) => void) | null = null;
let timer: ReturnType<typeof setInterval> | null = null;

function simulate(s: Service): CheckResult {
  const flaky = s.name === "CI runner";
  const up = !flaky || Math.random() > 0.5;
  const https = s.url.startsWith("https");
  return {
    up,
    status: up ? 200 : 502,
    latencyMs: up ? Math.round(8 + Math.random() * (s.id * 9)) : 41,
    error: up ? null : "HTTP 502 isn't in 200-399",
    tls: https
      ? {
          trusted: !s.allowUntrusted,
          problem: s.allowUntrusted ? "unknown issuer or self-signed" : null,
          expires: now() + (s.name === "Git" ? 9 : 64) * DAY,
          subject: `CN=${new URL(s.url).hostname}`,
          issuer: s.allowUntrusted ? `CN=${new URL(s.url).hostname}` : "CN=Example CA",
          fingerprint: (s.id.toString(16).padStart(2, "0") + "ab").repeat(16),
        }
      : null,
  };
}

function status(s: Service): ServiceStatus {
  return { serviceId: s.id, ts: now(), result: simulate(s) };
}

function runAll() {
  for (const s of services) if (s.checkEnabled) listener?.({ kind: "status", ...status(s) });
}

function norm(i: ServiceInput, base?: Service): Service {
  if (!/^https?:\/\/[^/\s@]+/i.test(i.url ?? "")) throw { message: "url: use http:// or https://" };
  if (!i.name?.trim()) throw { message: "name: 1–120 characters" };
  return {
    ...(base ?? mk({ name: "", url: "" })),
    name: i.name.trim(),
    url: i.url!.trim(),
    hostId: i.hostId ?? null,
    group: (i.group ?? "").trim(),
    description: (i.description ?? "").trim(),
    icon: i.icon ?? null,
    checkEnabled: i.checkEnabled ?? false,
    expectStatus: i.expectStatus?.trim() || "200-399",
    allowUntrusted: i.allowUntrusted ?? false,
  };
}

const HOMEPAGE = [
  { group: "Media", name: "Media server", url: "https://media.example.lan/", description: "Movies and shows", icon: "media.png" },
  { group: "Media", name: "Books", url: "http://192.0.2.10:8083", description: "", icon: null },
  { group: "Infra", name: "Proxy", url: "https://proxy.example.lan", description: "Reverse proxy", icon: null },
];

export const mockServices = {
  async subscribe(on: (e: ServiceEvent) => void): Promise<null> {
    listener = on;
    runAll();
    if (timer) clearInterval(timer);
    timer = setInterval(runAll, 15_000);
    return null;
  },
  async checkAll(): Promise<null> {
    runAll();
    return null;
  },
  async list(): Promise<Service[]> {
    return services.map((s) => ({ ...s }));
  },
  async create(i: ServiceInput): Promise<Service> {
    const s = norm(i);
    services = [...services, s];
    if (s.checkEnabled) setTimeout(() => listener?.({ kind: "status", ...status(s) }), 300);
    return s;
  },
  async update(id: number, i: ServiceInput): Promise<Service> {
    const old = services.find((s) => s.id === id);
    if (!old) throw { message: "service not found" };
    const s = norm(i, old);
    services = services.map((x) => (x.id === id ? s : x));
    if (!s.checkEnabled) listener?.({ kind: "cleared", serviceId: id });
    return s;
  },
  async remove(id: number): Promise<null> {
    services = services.filter((s) => s.id !== id);
    listener?.({ kind: "cleared", serviceId: id });
    return null;
  },
  async check(id: number): Promise<ServiceStatus> {
    const s = services.find((x) => x.id === id);
    if (!s) throw { message: "service not found" };
    const st = status(s);
    listener?.({ kind: "status", ...st });
    return st;
  },
  async history(id: number, from: number): Promise<CheckPoint[]> {
    const out: CheckPoint[] = [];
    const end = now();
    for (let ts = Math.max(from, end - DAY); ts < end; ts += 600) {
      const down = id === 5 && Math.sin(ts / 5000) > 0.8;
      out.push({ ts, up: !down, status: down ? 502 : 200, latencyMs: down ? null : Math.round(10 + id * 6 + Math.sin(ts / 3000 + id) * 8) });
    }
    return out;
  },
  async detect(hostId: number): Promise<HomepageCandidate[]> {
    return hostId === 1 ? [{ container: "homepage", dir: "/opt/stacks/homepage/config" }] : [];
  },
  async preview(_side: Side, dir: string): Promise<HomepagePreview> {
    if (!dir.trim()) throw { message: "No such file: services.yaml" };
    return {
      path: dir.replace(/\/$/, "") + "/services.yaml",
      layout: true,
      parse: { entries: HOMEPAGE, skipped: ["Infra/Router: URL uses a Homepage variable ({{HOMEPAGE_VAR_ROUTER}})"] },
    };
  },
  async importHomepage(_side: Side, _dir: string, linkHost: boolean, prune: boolean): Promise<ImportSummary> {
    let added = 0;
    let unchanged = 0;
    for (const e of HOMEPAGE) {
      if (services.some((s) => s.source === "homepage" && s.name === e.name)) {
        unchanged++;
        continue;
      }
      services = [
        ...services,
        mk({ ...e, source: "homepage", hostId: linkHost ? 1 : null }),
      ];
      added++;
    }
    void prune;
    runAll();
    return { added, updated: 0, unchanged, removed: 0, stale: [] };
  },
};
