/** Lab blueprints (0.10.82): wording and step grouping for Fleet → BLUEPRINTS. */
import type { BpStep, BpStepKind, BpStepResult, BpStepState } from "./api";

export const MIN_PASSPHRASE = 8;

export const STATE_WORD: Record<BpStepState, string> = { new: "NEW", same: "THERE", differs: "DIFFERS", manual: "BY HAND" };

export const KIND_LABEL: Record<BpStepKind, string> = {
  stackFiles: "Stacks",
  stackStart: "Stacks",
  cron: "Cron",
  timer: "Timers",
  firewall: "Firewall",
  caddy: "Proxy",
  traefik: "Proxy",
};

/** Steps grouped by section, in plan order. */
export function groups(steps: BpStep[]): { label: string; steps: BpStep[] }[] {
  const out: { label: string; steps: BpStep[] }[] = [];
  for (const s of steps) {
    const label = KIND_LABEL[s.kind];
    const g = out.find((x) => x.label === label);
    if (g) g.steps.push(s);
    else out.push({ label, steps: [s] });
  }
  return out;
}

/** Steps that can be ticked (manual ones can't). */
export const pickable = (s: BpStep): boolean => s.state !== "manual";

export function initialPick(steps: BpStep[]): Set<string> {
  return new Set(steps.filter((s) => s.defaultOn && pickable(s)).map((s) => s.id));
}

/**
 * Starting a stack only makes sense once its files are written: ticking a
 * start ticks its files step when that is new, unticking files unticks start.
 */
export function toggleStep(picked: Set<string>, steps: BpStep[], id: string): Set<string> {
  const next = new Set(picked);
  const on = !next.has(id);
  if (on) next.add(id);
  else next.delete(id);
  const [kind, name] = id.split(/:(.*)/s);
  if (kind === "start" && on) {
    const files = steps.find((s) => s.id === `stack:${name}`);
    if (files?.state === "new") next.add(files.id);
  }
  if (kind === "stack" && !on) next.delete(`start:${name}`);
  return next;
}

export function passphraseProblem(a: string, b: string): string | null {
  if (a.length < MIN_PASSPHRASE) return `Use at least ${MIN_PASSPHRASE} characters.`;
  if (a !== b) return "The two passphrases don't match.";
  return null;
}

export function resultsLine(r: BpStepResult[]): string {
  const ok = r.filter((x) => x.ok).length;
  if (r.length === ok) return ok === 1 ? "The step was applied." : `All ${ok} steps applied.`;
  return `${ok} of ${r.length} steps applied, ${r.length - ok} failed.`;
}
