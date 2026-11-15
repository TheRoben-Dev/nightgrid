/**
 * SSH key manager helpers (M10k): which key a host uses, which keys each
 * host accepts, and the per-host rotation plan. Pure, so they're testable.
 */
import type { AuthorizedKey, Host, HostInput, SshKey } from "./api";

/** `SHA256:abcdefgh…wxyz`: enough to tell keys apart at a glance. */
export function shortFp(fp: string): string {
  const [alg, body = ""] = fp.split(":", 2);
  return body.length > 14 ? `${alg}:${body.slice(0, 8)}…${body.slice(-4)}` : fp;
}

/** Home directory from the scanned `.ssh` directory. */
export function homeOf(sshDir: string): string {
  return sshDir.replace(/[\\/]\.ssh[\\/]?$/, "");
}

const normPath = (p: string, home: string) =>
  (p.startsWith("~/") || p.startsWith("~\\") ? home + p.slice(1) : p).replace(/\\/g, "/").replace(/\/+/g, "/");

/** The key file a host is set to use, if it's one of `keys`. */
export function keyForHost(h: Pick<Host, "identityFile">, keys: SshKey[], home: string): SshKey | undefined {
  if (!h.identityFile) return undefined;
  const want = normPath(h.identityFile, home);
  return keys.find((k) => k.file && normPath(k.file, home) === want);
}

/** Short name of a key: its file name, else its comment, else the fingerprint. */
export function keyLabel(k: Pick<SshKey, "file" | "comment" | "fingerprint">): string {
  const base = k.file?.split(/[\\/]/).pop();
  return base || k.comment || shortFp(k.fingerprint);
}

/** What reading one host's authorized_keys gave. */
export interface HostKeys {
  hostId: number;
  entries: AuthorizedKey[] | null;
  error: string | null;
}

/** Fingerprints a host accepts (empty when it couldn't be read). */
export function accepted(r: HostKeys | undefined): Set<string> {
  return new Set((r?.entries ?? []).map((e) => e.fingerprint));
}

/** Keys on the host that aren't any of the user's keys. */
export function unknownKeys(r: HostKeys | undefined, keys: SshKey[]): AuthorizedKey[] {
  const mine = new Set(keys.map((k) => k.fingerprint));
  const seen: string[] = [];
  return (r?.entries ?? []).filter((e) => {
    if (mine.has(e.fingerprint) || seen.includes(e.fingerprint)) return false;
    seen.push(e.fingerprint);
    return true;
  });
}

/** Hosts (read successfully) that accept `fingerprint`. */
export function hostsAccepting(results: HostKeys[], fingerprint: string): number[] {
  return results.filter((r) => r.entries?.some((e) => e.fingerprint === fingerprint)).map((r) => r.hostId);
}

/** The host's settings, switched to log in with `file`. Agent hosts keep the
 * agent and prefer this key; password hosts move to key login. */
export function switchedInput(h: Host, file: string): HostInput {
  return {
    ...h,
    port: h.port === 22 ? null : h.port,
    tags: [...h.tags],
    identityFile: file,
    authMethod: h.authMethod === "password" ? "key" : h.authMethod,
  };
}

/** A free file name for a new key: `id_ed25519_nightgrid`, then `-2`, `-3`… */
export function freeKeyName(keys: SshKey[], base = "id_ed25519_nightgrid"): string {
  const taken = new Set(keys.map((k) => k.file?.split(/[\\/]/).pop()).filter(Boolean));
  if (!taken.has(base)) return base;
  for (let i = 2; ; i++) if (!taken.has(`${base}-${i}`)) return `${base}-${i}`;
}

/** Valid key file name (same rule as the backend). */
export function validKeyName(n: string): boolean {
  return /^[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}$/.test(n) && !n.endsWith(".pub");
}

/** One step of rotating a key on one host. */
export type RotateStep = "push" | "verify" | "switch" | "remove";
export const ROTATE_STEPS: RotateStep[] = ["push", "verify", "switch", "remove"];
export const STEP_LABEL: Record<RotateStep, string> = {
  push: "add new key",
  verify: "log in with it",
  switch: "use it in Nightgrid",
  remove: "remove old key",
};

/** Steps for one host. The old key is only removed after the new one logged in. */
export function rotationSteps(opts: { switchHosts: boolean; removeOld: boolean }): RotateStep[] {
  return ROTATE_STEPS.filter((s) => (s === "switch" ? opts.switchHosts : s === "remove" ? opts.removeOld : true));
}
