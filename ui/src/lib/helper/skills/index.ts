/**
 * DRIFT skills (M10p): picks the exact checks that fit a question and its
 * attached text, runs them, and formats the results for the model and for
 * the card shown above the reply.
 */
import { checkCommand, looksLikeCommand, type Level } from "./command";
import { looksLikeCompose, reviewCompose } from "./compose";
import { cronFromWords, describeCron, findCron, nextRuns, toOnCalendar } from "./cron";
import { lookupErrors } from "./errors";
import { explainRegex, findRegex, testRegex, toolNotes } from "./regex";
import { summaryFacts, type SummaryInput } from "./summary";
import { TRIAGE } from "./triage";
import type { AlertKind } from "../../api";

export interface CheckItem {
  level?: Level;
  text: string;
}

export interface Check {
  kind: "command" | "compose" | "errors" | "cron" | "regex" | "triage" | "summary";
  title: string;
  items: CheckItem[];
}

/** Commands written in backticks in the question, or a one-line selection that is a command. */
export function commandsIn(question: string, selection: string | null | undefined): string[] {
  const out: string[] = [];
  for (const m of question.matchAll(/`([^`\n]+)`/g)) if (looksLikeCommand(m[1])) out.push(m[1].trim());
  if (!out.length) {
    const m = /^(?:explain|what does|what do|is it safe to run|is this safe|should i run|check)\s*:?\s+(.+?)\s*(?:do|does)?\??$/i.exec(question.trim());
    if (m && looksLikeCommand(m[1])) out.push(m[1].trim());
  }
  const sel = selection?.trim();
  if (sel && !sel.includes("\n") && looksLikeCommand(sel)) out.push(sel.replace(/^[$#]\s+/, ""));
  return [...new Set(out)].slice(0, 3);
}

const fmtTime = (d: Date) => `${d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;

/** Runs every check that applies. `now` is passed in so tests are stable. */
export function runSkills(question: string, selection: string | null | undefined, now = new Date()): Check[] {
  const checks: Check[] = [];
  const sel = selection ?? "";
  const both = `${question}\n${sel}`;

  for (const cmd of commandsIn(question, sel)) {
    const fs = checkCommand(cmd);
    checks.push({
      kind: "command",
      title: `Command check: ${cmd.length > 80 ? `${cmd.slice(0, 77)}…` : cmd}`,
      items: fs.length ? fs.map((f) => ({ level: f.level, text: `${f.what}: ${f.why}` })) : [{ text: "No risky patterns found (Nightgrid checks deletes, overwrites, permissions, firewalls, shutdowns, pipes into a shell and --force)." }],
    });
  }

  const composeText = looksLikeCompose(sel) ? sel : looksLikeCompose(question) ? question : "";
  if (composeText) {
    const r = reviewCompose(composeText);
    checks.push({
      kind: "compose",
      title: `Compose review: ${r.services.length} service${r.services.length === 1 ? "" : "s"}`,
      items: r.error ? [{ text: r.error }] : r.findings.length ? r.findings.map((f) => ({ level: f.level, text: `${f.service ? `${f.service}: ` : ""}${f.what}: ${f.why}` })) : [{ text: "Nothing flagged: restart policies, pinned tags, published ports, secrets and privileges look fine." }],
    });
  }

  const errs = lookupErrors(both);
  if (errs.length) checks.push({ kind: "errors", title: "Error reference", items: errs.map((e) => ({ text: `${e.title}: ${e.meaning} Next: ${e.next}` })) });

  const expr = findCron(both);
  if (expr) {
    const words = describeCron(expr);
    const runs = nextRuns(expr, now, 3);
    const cal = toOnCalendar(expr);
    if (words) {
      const items: CheckItem[] = [{ text: `${expr} = ${words}` }];
      if (runs.length) items.push({ text: `Next runs (this computer's time zone; cron uses the host's): ${runs.map(fmtTime).join(", ")}.` });
      if (cal) items.push({ text: `systemd timer: OnCalendar=${cal}` });
      checks.push({ kind: "cron", title: "Cron schedule", items });
    }
  } else if (/\b(cron|crontab|schedule|timer|every|daily|weekly|monthly|hourly)\b/i.test(question)) {
    const d = cronFromWords(question);
    if (d && /\b(cron|crontab|schedule|timer|expression)\b/i.test(question)) {
      const words = describeCron(d.expr);
      const cal = toOnCalendar(d.expr);
      const items: CheckItem[] = [{ text: `${d.expr}${words ? ` = ${words}` : ""}` }];
      if (d.note) items.push({ level: "info", text: d.note });
      if (cal) items.push({ text: `systemd timer: OnCalendar=${cal}` });
      checks.push({ kind: "cron", title: "Cron from your words", items });
    }
  }
  const rx = findRegex(question) ?? (sel && !sel.includes("\n") ? findRegex(sel) : null);
  if (rx) {
    const items: CheckItem[] = explainRegex(rx.pattern).map((t) => ({ text: t }));
    const sample = sel && findRegex(sel) === null ? sel : "";
    if (sample) {
      const t = testRegex(rx.pattern, rx.flags, sample);
      items.push(t.error ? { level: "caution", text: `Doesn't compile in JavaScript: ${t.error}` } : { text: `On the attached text: ${t.matched} of ${t.lines} lines match${t.samples.length ? `, e.g. ${t.samples.join("; ")}` : ""}.` });
    } else {
      const t = testRegex(rx.pattern, rx.flags, "");
      if (t.error) items.push({ level: "caution", text: `Doesn't compile in JavaScript: ${t.error}` });
    }
    for (const n of toolNotes(rx.pattern, question)) items.push({ level: "info", text: n });
    checks.push({ kind: "regex", title: `Regex: /${rx.pattern}/${rx.flags}`, items });
  }
  return checks;
}

/** Usual causes and first checks for an alert of `kind`. */
export function triageCheck(kind: AlertKind, title: string): Check {
  const t = TRIAGE[kind];
  return {
    kind: "triage",
    title: `Triage: ${title}`,
    items: [...t.causes.map((c) => ({ text: `Usual cause: ${c}` })), ...t.checks.map((c) => ({ text: `Check: ${c}` })), { text: `In Nightgrid: ${t.where}` }],
  };
}

/** The last 24 hours as Nightgrid knows them. */
export function summaryCheck(i: SummaryInput): Check {
  return { kind: "summary", title: "Today, from Nightgrid's own data", items: summaryFacts(i).map((t) => ({ text: t })) };
}

const TAG: Record<Level, string> = { danger: "DANGER", caution: "CAUTION", info: "NOTE" };

/** The checks as text for the prompt. */
export function checksText(checks: Check[]): string {
  return checks.map((c) => `## ${c.title}\n${c.items.map((i) => `- ${i.level ? `[${TAG[i.level]}] ` : ""}${i.text}`).join("\n")}`).join("\n\n");
}
