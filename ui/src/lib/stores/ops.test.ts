import { describe, expect, it, vi } from "vitest";
import type { Alert, BackupStatus, ProxyRoute } from "../api";
import { logSource, targetLabel } from "./alerts.svelte";
import { backups, bytes, jobState, staleWords, testUnsupported } from "./backups.svelte";
import { system } from "./system.svelte";
import { certDays, certTone } from "./proxy.svelte";

const st = (o: Partial<BackupStatus>): BackupStatus => ({
  jobId: 1,
  result: "ok",
  lastRun: 0,
  lastOk: 0,
  exitCode: 0,
  size: null,
  added: null,
  summary: null,
  stale: false,
  error: null,
  ...o,
});

describe("backup board", () => {
  it("words and states", () => {
    expect(bytes(41_500_000_000)).toBe("41.5 GB");
    expect(bytes(512)).toBe("512 B");
    expect(staleWords(26)).toBe("26 hours");
    expect(staleWords(72)).toBe("3 days");
    expect(jobState(undefined)).toBe("unknown");
    expect(jobState(st({ stale: true }))).toBe("stale");
    expect(jobState(st({ result: "failed", stale: true }))).toBe("failed");
    expect(jobState(st({ error: "missing" }))).toBe("error");
  });

  it("adds a found job, checks it and removes it", async () => {
    await backups.load();
    const before = backups.jobs.length;
    await backups.detect(1);
    expect(backups.suggestions?.length).toBeGreaterThan(0);
    backups.adopt(backups.suggestions![0]);
    expect(backups.editing?.input.target).toBe("restic-backup.timer");
    await backups.save();
    expect(backups.jobs.length).toBe(before + 1);
    const job = backups.jobs.at(-1)!;
    await backups.checkHost(1);
    expect(jobState(backups.status[job.id])).toBe("ok");
    // Test restore: asks for sudo (answered here), then records the result.
    const run = backups.testRestore(job);
    await vi.waitFor(() => expect(system.sudoAsk?.reason).toBe(`test-restore ${job.name}`));
    expect(system.sudoAsk?.host).toBeTruthy();
    system.answerSudo({ password: "pw", remember: false });
    await run;
    expect(backups.testResult?.ok).toBe(true);
    expect(backups.tests[job.id]?.note).toMatch(/^restored /);
    await backups.loadTests();
    expect(backups.tests[job.id]?.ok).toBe(true);
    expect(testUnsupported({ ...job, tool: "rclone" })).toMatch(/rclone/);
    expect(testUnsupported({ ...job, tool: "rsync", source: "unit" })).toMatch(/path job/);
    expect(testUnsupported(job)).toBeNull();
    backups.edit(job);
    backups.editing!.input.source = "path";
    backups.editing!.input.target = "relative";
    await backups.save();
    expect(backups.saveError).toMatch(/absolute/);
    backups.editing = null;
    await backups.remove(job);
    expect(backups.jobs.length).toBe(before);
  });
});

describe("proxy routes", () => {
  const route = (o: Partial<ProxyRoute>): ProxyRoute => ({
    proxy: "traefik",
    proxyName: "traefik",
    domains: ["a.example.com"],
    upstream: "a:80",
    container: "a",
    tls: true,
    cert: null,
    origin: "router a",
    ...o,
  });
  it("grades certificates", () => {
    const now = 1_000_000;
    expect(certTone(route({ tls: false }), now)).toBe("none");
    expect(certTone(route({ cert: { notAfter: now + 60 * 86400, issuer: null, error: null } }), now)).toBe("ok");
    expect(certTone(route({ cert: { notAfter: now + 5 * 86400, issuer: null, error: null } }), now)).toBe("warn");
    expect(certTone(route({ cert: { notAfter: now - 10, issuer: null, error: null } }), now)).toBe("bad");
    expect(certTone(route({ cert: { notAfter: null, issuer: null, error: "none" } }), now)).toBe("bad");
    expect(certDays(route({ cert: { notAfter: now + 3 * 86400 + 5, issuer: null, error: null } }), now)).toBe(3);
  });
});

describe("log alerts", () => {
  const alert = (key: string, kind: Alert["kind"]): Alert => ({ id: 1, key, kind, severity: "warn", title: "t", detail: "d", hostId: 2, since: 0, resolved: null, causedBy: null, silenced: null });
  it("finds the source", () => {
    expect(logSource(alert("logs:2:container/web:burst", "logBurst"))).toBe("container/web");
    expect(logSource(alert("logs:2:journal/sshd:new", "logNewError"))).toBe("journal/sshd");
    expect(targetLabel(alert("logs:2:container/web:burst", "logBurst"))).toBe("Docker logs");
    expect(targetLabel(alert("logs:2:journal/sshd:new", "logNewError"))).toBe("Journal");
    expect(targetLabel(alert("backup:2:1", "backupStale"))).toBe("Fleet backups");
  });
});

describe("profile reminder", () => {
  it("is due weekly once there are hosts", async () => {
    const { reminderDue, reminderWords, daysSince } = await import("./profileReminder.svelte");
    const day = 86400;
    const base = { days: 7, last: 0, snooze: null, firstSeen: 0, hostCount: 1 };
    expect(reminderDue(base, 6 * day)).toBe(false);
    expect(reminderDue(base, 7 * day)).toBe(true);
    expect(reminderDue({ ...base, hostCount: 0 }, 30 * day)).toBe(false);
    expect(reminderDue({ ...base, days: 0 }, 30 * day)).toBe(false);
    expect(reminderDue({ ...base, snooze: 8 * day }, 7.5 * day)).toBe(false);
    expect(reminderDue({ ...base, last: null, firstSeen: 2 * day }, 8 * day)).toBe(false);
    expect(reminderDue({ ...base, last: null, firstSeen: 2 * day }, 9 * day)).toBe(true);
    expect(reminderWords(14)).toBe("Every 2 weeks");
    expect(daysSince(0, 3.5 * day)).toBe(3);
  });
});
