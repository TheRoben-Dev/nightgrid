import { api, type Alert, type AlertKind, type AlertsSnapshot } from "../api";
import { docker } from "./docker.svelte";
import { fleet } from "./fleet.svelte";
import { metrics } from "./metrics.svelte";
import { system } from "./system.svelte";
import { ui } from "./ui.svelte";
import { settings } from "./settings.svelte";

export const KIND_LABEL: Record<AlertKind, string> = {
  hostOffline: "HOST OFFLINE",
  diskFull: "DISK FULL",
  containerDown: "CONTAINER DOWN",
  serviceDown: "SERVICE DOWN",
  certExpiring: "CERT EXPIRING",
  smartWarning: "SMART WARNING",
  backupFailed: "BACKUP FAILED",
  backupStale: "BACKUP STALE",
  logBurst: "ERROR BURST",
  logNewError: "NEW ERROR",
  expirySoon: "EXPIRING",
  diskFilling: "FILLING UP",
  poolDegraded: "POOL DEGRADED",
  upsOnBattery: "ON BATTERY",
  logMatch: "LOG MATCH",
};

/** Log alerts can be explained from their recent error lines. */
export const isLogAlert = (a: Alert): boolean => a.kind === "logBurst" || a.kind === "logNewError";

const LOG_KEY = /^logs:\d+:(.+?):(burst|404|new:([0-9a-f]{16})|new)$/;

/** `logs:3:container/web:burst` or `logs:3:container/web:new:<fingerprint>` → "container/web". */
export function logSource(a: Alert): string | null {
  return LOG_KEY.exec(a.key)?.[1] ?? null;
}

/** The pattern behind a NEW ERROR alert, for "Mute this pattern". */
export function logFingerprint(a: Alert): string | null {
  return LOG_KEY.exec(a.key)?.[3] ?? null;
}

/** Where clicking an alert takes you. */
export function targetLabel(a: Alert): string {
  switch (a.kind) {
    case "hostOffline":
    case "diskFull":
      return "Monitor";
    case "containerDown":
      return "Docker";
    case "serviceDown":
    case "certExpiring":
    case "expirySoon":
      return "Services";
    case "smartWarning":
      return "Disks";
    case "diskFilling":
    case "poolDegraded":
      return "Fleet disks";
    case "backupFailed":
    case "backupStale":
      return "Fleet backups";
    case "logBurst":
    case "logNewError":
      return logSource(a)?.startsWith("container/") ? "Docker logs" : "Journal";
    case "upsOnBattery":
      return "Fleet UPS";
    case "logMatch":
      return "Fleet logs";
  }
}

/** "4m", "2h 5m", "3d" since `ts` (unix seconds). */
export function ago(ts: number, now = Math.floor(Date.now() / 1000)): string {
  const s = Math.max(0, now - ts);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${h}h ${m % 60}m` : `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

/** Minutes after midnight ↔ "HH:MM". */
export const toHhmm = (min: number) => `${String(Math.floor(min / 60)).padStart(2, "0")}:${String(min % 60).padStart(2, "0")}`;
export function fromHhmm(s: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  return h < 24 && min < 60 ? h * 60 + min : null;
}

/**
 * Active alerts with those explained by an outage upstream (M10s) listed
 * under their cause. An alert whose cause isn't in the list stays on top.
 */
export function groupByCause(active: Alert[]): { alert: Alert; folded: Alert[] }[] {
  const ids = active.map((a) => a.id);
  const roots = active.filter((a) => a.causedBy == null || !ids.includes(a.causedBy));
  return roots.map((alert) => ({ alert, folded: active.filter((a) => a.causedBy === alert.id && a.id !== alert.id) }));
}

export type Tone = "ok" | "warn" | "bad";
export function tone(s: AlertsSnapshot | null): Tone {
  if (!s?.active.length) return "ok";
  return s.active.some((a) => a.severity === "critical") ? "bad" : "warn";
}

class AlertsStore {
  snap = $state<AlertsSnapshot | null>(null);
  open = $state(false);
  private started = false;

  get active(): Alert[] {
    return this.snap?.active ?? [];
  }

  /** Active alerts without those folded under a cause. */
  get roots(): Alert[] {
    return groupByCause(this.active).map((g) => g.alert);
  }

  get history(): Alert[] {
    return this.snap?.history ?? [];
  }

  get tone(): Tone {
    return tone(this.snap);
  }

  start() {
    if (this.started) return;
    this.started = true;
    void api.alertsSubscribe((s) => (this.snap = s));
  }

  /** Jumps to the view that explains the alert. */
  go(a: Alert) {
    this.open = false;
    const host = a.hostId;
    switch (a.kind) {
      case "hostOffline":
      case "diskFull":
        if (host != null) metrics.detailId = host;
        break;
      case "containerDown":
        if (host != null) docker.select(host);
        ui.go("docker");
        break;
      case "serviceDown":
      case "certExpiring":
      case "expirySoon":
        ui.go("services");
        break;
      case "smartWarning":
        if (host != null) system.select(host);
        void system.openTab("disks");
        ui.go("system");
        break;
      case "diskFilling":
      case "poolDegraded":
        fleet.tab = "disks";
        ui.go("fleet");
        break;
      case "backupFailed":
      case "backupStale":
        fleet.openBackups();
        ui.go("fleet");
        break;
      case "upsOnBattery":
        fleet.tab = "ups";
        ui.go("fleet");
        break;
      case "logMatch":
        fleet.tab = "logs";
        ui.go("fleet");
        break;
      case "logBurst":
      case "logNewError": {
        const src = logSource(a) ?? "";
        if (src.startsWith("container/")) {
          if (host != null) docker.select(host);
          docker.tab = "containers";
          ui.go("docker");
        } else {
          if (host != null) system.select(host);
          void system.openTab("journal");
          ui.go("system");
        }
        break;
      }
    }
  }

  /** Recent error lines behind a log alert, for "what broke". */
  samples(a: Alert): Promise<string[]> {
    return api.alertsLogSamples(a.key);
  }

  /** New log errors that were only noted, newest first. */
  get notes() {
    return this.snap?.logNotes ?? [];
  }

  /** Events left out by mutes or as 404 noise, per source. */
  get quietLogs() {
    return (this.snap?.logQuiet ?? []).filter((q) => q.muted || q.notFound);
  }

  /** "Mute this pattern": it stops alerting and being noted (still counted). */
  async mute(hostId: number, source: string, fingerprint: string) {
    const s = await api.alertsLogMute(hostId, source, fingerprint);
    settings.value = s;
  }

  clearHistory() {
    void api.alertsClearHistory();
  }
}

export const alerts = new AlertsStore();
