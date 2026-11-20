import { sound } from "./sound.svelte";
import { api, type AckFor, type Alert, type AlertKind, type AlertsSnapshot, type MuteScope, type Silence } from "../api";
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
  netNewDevice: "NEW DEVICE",
  netMissing: "DEVICE MISSING",
  netRisk: "NETWORK RISK",
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
    case "netNewDevice":
    case "netMissing":
    case "netRisk":
      return "Network map";
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

/** Active alerts nobody acknowledged or muted (D-196): these badge and notify. */
export const loud = (active: Alert[]): Alert[] => active.filter((a) => a.silenced == null);

/** Durations offered for acknowledging and muting, in seconds. */
export const DURATIONS = [
  { secs: 3600, label: "1 hour" },
  { secs: 8 * 3600, label: "8 hours" },
  { secs: 86400, label: "1 day" },
  { secs: 7 * 86400, label: "1 week" },
  { secs: 30 * 86400, label: "30 days" },
] as const;

/** Custom duration units, in seconds. */
export const UNITS = [
  { secs: 60, label: "minutes" },
  { secs: 3600, label: "hours" },
  { secs: 86400, label: "days" },
  { secs: 7 * 86400, label: "weeks" },
] as const;

/** A custom duration in seconds, or null when out of range (a minute to a year). */
export function customSecs(n: number, unit: number): number | null {
  const s = Math.round(n * unit);
  return Number.isFinite(s) && s >= 60 && s <= 366 * 86400 ? s : null;
}

/** "until it clears", "for 3h 20m more", "for good" (and "unless it gets worse" for acknowledgements). */
export function silenceWhen(s: Silence, now = Math.floor(Date.now() / 1000)): string {
  const worse = s.mode === "ack" ? ", unless it gets worse" : "";
  if (s.until != null) return `${ago(now, s.until)} left${worse}`;
  if (s.mode === "ack" && s.untilClear) return `until it clears${worse}`;
  return `for good${worse}`;
}

/** What a mute covers, in words, for the scope picker. */
export function scopeLabel(scope: MuteScope["type"], a: Alert, hostName: string | null): string {
  const kind = KIND_LABEL[a.kind];
  switch (scope) {
    case "alert":
      return "This alert only";
    case "kindOnHost":
      return `Every ${kind} on ${hostName ?? "this host"}`;
    case "host":
      return `Everything on ${hostName ?? "this host"}`;
    case "kind":
      return `Every ${kind} on every host`;
  }
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

/**
 * The sound for a new snapshot (D-178): the worst new alert, else a clear
 * when one went away. Nothing for the first snapshot (alerts already active
 * at launch) or for alerts folded under a cause.
 */
export function alertCue(prev: AlertsSnapshot | null, next: AlertsSnapshot): "alertCritical" | "alertWarn" | "alertClear" | null {
  if (!prev) return null;
  const before = prev.active.map((a) => a.id);
  const now = next.active.map((a) => a.id);
  const fresh = next.active.filter((a) => !before.includes(a.id) && a.causedBy == null && a.silenced == null);
  if (fresh.some((a) => a.severity === "critical")) return "alertCritical";
  if (fresh.length) return "alertWarn";
  if (prev.active.some((a) => !now.includes(a.id) && a.causedBy == null && a.silenced == null)) return "alertClear";
  return null;
}

export type Tone = "ok" | "warn" | "bad";
export function tone(s: AlertsSnapshot | null): Tone {
  const act = loud(s?.active ?? []);
  if (!act.length) return "ok";
  return act.some((a) => a.severity === "critical") ? "bad" : "warn";
}

class AlertsStore {
  snap = $state<AlertsSnapshot | null>(null);
  open = $state(false);
  private started = false;

  get active(): Alert[] {
    return this.snap?.active ?? [];
  }

  /** Active alerts without those folded under a cause, acknowledged or muted. */
  get roots(): Alert[] {
    return groupByCause(loud(this.active)).map((g) => g.alert);
  }

  /** Acknowledgements and mutes, newest first (D-196). */
  get silences(): Silence[] {
    return this.snap?.silences ?? [];
  }

  /** Active alerts a silence covers. */
  covered(s: Silence): Alert[] {
    return this.active.filter((a) => a.silenced === s.id);
  }

  async ack(a: Alert, how: AckFor) {
    await api.alertsAck(a.key, how);
  }

  async mute(a: Alert, scope: MuteScope, secs: number | null) {
    await api.alertsMute(a.key, scope, secs);
  }

  async unsilence(id: number) {
    await api.alertsUnsilence(id);
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
    void api.alertsSubscribe((s) => {
      const cue = alertCue(this.snap, s);
      this.snap = s;
      if (cue) sound.play(cue);
    });
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
      case "netNewDevice":
      case "netMissing":
      case "netRisk":
        ui.go("network");
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
  async mutePattern(hostId: number, source: string, fingerprint: string) {
    const s = await api.alertsLogMute(hostId, source, fingerprint);
    settings.value = s;
  }

  clearHistory() {
    void api.alertsClearHistory();
  }
}

export const alerts = new AlertsStore();
