/** Security → SECRETS (1.1, D-169): labels and sums for the rotation list. */
import type { RotationState, SecretItem, SecretKind } from "./api";

const DAY = 86_400;

export const KINDS: { id: SecretKind; label: string }[] = [
  { id: "sshKey", label: "SSH keys" },
  { id: "hostPassword", label: "Host passwords" },
  { id: "vault", label: ".env vault" },
  { id: "integration", label: "Integrations" },
  { id: "appToken", label: "Nightgrid tokens" },
  { id: "expiry", label: "Certificates & expiry" },
  { id: "manual", label: "Listed by hand" },
];

export const kindLabel = (k: SecretKind) => KINDS.find((x) => x.id === k)?.label ?? k;

export const stateLabel: Record<RotationState, string> = {
  overdue: "OVERDUE",
  dueSoon: "DUE SOON",
  unknown: "NO DATE",
  ok: "OK",
  noPolicy: "NO POLICY",
};

export const stateTone: Record<RotationState, "bad" | "warn" | "muted" | "ok"> = {
  overdue: "bad",
  dueSoon: "warn",
  unknown: "muted",
  ok: "ok",
  noPolicy: "muted",
};

/** Policy choices in days; 0 = no policy. */
export const POLICIES = [30, 90, 180, 365, 730, 0];

export const policyLabel = (d: number) => (d === 0 ? "never" : d % 365 === 0 ? `${d / 365} year${d === 365 ? "" : "s"}` : `${d} days`);

function span(sec: number): string {
  const d = Math.round(Math.abs(sec) / DAY);
  if (d < 1) return "today";
  if (d < 60) return `${d} day${d === 1 ? "" : "s"}`;
  const m = Math.round(d / 30);
  if (m < 24) return `${m} months`;
  return `${Math.round(d / 365)} years`;
}

/** "rotated 14 months ago", "never stamped". */
export function ageText(i: SecretItem, now: number): string {
  if (i.kind === "expiry") return i.rotatedAt ? `renewed ${span(now - i.rotatedAt)} ago` : "";
  return i.rotatedAt ? `rotated ${span(now - i.rotatedAt)} ago` : "no date yet";
}

/** "due in 20 days", "3 months overdue", "expires in 9 days". */
export function dueText(i: SecretItem, now: number): string {
  if (i.dueAt == null) return i.state === "noPolicy" ? "no rotation policy" : i.kind === "expiry" ? "date not read yet" : "mark it rotated to start the clock";
  const left = i.dueAt - now;
  if (i.kind === "expiry") return left <= 0 ? `expired ${span(left)} ago` : `expires in ${span(left)}`;
  return left <= 0 ? `${span(left)} overdue` : `due in ${span(left)}`;
}

export function summary(items: SecretItem[]) {
  const count = (s: RotationState) => items.filter((i) => i.state === s).length;
  return { total: items.length, overdue: count("overdue"), soon: count("dueSoon"), unknown: count("unknown") };
}

/** The fingerprint of an `ssh:` key, else null. */
export const fingerprintOf = (key: string) => (key.startsWith("ssh:") ? key.slice(4) : null);
export const manualId = (key: string) => (key.startsWith("manual:") ? Number(key.slice(7)) : null);
