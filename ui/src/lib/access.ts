/** Login & access audit helpers (M10u): summaries and key names. */
import type { AccessReport, SshKey } from "./api";
import type { Finding } from "./bindings";

export type Tone = "ok" | "warn" | "bad" | "muted";

/** The worst finding's tone (`ok` when there are none). */
export function worstTone(findings: Finding[]): Tone {
  if (findings.some((f) => f.level === "bad")) return "bad";
  if (findings.some((f) => f.level === "warn")) return "warn";
  return "ok";
}

/** Addresses banned right now, across fail2ban jails and CrowdSec. */
export function bannedNow(r: AccessReport): number {
  return r.jails.reduce((a, j) => a + j.currentlyBanned, 0) + r.decisions.length;
}

/** What the report couldn't see without root, in words. */
export function limits(r: AccessReport): string[] {
  if (r.asRoot) return [];
  const out: string[] = [];
  if (r.logSource === "none") out.push("logins and failed attempts");
  if (r.fail2ban === "denied") out.push("fail2ban");
  if (r.crowdsec === "denied") out.push("CrowdSec");
  if (!r.sudoersReadable) out.push("sudo rules");
  out.push("other users' keys");
  return out;
}

/** The local key a fingerprint belongs to, by file name or comment. */
export function keyName(fingerprint: string | null, local: Pick<SshKey, "fingerprint" | "file" | "comment">[]): string | null {
  if (!fingerprint) return null;
  const k = local.find((x) => x.fingerprint === fingerprint);
  if (!k) return null;
  return k.file?.split(/[\\/]/).pop() || k.comment || null;
}

/** `SHA256:abcdefgh…` → `abcdefgh…` shortened for tables. */
export function shortFp(fp: string): string {
  const s = fp.replace(/^SHA256:/, "");
  return s.length > 12 ? `${s.slice(0, 12)}…` : s;
}

/** "5 min ago", "3 h ago", "2 d ago". */
export function ago(ts: number, now = Date.now() / 1000): string {
  const d = Math.max(0, now - ts);
  if (d < 90) return "just now";
  if (d < 3600) return `${Math.round(d / 60)} min ago`;
  if (d < 86_400 * 2) return `${Math.round(d / 3600)} h ago`;
  return `${Math.round(d / 86_400)} d ago`;
}

/** Plain-text summary for DRIFT and the clipboard. */
export function reportText(r: AccessReport, host: string): string {
  const lines = [`Access audit of ${host} (${r.asRoot ? "as root" : `as ${r.loginUser}, without root`}), last ${r.days} days:`];
  for (const f of r.findings) lines.push(`- [${f.level}] ${f.text}`);
  lines.push(`Logins: ${r.logins.length}; failed attempts: ${r.failedTotal} from ${r.failures.length} addresses; banned now: ${bannedNow(r)}.`);
  for (const l of r.logins.slice(0, 10)) lines.push(`  login ${new Date(l.ts * 1000).toISOString()} ${l.user} from ${l.from} (${l.method})`);
  for (const f of r.failures.slice(0, 10)) lines.push(`  failed ${f.count}x from ${f.from} (users: ${f.users.join(", ")})`);
  lines.push(`Users with a shell: ${r.users.map((u) => `${u.name}${u.sudo ? " (sudo)" : ""}`).join(", ")}.`);
  for (const s of r.sshd) lines.push(`  sshd ${s.key} ${s.value}`);
  return lines.join("\n");
}
