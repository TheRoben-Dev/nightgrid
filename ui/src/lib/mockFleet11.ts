/** Browser-only stand-ins for the 1.1 Fleet tabs: CATALOG, DRILLS, CAPACITY, REPLAY. */
import type {
  CatalogHost_Serialize as CatalogHost,
  Drill_Serialize as Drill,
  DrillBoard_Serialize as DrillBoard,
  DrillRun_Serialize as DrillRun,
  HostCapacity_Serialize as HostCapacity,
  Incident_Serialize as Incident,
  Replay_Serialize as Replay,
  ReplayEvent_Serialize as ReplayEvent,
} from "./bindings";
import { mockBackend } from "./mock";

const now = () => Math.floor(Date.now() / 1000);
const GB = 1e9;
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const mockCatalog = {
  async build(hostIds: number[]): Promise<CatalogHost[]> {
    await wait(400);
    const hs = (await mockBackend.hostsList()).filter((h) => hostIds.includes(h.id));
    return hs.map((h, i) => ({
      hostId: h.id,
      name: h.name,
      error: i === 2 ? "Docker isn't installed" : null,
      jobsRead: i === 0 ? 1 : 0,
      entries:
        i === 2
          ? []
          : [
              { name: "jellyfin", image: "jellyfin/jellyfin:10.9.11", version: "10.9.11", state: "running", health: "healthy", project: "media", ports: ["8096→8096/tcp"], urls: [`http://${h.hostname}:8096`, "https://media.example.org"], mounts: [{ source: "/srv/media/jellyfin/config", destination: "/config", kind: "bind", name: null, backedUpBy: i === 0 ? "restic-daily" : null }, { source: "/mnt/cache/jellyfin", destination: "/cache", kind: "bind", name: null, backedUpBy: null }], coverage: i === 0 ? "partial" : "none" },
              { name: "postgres", image: "postgres:16", version: "16", state: "running", health: null, project: "immich", ports: [], urls: [], mounts: [{ source: "/var/lib/docker/volumes/immich_pg/_data", destination: "/var/lib/postgresql/data", kind: "volume", name: "immich_pg", backedUpBy: i === 0 ? "restic-daily" : null }], coverage: i === 0 ? "covered" : "none" },
              { name: "whoami", image: "traefik/whoami:latest", version: null, state: "exited", health: null, project: null, ports: ["8081→80/tcp"], urls: [], mounts: [], coverage: "stateless" },
            ],
    }));
  },
  async markdown(hosts: CatalogHost[]): Promise<string> {
    return `# Service catalog\n\n${hosts.map((h) => `## ${h.name}\n\n${h.entries.map((e) => `- ${e.name} (${e.image})`).join("\n")}`).join("\n\n")}\n`;
  },
};

let drills: Drill[] = [];
const runs = new Map<number, DrillRun[]>();
const running = new Set<number>();

export const mockDrills = {
  async list(): Promise<DrillBoard> {
    return {
      running: [...running],
      drills: drills.map((d) => {
        const r = runs.get(d.id!) ?? [];
        const last = r[0] ?? null;
        return { drill: d, last, passed: r.slice(0, 10).filter((x) => x.ok).length, runs: Math.min(10, r.length), nextAt: d.enabled && d.everyDays ? (last ? last.at + d.everyDays * 86400 : now()) : null };
      }),
    };
  },
  async save(d: Drill): Promise<number> {
    if (d.id != null) {
      drills = drills.map((x) => (x.id === d.id ? d : x));
      return d.id;
    }
    const id = Math.max(0, ...drills.map((x) => x.id!)) + 1;
    drills.push({ ...d, id });
    return id;
  },
  async remove(id: number): Promise<null> {
    drills = drills.filter((d) => d.id !== id);
    return null;
  },
  async runs(id: number): Promise<DrillRun[]> {
    return runs.get(id) ?? [];
  },
  async run(id: number): Promise<DrillRun> {
    const d = drills.find((x) => x.id === id)!;
    running.add(id);
    await wait(1500);
    running.delete(id);
    const ok = Math.random() > 0.25;
    const steps =
      d.kind === "proxmox"
        ? [`Newest backup: backup:backup/vzdump-qemu-${d.vmid}-2026_11_18-02_00_03.vma.zst (9 hours old, 12.4 GB)`, `Restoring into a new qemu 9001 on ${d.targetStorage}`, "Restored in 184 s", "Isolated: net0: cable pulled (link down)", "Started", ok ? "The guest agent answered" : "The restored guest didn't stay running", "Destroyed qemu 9001"]
        : ["Latest snapshot 3f2a9c1e (2026-11-18 02:00)", "Restored /srv/media/jellyfin/config/system.xml (14 KB)", ok ? "Size matches" : "Size differs", "Deleted the temporary folder"];
    const r: DrillRun = { id: Date.now(), drillId: id, at: now(), seconds: d.kind === "proxmox" ? 241 : 12, ok, steps, error: ok ? null : "the check failed", detail: ok ? "restored and checked" : "" };
    runs.set(id, [r, ...(runs.get(id) ?? [])]);
    return r;
  },
  async report(id: number): Promise<string> {
    const d = drills.find((x) => x.id === id);
    return `# Restore drill: ${d?.name}\n\n${(runs.get(id) ?? []).map((r) => `- ${new Date(r.at * 1000).toISOString()}: ${r.ok ? "passed" : "FAILED"}`).join("\n")}\n`;
  },
};

