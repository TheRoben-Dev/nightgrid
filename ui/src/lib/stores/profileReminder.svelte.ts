/**
 * Reminds to export an encrypted profile every N days (default weekly).
 * The last export time is recorded by the backend on every export, to a file
 * or the sync folder; "Tomorrow" snoozes for a day.
 */
import { helper } from "./helper.svelte";
import { hosts } from "./hosts.svelte";
import { settings } from "./settings.svelte";

export const REMINDER_STEPS = [0, 7, 14, 30] as const;

export function reminderWords(days: number): string {
  if (days === 0) return "Never";
  if (days === 7) return "Weekly";
  if (days === 14) return "Every 2 weeks";
  if (days === 30) return "Monthly";
  return `Every ${days} days`;
}

/** Whole days since the last export, or null if there never was one. */
export function daysSince(last: number | null, now: number): number | null {
  return last == null ? null : Math.floor((now - last) / 86400);
}

/** Is a reminder due? Only once there's something worth saving (a host). */
export function reminderDue(o: { days: number; last: number | null; snooze: number | null; firstSeen: number; hostCount: number }, now: number): boolean {
  if (o.days <= 0 || o.hostCount === 0) return false;
  if (o.snooze != null && now < o.snooze) return false;
  const since = o.last ?? o.firstSeen;
  return now - since >= o.days * 86400;
}

const FIRST_SEEN_KEY = "nightgrid.firstSeen";

/** When this install first ran (kept per machine in local storage). */
function firstSeen(now: number): number {
  try {
    const v = Number(localStorage.getItem(FIRST_SEEN_KEY));
    if (v > 0) return v;
    localStorage.setItem(FIRST_SEEN_KEY, String(now));
  } catch {
    /* storage unavailable */
  }
  return now;
}

class ProfileReminder {
  now = $state(Date.now() / 1000);
  private timer: ReturnType<typeof setInterval> | null = null;
  private first = firstSeen(Date.now() / 1000);

  get due(): boolean {
    const s = settings.value;
    return (
      settings.loaded &&
      reminderDue(
        { days: s.profileReminderDays, last: s.lastProfileExport, snooze: s.profileReminderSnooze, firstSeen: this.first, hostCount: hosts.hosts.length },
        this.now,
      )
    );
  }

  get message(): string {
    const d = daysSince(settings.value.lastProfileExport, this.now);
    return d === null ? "You haven't exported a profile backup yet." : `Your last profile backup was ${d} day${d === 1 ? "" : "s"} ago.`;
  }

  start() {
    if (this.timer) return;
    this.timer = setInterval(() => (this.now = Date.now() / 1000), 15 * 60 * 1000);
  }

  exportNow() {
    void helper.point("settings-backup", "Pick a passphrase and export. Tick saved secrets to bring passwords and tokens along.");
  }

  snooze() {
    void settings.update({ profileReminderSnooze: Date.now() / 1000 + 86400 });
  }
}

export const profileReminder = new ProfileReminder();
