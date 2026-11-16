/**
 * Grounded triage for log alerts and notes: DRIFT gets the attached events
 * as Nightgrid's log rules judge them, the container's real state, a verdict
 * and commands that show something as written. No "usual causes" guesswork.
 */
import type { ContainerDetail, LogEventView } from "../api";
import type { Check, CheckItem } from "./skills";

/** What Nightgrid read about the container (null fields: couldn't tell). */
export interface ContainerFacts {
  name: string;
  state: string | null;
  health: string | null;
  /** Unix seconds. */
  startedAt: number | null;
  restartCount: number | null;
  exitCode: number | null;
  /** Why the state couldn't be read. */
  error: string | null;
}

export function factsFrom(name: string, d: ContainerDetail | null, error: string | null): ContainerFacts {
  if (!d) return { name, state: null, health: null, startedAt: null, restartCount: null, exitCode: null, error: error ?? "not read" };
  const t = d.startedAt ? Date.parse(d.startedAt) : NaN;
  return {
    name,
    state: d.state || null,
    health: d.health,
    startedAt: Number.isFinite(t) ? Math.floor(t / 1000) : null,
    restartCount: d.restartCount,
    exitCode: d.state === "exited" ? d.exitCode : null,
    error: null,
  };
}

function span(secs: number): string {
  if (secs < 90) return `${Math.max(0, Math.round(secs))} s`;
  const m = Math.round(secs / 60);
  if (m < 90) return `${m} min`;
  const h = Math.floor(m / 60);
  return h < 48 ? `${h} h ${m % 60} min` : `${Math.floor(h / 24)} days`;
}

/** "running · healthy · up 3 h 12 min · restarted 0 times". */
export function stateLine(c: ContainerFacts, now: number): string {
  if (!c.state) return `unknown (couldn't read it: ${c.error ?? "not read"})`;
  const bits = [c.state];
  bits.push(c.health ? c.health : "no health check");
  if (c.state === "running" && c.startedAt) bits.push(`up ${span(now - c.startedAt)}`);
  if (c.exitCode != null) bits.push(`exit code ${c.exitCode}`);
  if (c.restartCount != null) bits.push(`restarted ${c.restartCount} time${c.restartCount === 1 ? "" : "s"}`);
  return bits.join(" · ");
}

export type Verdict = "harmless" | "problem" | "unclear";

/** Is the container fine right now? null when unknown. */
function healthy(c: ContainerFacts | null): boolean | null {
  if (!c?.state) return null;
  return c.state === "running" && c.health !== "unhealthy";
}

export function verdictOf(events: LogEventView[], c: ContainerFacts | null): Verdict {
  const fine = healthy(c);
  if (c?.state && (c.state === "restarting" || c.state === "exited" || c.state === "dead" || c.health === "unhealthy")) return "problem";
  if (events.some((e) => e.level === "fatal")) return "problem";
  const real = events.filter((e) => !e.harmless && (e.level === "error" || e.level === "fatal"));
  if (!real.length && events.length && fine !== false) return "harmless";
  return real.length >= 3 ? "problem" : "unclear";
}

const sq = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;

/** Commands that show something as written (no placeholders). */
export function commandsFor(source: string): string[] {
  const [kind, name] = source.split("/", 2);
  if (kind === "container" && name) {
    const n = sq(name);
    return [
      `docker inspect -f '{{.State.Status}} health={{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}} restarts={{.RestartCount}} started={{.State.StartedAt}}' ${n}`,
      `docker logs --since 30m ${n} 2>&1 | tail -n 100`,
    ];
  }
  if (kind === "journal" && name) return [`journalctl -t ${sq(name)} --since -30min --no-pager | tail -n 100`];
  return [];
}

/**
 * The check DRIFT answers from, and the question. `events` are the attached
 * lines grouped and judged by the log rules (`log_classify`).
 */
export function logTriage(o: { title: string; source: string; events: LogEventView[]; container: ContainerFacts | null; now: number }): { check: Check; question: string; verdict: Verdict } {
  const items: CheckItem[] = [];
  const verdict = verdictOf(o.events, o.container);
  if (o.container) items.push({ text: `Container state now: ${stateLine(o.container, o.now)}` });
  o.events.slice(0, 8).forEach((e, i) => {
    const head = e.head.length > 140 ? `${e.head.slice(0, 139)}…` : e.head;
    const more = e.lines > 1 ? ` (${e.lines} lines, one event)` : "";
    items.push({ text: `Event ${i + 1}${more}: ${head} → ${e.harmless ? `likely harmless: ${e.harmless}` : `${e.level.toUpperCase()}: counts as a real error`}` });
  });
  if (o.events.length > 8) items.push({ text: `…and ${o.events.length - 8} more events` });
  const fine = healthy(o.container);
  if (verdict === "harmless") {
    items.push({ level: "info", text: `Verdict: likely harmless, no action needed${fine ? `. The container is ${o.container?.health === "healthy" ? "running and healthy" : "running"}` : ""}.` });
  } else if (verdict === "problem") {
    items.push({ level: "caution", text: "Verdict: a real problem (a fatal error, or the container isn't running cleanly)." });
  } else {
    items.push({ text: `Verdict: unclear from these lines alone${fine ? "; the container itself is running" : ""}.` });
  }
  const cmds = commandsFor(o.source);
  if (cmds.length) items.push({ text: `Commands that show more, exactly as written: ${cmds.map((c) => `\`${c}\``).join(" ; ")}` });
  items.push({ text: "Unknown: anything not in these lines or this state (file paths, environment variables, settings, what changed)." });
  const question =
    verdict === "harmless"
      ? "Using only the attached lines and the CHECKS: is this a real problem? The verdict says likely harmless, so say that plainly in one or two sentences and why. Don't suggest fixes."
      : "Using only the attached lines and the CHECKS: what do these lines show, is it a real problem, and what should I check? Say \"unknown\" for anything they don't show, and only suggest the commands listed.";
  return { check: { kind: "triage", title: `Triage: ${o.title}`, items }, question, verdict };
}
