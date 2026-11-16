/**
 * Builds what DRIFT sees (system prompt, retrieved guide sections, the live
 * context the user attached) and parses its replies for "show me" actions.
 * Everything here is pure so it can be tested and shown to the user verbatim.
 */
import { RAIL, SECTIONS, railPath, type SectionId } from "../sections";
import { FEATURES } from "../features";
import { GUIDE_TARGETS, guideTarget, VIEW_IDS } from "./guide";
import type { Chunk } from "./knowledge";
import { TOURS, tourById } from "./tours";

export interface Turn {
  role: "system" | "user" | "assistant";
  content: string;
}

/** Live context the user can see (and remove) before asking. */
export interface HelperContext {
  view: SectionId;
  host?: string | null;
  /** Text the user selected (log lines, an error, a compose file). */
  selection?: string | null;
  /** What the attached text is, when a view attached it ("Docker logs: web"). */
  source?: string | null;
  /** Terminal view: the known program in front of the active pane, with its keys. */
  program?: { name: string; keys: string } | null;
  /** The user's labels for network devices that matter to the question (one line each). */
  devices?: string[] | null;
  /** Live facts about the saved hosts the question names (one block each). */
  hosts?: string[] | null;
  /** Exact results of DRIFT's skills (command check, compose review, error reference, cron). */
  checks?: string | null;
}

export type HelperAction = { kind: "show"; view: SectionId } | { kind: "point"; id: string; note: string } | { kind: "tour"; id: string };

/** Longest selection passed to the model (it's small; keep room to answer). */
export const MAX_SELECTION = 6000;

/**
 * Masks values that look like secrets before anything reaches the model:
 * `KEY=value` / `key: value` for secret-ish names, bearer tokens, URLs with
 * passwords, and private key blocks. The model is local, but the prompt is
 * shown on screen and may be copied, so it stays clean.
 */
export function redact(text: string): string {
  return text
    .replace(/-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g, "[private key removed]")
    .replace(/\b(bearer|token|basic)\s+[A-Za-z0-9._~+/=-]{12,}/gi, "$1 ••••")
    .replace(/(\b[a-z][a-z0-9+.-]*:\/\/[^:/\s@]+:)[^@\s]+@/gi, "$1••••@")
    .replace(
      /(\b[A-Za-z0-9_.-]*(?:pass(?:word|wd)?|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|auth|credential|pwd)[A-Za-z0-9_.-]*\s*[:=]\s*)("[^"\n]*"|'[^'\n]*'|[^\s,;]+)/gi,
      "$1••••",
    );
}

const viewLabel = (id: SectionId) => SECTIONS.find((s) => s.id === id)?.label ?? id;

// Settings → Features check, set by the helper store (importing the settings
// store here would loop back through the mock backend).
let on: (id: string) => boolean = () => true;
export function setFeatureCheck(f: (id: string) => boolean) {
  on = f;
}

