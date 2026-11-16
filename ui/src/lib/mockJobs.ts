/**
 * Browser-only stand-in for the cron and timer editor (M10v): crontabs and
 * Nightgrid timers kept in memory, edited like the backend does (simplified).
 */
import type {
  CalendarPreview,
  CronEdit,
  Crontab_Serialize as Crontab,
  CrontabJob_Serialize as CrontabJob,
  InstalledJob_Serialize as InstalledJob,
  TimerJob_Serialize as TimerJob,
} from "./bindings";

const OFF = "#nightgrid-off ";
const tabs: Record<string, string> = {
  me: "MAILTO=admin@example.com\n# nightly backup\n0 3 * * * /usr/local/bin/backup.sh --quiet\n#nightgrid-off */5 * * * * curl -fsS https://example.com/ping\n@reboot sleep 30 && mount -a\n",
  root: "15 4 * * 0 /usr/sbin/fstrim -av\n",
};
let timers: InstalledJob[] = [
  { unit: "nightgrid-job-prune-images", job: { name: "Prune images", command: "docker image prune -af --filter until=168h", calendar: "Sun *-*-* 05:00", runAs: null, persistent: true } },
];

const FIELD = /^(\*|\d+(-\d+)?)(\/\d+)?(,(\*|\d+(-\d+)?)(\/\d+)?)*$/;
function describe(s: string): string | null {
  const t = s.trim().split(/\s+/).join(" ");
  const at: Record<string, string> = { "@reboot": "at boot", "@daily": "daily at 00:00", "@hourly": "hourly at :00", "@weekly": "weekly on Sun at 00:00", "@monthly": "monthly on the 1st at 00:00" };
  if (at[t]) return at[t];
  const f = t.split(" ");
  if (f.length !== 5 || !f.every((x) => FIELD.test(x) || /^[a-z]{3}(-[a-z]{3})?$/i.test(x))) return null;
  if (/^\d+$/.test(f[0]) && /^\d+$/.test(f[1]) && f.slice(2).every((x) => x === "*")) return `daily at ${f[1].padStart(2, "0")}:${f[0].padStart(2, "0")}`;
  if (f[0].startsWith("*/") && f.slice(1).every((x) => x === "*")) return `every ${f[0].slice(2)} minutes`;
  return t;
}
function hash(t: string): string {
  let h = 0;
  for (let i = 0; i < t.length; i++) h = (h * 31 + t.charCodeAt(i)) | 0;
  return (h >>> 0).toString(16);
}
function split(line: string): [string, string] | null {
  const t = line.trim();
  if (!t || t.startsWith("#") || /^[A-Za-z_][A-Za-z0-9_]*\s*=/.test(t)) return null;
  const toks = t.split(/\s+/);
  const n = toks[0].startsWith("@") ? 1 : 5;
  if (toks.length <= n) return null;
  return [toks.slice(0, n).join(" "), toks.slice(n).join(" ")];
}
function parse(user: string | null, text: string): Crontab {
  const jobs: CrontabJob[] = [];
  const env: string[] = [];
  text.split("\n").forEach((l, i) => {
    const off = l.startsWith(OFF);
    const body = off ? l.slice(OFF.length) : l;
    if (!off && /^[A-Za-z_][A-Za-z0-9_]*\s*=/.test(l.trim())) return void env.push(l.trim());
    const j = split(body);
    if (j) jobs.push({ line: i + 1, schedule: j[0], every: describe(j[0]) ?? j[0], command: j[1], enabled: !off, next: off || j[0] === "@reboot" ? null : Math.floor(Date.now() / 1000) + 3600 * (i + 1) });
  });
  return { user, text, hash: hash(text), jobs, env };
}
const key = (user: string | null) => user ?? "me";
const wait = () => new Promise((r) => setTimeout(r, 200));

export const mockJobs = {
  async read(user: string | null): Promise<Crontab> {
    await wait();
    return parse(user, tabs[key(user)] ?? "");
  },
  async apply(user: string | null, h: string, edit: CronEdit): Promise<Crontab> {
    await wait();
    const text = tabs[key(user)] ?? "";
    if (hash(text) !== h) throw new Error("the crontab changed on the host since it was read: reload it and try again");
    const lines = text.replace(/\n$/, "").split("\n").filter((l, i, a) => l || i < a.length - 1);
    const check = (s: string, c: string) => {
      if (!describe(s)) throw new Error("schedule: not a cron schedule (five fields such as `*/5 * * * *`, or @daily)");
      if (!c.trim()) throw new Error("command: empty");
      if (/(^|[^\\])%/.test(c)) throw new Error("command: cron treats % as a line break: write \\% (for example date +\\%F)");
      return `${s.trim().split(/\s+/).join(" ")} ${c.trim()}`;
    };
    if (edit.op === "add") lines.push(check(edit.schedule, edit.command));
    else {
      const i = edit.line - 1;
      if (!lines[i] || !split(lines[i].replace(OFF, ""))) throw new Error("line: not a job");
      const off = lines[i].startsWith(OFF);
      if (edit.op === "update") lines[i] = (off ? OFF : "") + check(edit.schedule, edit.command);
      else if (edit.op === "remove") lines.splice(i, 1);
      else lines[i] = edit.enabled ? lines[i].replace(OFF, "") : off ? lines[i] : OFF + lines[i];
    }
    tabs[key(user)] = lines.join("\n") + "\n";
    return parse(user, tabs[key(user)]);
  },
  async describe(s: string): Promise<string | null> {
    return describe(s);
  },
  async timers(): Promise<InstalledJob[]> {
    await wait();
    return timers;
  },
  async saveTimer(job: TimerJob, replaces: string | null): Promise<InstalledJob> {
    await wait();
    if (!job.name.trim() || !job.command.trim() || !job.calendar.trim()) throw new Error("name, command and schedule are needed");
    const unit = `nightgrid-job-${job.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`;
    const j = { unit, job: { ...job, name: job.name.trim(), runAs: job.runAs?.trim() || null } };
    timers = [...timers.filter((t) => t.unit !== unit && t.unit !== replaces), j];
    return j;
  },
  async removeTimer(unit: string): Promise<null> {
    await wait();
    timers = timers.filter((t) => t.unit !== unit);
    return null;
  },
  async preview(c: string): Promise<CalendarPreview> {
    await wait();
    if (/nope|;/.test(c)) return { ok: false, normalized: `Failed to parse calendar specification '${c}': Invalid argument`, next: [] };
    return { ok: true, normalized: c.includes(":") ? c : "*-*-* 00:00:00", next: ["Tue 2026-11-17 03:00:00 UTC", "Wed 2026-11-18 03:00:00 UTC", "Thu 2026-11-19 03:00:00 UTC"] };
  },
};
