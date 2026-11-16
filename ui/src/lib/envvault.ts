/** Pure helpers for the .env vault (M10w). Same rules as core/src/envvault.rs. */
import type { EnvDiff } from "./bindings";

/** Letters, digits and `_`, not starting with a digit, at most 128 chars. */
export const validName = (n: string): boolean => /^[A-Za-z_][A-Za-z0-9_]*$/.test(n) && n.length <= 128;

const SECRET_WORDS = ["pass", "secret", "token", "key", "auth", "credential", "private", "salt", "cookie", "session", "dsn", "database_url", "db_url", "cert", "webhook"];

/** Whether a name says it holds a secret: such variables start masked. */
export const looksSecret = (name: string): boolean => {
  const n = name.toLowerCase();
  return SECRET_WORDS.some((w) => n.includes(w));
};

/** Whether deploying changes nothing. */
export const diffEmpty = (d: EnvDiff): boolean => d.exists && !d.added.length && !d.changed.length && !d.removed.length;

/** One line for the deploy dialog, e.g. "2 added, 1 changed, 1 removed, 3 unchanged". */
export function diffSummary(d: EnvDiff): string {
  const parts: string[] = [];
  if (d.added.length) parts.push(`${d.added.length} added`);
  if (d.changed.length) parts.push(`${d.changed.length} changed`);
  if (d.removed.length) parts.push(`${d.removed.length} removed`);
  if (d.unchanged) parts.push(`${d.unchanged} unchanged`);
  return parts.join(", ") || "nothing to write";
}

/** Masked placeholder: never hints at the value's length. */
export const MASK = "••••••••";

/** Splits pasted `NAME=value` into its parts (for the add form). */
export function splitPaste(text: string): { name: string; value: string } | null {
  const m = /^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=(.*)$/s.exec(text);
  if (!m) return null;
  let v = m[2].trim();
  if (v.length >= 2 && ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'")))) v = v.slice(1, -1);
  return { name: m[1], value: v };
}
