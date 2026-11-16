import { describe, expect, it } from "vitest";
import type { Alert, BackupJob, BackupStatus, Host, HostPing, MountForecast, Service, ServiceStatus, UpdatesReport } from "./api";
import { widgetOn } from "./dashboard";
import { diskRows, updateRows, type HostDisks } from "./fleet";
import { agoShort, autoHosts, backupLines, diskLines, due, recentIncidents, serviceRows, shortError, updateLines } from "./overview";
import { healthOf } from "./stores/services.svelte";

const host = (id: number, name: string, role: "server" | "client" = "server") => ({ id, name, role, groupId: null }) as unknown as Host;
const ping = (state: "online" | "offline" | "unknown") => ({ state }) as unknown as HostPing;

describe("overview auto-fetch", () => {
  it("is due when never fetched or older than the schedule", () => {
    expect(due(null, 1000, 5000)).toBe(true);
    expect(due(4500, 1000, 5000)).toBe(false);
    expect(due(3000, 1000, 5000)).toBe(true);
  });

  it("reads servers that aren't offline, never clients", () => {
    const hs = [host(1, "nas"), host(2, "deck", "client"), host(3, "pi"), host(4, "new")];
    expect(autoHosts(hs, { 1: ping("online"), 2: ping("online"), 3: ping("offline") })).toEqual([1, 4]);
  });

  it("hides widgets whose feature is off", () => {
    const off = (f: string) => f !== "fleet.updates";
    expect(widgetOn("updates", off)).toBe(false);
    expect(widgetOn("disks", off)).toBe(true);
    expect(widgetOn("nodes", () => false)).toBe(true);
    expect(widgetOn("cmd:3", () => false)).toBe(true);
  });
});

describe("services widget", () => {
  const svc = (id: number, name: string, checkEnabled = true) => ({ id, name, url: `https://${name}.example.com`, checkEnabled }) as unknown as Service;
  const st = (id: number, up: boolean, o: Partial<ServiceStatus["result"]> = {}): ServiceStatus => ({ serviceId: id, ts: 1, result: { up, status: up ? 200 : null, latencyMs: up ? 42 : null, error: null, tls: null, ...o } }) as ServiceStatus;

  it("lists problems first with latency or the reason", () => {
    const rows = serviceRows(
      [svc(1, "alpha"), svc(2, "beta"), svc(3, "gamma"), svc(4, "delta", false), svc(5, "eps")],
      { 1: st(1, true), 2: st(2, false, { status: 502, error: "HTTP 502 isn't in 200-399 · from nginx" }), 3: st(3, false, { error: "timed out connecting to x · tried 10.0.0.1" }) },
      (s, x) => healthOf(s, x, 1),
    );
    expect(rows.map((r) => [r.svc.name, r.health, r.note])).toEqual([
      ["beta", "down", "HTTP 502"],
      ["gamma", "down", "timed out connecting to x"],
      ["eps", "pending", "…"],
      ["alpha", "up", "42 ms"],
      ["delta", "off", "off"],
    ]);
    expect(rows[1].detail).toContain("tried 10.0.0.1");
  });

  it("shortens long errors", () => {
    expect(shortError("a very long reason that goes on and on and on · more")).toHaveLength(28);
  });
});