export const mockCapacity = {
  async list(days: number): Promise<HostCapacity[]> {
    const hs = await mockBackend.hostsList();
    return hs.map((h, i) => ({
      hostId: h.id,
      spec: { hostId: h.id, cores: [8, 4, 16, 2][i % 4], memTotal: [32, 8, 64, 4][i % 4] * GB, at: now() - 600 },
      cpuAvg: [12, 35, 6, 50][i % 4],
      cpuP95: [41, 78, 18, 92][i % 4],
      memAvg: [48, 80, 30, 70][i % 4],
      memP95: [55, 91, 34, 88][i % 4],
      gpuP95: i === 0 ? 22 : null,
      hours: Math.min(days * 24, 400),
      disks: [{ mount: "/", size: [500, 120, 1000, 32][i % 4] * GB, used: [210, 101, 300, 12][i % 4] * GB, daysLeft: i === 1 ? 23 : null }],
    }));
  },
};

export const mockReplay = {
  async incidents(): Promise<Incident[]> {
    const hs = await mockBackend.hostsList();
    const h = hs[0];
    const t = now();
    return [
      { kind: "outage", hostId: h?.id ?? null, serviceId: 1, title: "Jellyfin down", detail: "connection refused", start: t - 5 * 3600, end: t - 5 * 3600 + 540, level: "critical" },
      { kind: "alerts", hostId: h?.id ?? null, serviceId: null, title: `${h?.name ?? "server01"}: MEMORY HIGH and 1 more`, detail: "MEMORY HIGH; CONTAINER UNHEALTHY", start: t - 30 * 3600, end: t - 30 * 3600 + 1800, level: "warn" },
    ];
  },
  async load(incident: Incident): Promise<Replay> {
    const from = incident.start - 1800;
    const to = incident.end + 1800;
    const metrics = [];
    for (let ts = from; ts <= to; ts += 60) {
      const inside = ts >= incident.start && ts <= incident.end;
      metrics.push({ ts, cpu: inside ? 85 + Math.random() * 10 : 15 + Math.random() * 10, mem: inside ? 93 : 60, swap: null, load1: null, disk: 61, netRx: null, netTx: null, diskRead: null, diskWrite: null, temp: null, gpu: null, gpuMem: null, gpuTemp: null, gpuPower: null });
    }
    const ev = (at: number, source: ReplayEvent["source"], level: ReplayEvent["level"], title: string, detail = ""): ReplayEvent => ({ at, source, level, title, detail, hostId: incident.hostId });
    return {
      incident,
      from,
      to,
      metrics,
      events: [
        ev(incident.start - 600, "change", "info", "Deployed stack media", "compose up -d"),
        ev(incident.start - 60, "event", "warn", "Container jellyfin restarted", "exit 137"),
        ev(incident.start, "outage", "critical", "Jellyfin went down", incident.detail),
        ev(incident.start + 30, "event", "critical", "OOM kill", "ffmpeg (jellyfin)"),
        ev(incident.start + 45, "notice", "critical", "Jellyfin is down"),
        ev(incident.end, "outage", "ok", "Jellyfin last seen down"),
      ],
    };
  },
};