export function systemPrompt(): string {
  // Views turned off in Settings → Features aren't offered.
  const views = SECTIONS.filter((s) => on(s.id))
    .map((s) => `${s.id} (${railPath(s.id)})`)
    .join(", ");
  const own = (id: string) => /^(rail|viewtab)-/.exec(id) && id.slice(id.indexOf("-") + 1);
  const targetOn = (t: { id: string; view: SectionId | null }) => {
    const v = own(t.id);
    return (t.view === null || on(t.view)) && (!v || on(v));
  };
  // The sidebar as the user sees it: rows, and the views a row holds as tabs.
  const rows = RAIL.map((r) => ({ r, m: r.members.filter((m) => on(m)) }))
    .filter((x) => x.m.length)
    .map(({ r, m }) => (m.length > 1 ? `${r.label} (tabs: ${m.map((v) => r.tabs?.[v] ?? SECTIONS.find((s) => s.id === v)?.label).join(", ")})` : SECTIONS.find((s) => s.id === m[0])?.label))
    .join(", ");
  const targets = GUIDE_TARGETS.filter(targetOn).map((t) => `${t.id}: ${t.what}`).join("\n");
  const off = FEATURES.filter((f) => !on(f.id)).map((f) => f.label);
  const tours = TOURS.map((t) => `${t.id} (${t.what})`).join("; ");
  return `You are DRIFT, the built-in helper of Nightgrid, a desktop app that manages servers over SSH (terminals, files, Docker, monitoring, services, tunnels, system tasks, automation). You run locally on the user's computer.

How to answer:
- Be brief and practical: a few sentences or a short numbered list. Use markdown code blocks for commands and files.
- When CHECKS are given, they are exact results computed by Nightgrid (command risks, compose findings, error meanings, cron schedules). Base your answer on them, explain them in plain words, and never contradict or drop a DANGER item. Don't invent findings they don't list.
- When MY HOSTS facts are given, the question is about those saved hosts: answer from the facts (status, last seen, alerts, shell history) first, and say what they show and what they don't. Don't guess at events the facts don't contain.
- For questions about Nightgrid, answer only from the GUIDE sections below. If they don't cover it, say you're not sure rather than guessing.
- For server problems (logs, errors, Docker, compose files, systemd), explain the likely cause and suggest a fix. Mark commands that change or delete things as risky. You can't run commands; the user runs them, and Nightgrid asks before anything destructive.
- Never ask for passwords, keys or tokens. Values shown as •••• were hidden on purpose.
- Speak in the first person, never about "DRIFT" in the third person.

The sidebar lists the user's hosts, then these rows: ${rows}. Related views share a row and switch with tabs beside the view title; a row opens the view used last in it. Settings is at the bottom of the sidebar; its Features tab turns views and checks on or off. Point at a row to see its shortcut. When you say where something is, use these names (for example "System → Fleet").

Showing the user where things are: when it helps, end your reply with action lines, each on its own line, exactly like:
[[show VIEW]]
[[point TARGET: short note]]
"show" switches to a view: ${views}.
"point" highlights a control (it never clicks it). Only use these TARGET ids:
${targets}
Use at most one show and three points, in the order the user should follow.
For a task with several steps across views, offer a guided tour instead (it waits for the user at each step):
[[tour TOUR]]
Tours: ${tours}
Offer at most one tour, and only when it fits the question.${
    off.length ? `\nTurned off in Settings → Features (the user can turn them on there; don't show or point at them): ${off.join(", ")}.` : ""
  }`;
}

export function contextBlock(ctx: HelperContext, docs: Chunk[]): string {
  const parts: string[] = [];
  parts.push(`Current view: ${viewLabel(ctx.view)}${ctx.host ? `, host "${ctx.host}"` : ""}.`);
  if (ctx.program) parts.push(`Running in the terminal: ${ctx.program.name}. Its keys: ${ctx.program.keys}.`);
  if (ctx.checks?.trim()) parts.push(`CHECKS (exact, computed by Nightgrid):\n${redact(ctx.checks.trim())}`);
  if (ctx.hosts?.length) parts.push("MY HOSTS (live facts from Nightgrid, about the hosts this question names):\n" + ctx.hosts.map(redact).join("\n\n"));
  if (ctx.devices?.length) parts.push("NETWORK DEVICES (the user's own labels):\n" + ctx.devices.map((d) => `- ${d}`).join("\n"));
  const guide = docs.filter((d) => d.noteId === undefined);
  const mine = docs.filter((d) => d.noteId !== undefined);
  if (guide.length) {
    parts.push("GUIDE:\n" + guide.map((d) => `## ${d.title}\n${d.text}`).join("\n\n"));
  }
  if (mine.length) {
    parts.push("USER'S NOTES (written by the user; say which note you used):\n" + mine.map((d) => `## ${d.title}\n${d.text}`).join("\n\n"));
  }
  if (ctx.selection?.trim()) {
    const sel = redact(ctx.selection.trim());
    const cut = sel.length > MAX_SELECTION ? `${sel.slice(-MAX_SELECTION)}\n[earlier lines cut]` : sel;
    parts.push(`${ctx.source ? `Attached (${ctx.source})` : "Selected text"}:\n\`\`\`\n${cut}\n\`\`\``);
  }
  return parts.join("\n\n");
}

/**
 * The full conversation sent to the model. Earlier turns are kept as plain
 * text; only the newest question carries the context and guide sections,
 * which keeps the prompt small.
 */
export function buildTurns(history: Turn[], question: string, ctx: HelperContext, docs: Chunk[]): Turn[] {
  const past = history.filter((t) => t.role !== "system").slice(-8);
  return [
    { role: "system", content: systemPrompt() },
    ...past,
    { role: "user", content: `${contextBlock(ctx, docs)}\n\nQuestion: ${redact(question.trim())}` },
  ];
}

const ACTION = /^\s*\[\[\s*(show|point|tour)\s+([a-z0-9-]+)\s*(?::\s*(.*?))?\s*\]\]\s*$/i;

/** Splits a reply into display text and validated actions. */
export function parseReply(reply: string): { text: string; actions: HelperAction[] } {
  const actions: HelperAction[] = [];
  const keep: string[] = [];
  for (const line of reply.split("\n")) {
    const m = ACTION.exec(line);
    if (!m) {
      keep.push(line);
      continue;
    }
    const [, kind, id, note] = m;
    if (kind.toLowerCase() === "tour") {
      if (tourById(id) && !actions.some((a) => a.kind === "tour")) actions.push({ kind: "tour", id });
    } else if (kind.toLowerCase() === "show") {
      if ((VIEW_IDS as string[]).includes(id) && !actions.some((a) => a.kind === "show")) actions.push({ kind: "show", view: id as SectionId });
    } else if (guideTarget(id) && actions.filter((a) => a.kind === "point").length < 3) {
      actions.push({ kind: "point", id, note: (note ?? "").slice(0, 160) });
    }
  }
  return { text: keep.join("\n").replace(/\n{3,}/g, "\n\n").trim(), actions };
}

/** Hides a half-streamed action line (`[[poi…`) while the reply arrives. */
export function streamingText(partial: string): string {
  return partial
    .split("\n")
    .filter((l) => !/^\s*\[\[/.test(l))
    .join("\n")
    .trimEnd();
}