describe("updates and disks widgets", () => {
  const rep = (n: number, o: Partial<UpdatesReport> = {}): UpdatesReport => ({
    manager: "apt",
    updates: Array.from({ length: n }, (_, i) => ({ name: `p${i}`, current: "1", available: "2", repo: i === 0 ? "jammy-security" : "jammy-updates" })),
    rebootRequired: false,
    rebootReasons: [],
    note: null,
    ...o,
  });

  it("says per host what is pending", () => {
    const hs = [host(1, "a"), host(2, "b"), host(3, "c"), host(4, "never")];
    const lines = updateLines(
      updateRows(hs, {
        1: { status: "ok", report: rep(0), error: null, checkedAt: 1 },
        2: { status: "ok", report: rep(5, { rebootRequired: true, rebootReasons: ["linux-image"] }), error: null, checkedAt: 1 },
        3: { status: "error", report: null, error: "needs a login first", checkedAt: 1 },
      }),
    );
    expect(lines.map((l) => [l.host.name, l.text])).toEqual([
      ["b", "5 pending · 1 security · reboot"],
      ["c", "couldn't check"],
      ["a", "up to date"],
    ]);
    expect(lines[0].detail).toBe("linux-image");
  });

  const fc = (mount: string, used: number, size: number, daysLeft: number | null): MountForecast => ({ mount, used, size, perDay: daysLeft == null ? null : 1, daysLeft, spanDays: 7 }) as unknown as MountForecast;
  const d = (o: Partial<HostDisks>): HostDisks => ({ status: "ok", error: null, pools: null, smart: null, smartNote: null, forecasts: [], checkedAt: 1, ...o });

  it("names each host's worst disk finding and its fullest mount", () => {
    const hs = [host(1, "nas"), host(2, "web"), host(3, "db"), host(4, "unread")];
    const lines = diskLines(
      diskRows(
        hs,
        {
          1: d({ pools: { health: "bad", pools: [{ kind: "zfs", name: "tank", state: "DEGRADED", health: "bad", issues: ["sdb faulted"] }] } as never, forecasts: [fc("/", 50, 100, null)] }),
          2: d({ forecasts: [fc("/srv", 90, 100, 9)], smart: { health: "ok", disks: [] } as never }),
          3: d({ pools: { health: "ok", pools: [{ kind: "mdraid", name: "md0", state: "clean", health: "ok", issues: [] }] } as never, smart: { health: "ok", disks: [{}, {}] } as never }),
        },
        14,
      ),
    );
    expect(lines.map((l) => [l.host.name, l.text, l.pct])).toEqual([
      ["nas", "ZFS tank degraded", 50],
      ["web", "/srv full in about 9 days", 90],
      ["db", "1 pool ok · SMART ok (2)", null],
    ]);
    expect(lines[0].detail).toBe("sdb faulted");
  });
});

describe("backups and alerts widgets", () => {
  it("lists jobs worst first with the last good run's age", () => {
    const job = (id: number, name: string) => ({ id, name, hostId: 1, tool: "restic" }) as unknown as BackupJob;
    const bs = (jobId: number, o: Partial<BackupStatus>) => ({ jobId, result: "ok", lastRun: null, lastOk: null, exitCode: null, size: null, added: null, summary: null, stale: false, error: null, ...o }) as BackupStatus;
    const lines = backupLines(
      [job(1, "photos"), job(2, "db"), job(3, "etc"), job(4, "new")],
      { 1: bs(1, { lastOk: 10_000 - 7200 }), 2: bs(2, { result: "failed", summary: "repo locked" }), 3: bs(3, { stale: true, lastOk: 10_000 - 3 * 86_400 }) },
      (s) => (!s ? "unknown" : s.result === "failed" ? "failed" : s.stale ? "stale" : "ok"),
      10_000,
    );
    expect(lines.map((l) => [l.job.name, l.state, l.age])).toEqual([
      ["db", "failed", "never"],
      ["etc", "stale", "3d ago"],
      ["new", "unknown", "—"],
      ["photos", "ok", "2h ago"],
    ]);
    expect(lines[0].detail).toBe("repo locked");
  });

  it("keeps the last week's resolved alerts, newest first", () => {
    const al = (id: number, resolved: number | null) => ({ id, resolved, since: 0 }) as unknown as Alert;
    const now = 30 * 86_400;
    expect(recentIncidents([al(1, now - 100), al(2, null), al(3, now - 50), al(4, now - 8 * 86_400)], now).map((a) => a.id)).toEqual([3, 1]);
  });

  it("says short ages", () => {
    expect(agoShort(100, 130)).toBe("30s");
    expect(agoShort(0, 3 * 3600)).toBe("3h");
    expect(agoShort(0, 3 * 86_400)).toBe("3d");
  });
});
