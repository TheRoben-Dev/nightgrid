import type { BackupSummary } from "../api";

export const MIN_PASSPHRASE = 8;

/** Why the export passphrase can't be used yet, or null when it's fine. */
export function passphraseProblem(pass: string, confirm: string): string | null {
  if ([...pass].length < MIN_PASSPHRASE) return `At least ${MIN_PASSPHRASE} characters.`;
  if (pass !== confirm) return "The passphrases don't match.";
  return null;
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** "4 hosts · 2 groups · …", skipping empty kinds. */
export function summaryLines(s: BackupSummary): string[] {
  const parts: [number, string, string?][] = [
    [s.hosts, "host"],
    [s.groups, "group"],
    [s.tags, "tag"],
    [s.snippets, "snippet"],
    [s.playbooks, "playbook"],
    [s.services, "service"],
    [s.tunnels, "tunnel"],
    [s.integrations, "integration"],
    [s.bookmarks, "bookmark"],
    [s.secrets, "saved secret"],
  ];
  const out = parts.filter(([n]) => n > 0).map(([n, one, many]) => plural(n, one, many));
  if (s.settings) out.push("app settings");
  return out;
}

export function fmtDate(unix: number | null): string {
  if (unix === null) return "an unknown date";
  return new Date(unix * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}
