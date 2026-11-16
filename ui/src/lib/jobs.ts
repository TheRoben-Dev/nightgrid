/** Cron and timer editor helpers (M10v). */
import type { InstalledJob, Timer, TimerJob } from "./api";

export const CRON_PRESETS = [
  { label: "every 5 minutes", value: "*/5 * * * *" },
  { label: "every hour", value: "0 * * * *" },
  { label: "daily at 03:00", value: "0 3 * * *" },
  { label: "weekdays at 08:00", value: "0 8 * * 1-5" },
  { label: "weekly, Sunday 04:00", value: "0 4 * * 0" },
  { label: "monthly, 1st at 05:00", value: "0 5 1 * *" },
  { label: "at boot", value: "@reboot" },
] as const;

export const TIMER_PRESETS = [
  { label: "every 15 minutes", value: "*:0/15" },
  { label: "hourly", value: "hourly" },
  { label: "daily at 03:00", value: "*-*-* 03:00:00" },
  { label: "weekdays at 08:00", value: "Mon..Fri *-*-* 08:00:00" },
  { label: "weekly, Sunday 04:00", value: "Sun *-*-* 04:00:00" },
  { label: "monthly, 1st at 05:00", value: "*-*-01 05:00:00" },
] as const;

export const JOB_PREFIX = "nightgrid-job-";

/** The Nightgrid job behind a timer row (`nightgrid-job-x.timer`), if any. */
export function jobFor(t: Pick<Timer, "name">, list: InstalledJob[]): InstalledJob | null {
  if (!t.name.startsWith(JOB_PREFIX)) return null;
  const unit = t.name.replace(/\.timer$/, "");
  return list.find((j) => j.unit === unit) ?? null;
}

/** The unit a job name becomes (mirrors the backend's slug). */
export function unitFor(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40)
    .replace(/-+$/, "");
  return slug ? `${JOB_PREFIX}${slug}` : "";
}

export function blankJob(): TimerJob {
  return { name: "", command: "", calendar: "*-*-* 03:00:00", runAs: null, persistent: true };
}
