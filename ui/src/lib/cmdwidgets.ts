/**
 * Custom command widgets (M10x): pure helpers. The real checks (read-only
 * commands, no sudo) run in core/src/cmdwidget.rs; these give quick hints.
 */
import type { CmdReading_Serialize as CmdReading, CmdWidget_Serialize as CmdWidget } from "./bindings";

export type { CmdReading, CmdWidget };

export const PARSES: { id: string; label: string; what: string }[] = [
  { id: "number", label: "First number", what: "The first number in the output, e.g. 42 from \"42 users\"" },
  { id: "regex", label: "Regex", what: "The first group of a regular expression (or the whole match)" },
  { id: "lines", label: "Line count", what: "How many non-empty lines it printed" },
  { id: "exit", label: "Exit code", what: "OK when it exits 0, FAIL otherwise" },
  { id: "text", label: "Text", what: "The first line as it is" },
];

export const INTERVALS: [number, string][] = [
  [10, "10 s"],
  [30, "30 s"],
  [60, "1 min"],
  [300, "5 min"],
  [900, "15 min"],
  [3600, "1 h"],
  [21600, "6 h"],
  [86400, "24 h"],
];

export const blank = (hostId: number): CmdWidget => ({
  id: 0,
  hostId,
  name: "",
  command: "",
  parse: "number",
  pattern: "",
  unit: "",
  warn: null,
  crit: null,
  below: false,
  intervalSecs: 60,
});

/** Ready-made starting points; all read-only and portable. */
export const EXAMPLES: { label: string; w: Partial<CmdWidget> }[] = [
  { label: "Root disk used", w: { name: "Root used", command: "df -P / | tail -1", parse: "regex", pattern: "(\\d+)%", unit: "%", warn: 80, crit: 90 } },
  { label: "Load (1 min)", w: { name: "Load", command: "cat /proc/loadavg", parse: "number", warn: 4, crit: 8 } },
  { label: "Logged-in users", w: { name: "Users", command: "who", parse: "lines" } },
  { label: "Failed units", w: { name: "Failed units", command: "systemctl --failed --no-legend --plain", parse: "lines", warn: 1, crit: 3 } },
  { label: "Running containers", w: { name: "Containers", command: "docker ps -q", parse: "lines" } },
  { label: "ZFS pool state", w: { name: "Pool", command: "zpool status -x", parse: "text" } },
  { label: "Port 443 listening", w: { name: "HTTPS up", command: "ss -ltn | grep -q ':443 '", parse: "exit" } },
];

const ELEVATE = new Set(["sudo", "doas", "su", "pkexec", "run0"]);
// Same words danger.rs looks for first; the backend has the final say.
const WRITES = /(^|[\s;|&(`])(rm|rmdir|kill|killall|pkill|reboot|shutdown|poweroff|halt|mkfs(\.\w+)?|dd|truncate|shred)(\s|$)|\bprune\b|\bcompose\b.*\bdown\b|\b(apt|apt-get|dnf|yum|pacman|zypper)\b.*\b(upgrade|dist-upgrade|full-upgrade)\b|>\s*[^&\s/]|>\s*\/(?!dev\/)/;

/** A hint shown before saving, or null. */
export function commandHint(cmd: string): string | null {
  const c = cmd.trim();
  if (!c) return null;
  if (c.split(/[\s;|&()`]+/).some((w) => ELEVATE.has(w))) return "Widgets run unattended, so sudo can't be used. Pick a command your user can run.";
  if (WRITES.test(c)) return "Widgets refresh on their own, so the command must only read. This one looks like it changes something.";
  return null;
}

/** Problems with the form, by field. */
export function problems(w: CmdWidget): Record<string, string> {
  const p: Record<string, string> = {};
  if (!w.name.trim()) p.name = "Name it";
  else if ([...w.name.trim()].length > 40) p.name = "At most 40 characters";
  if (!w.command.trim()) p.command = "Type a command";
  else if (w.command.length > 1000) p.command = "At most 1000 characters";
  else {
    const h = commandHint(w.command);
    if (h) p.command = h;
  }
  if (w.parse === "regex") {
    if (!w.pattern) p.pattern = "Type a pattern";
    else
      try {
        new RegExp(w.pattern);
      } catch {
        p.pattern = "Not a valid pattern";
      }
  }
  if (w.warn != null && w.crit != null && (w.below ? w.crit > w.warn : w.crit < w.warn))
    p.crit = w.below ? "Critical must be at or below warning" : "Critical must be at or above warning";
  return p;
}

/** The big value on the tile. */
export function display(r: CmdReading | undefined, unit: string): string {
  if (!r) return "--";
  if (r.value != null && r.level !== "error" && /^-?\d/.test(r.text)) return `${r.text}${unit && r.text !== "OK" ? unit : ""}`;
  return r.text || "--";
}

/** Text for numbers typed in the form (blank = no threshold). */
export function parseThreshold(s: string): number | null {
  const t = s.trim();
  if (!t) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

export const intervalLabel = (s: number): string => INTERVALS.find(([n]) => n === s)?.[1] ?? (s >= 60 ? `${Math.round(s / 60)} min` : `${s} s`);
