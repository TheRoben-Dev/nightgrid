/**
 * Builds what DRIFT sees (system prompt, retrieved guide sections, the live
 * context the user attached) and parses its replies for "show me" actions.
 * Everything here is pure so it can be tested and shown to the user verbatim.
 */
import { RAIL, SECTIONS, railPath, type SectionId } from "../sections";
import { FEATURES } from "../features";
import { GUIDE_TARGETS, guideTarget, VIEW_IDS } from "./guide";
import { clip, terms, type Chunk } from "./knowledge";
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

/** What the question is about, so the prompt lists only the controls that matter. */
export interface Focus {
  question: string;
  view?: SectionId;
  /** Titles of the retrieved guide sections ("Docker", "Fleet › Updates"). */
  titles?: string[];
}

/** Most "point" targets listed for one question (beyond the sidebar rows). */
export const MAX_TARGETS = 24;
const TARGETS_CHARS = 3600;

/**
 * The view-specific "point" targets worth listing for a question: the ones
 * whose words match it, then the ones in the views it's about (the current
 * view, views it names and views the retrieved sections cover). Listing all
 * 200+ cost about 6k tokens, more than the small models' whole window.
 */
export function focusTargets(all: { id: string; view: SectionId | null; what: string }[], focus?: Focus): typeof all {
  if (!focus) return [];
  const q = new Set(terms(focus.question));
  const text = `${focus.question} ${(focus.titles ?? []).join(" ")}`.toLowerCase();
  const views = new Set<string>(focus.view ? [focus.view] : []);
  for (const s of SECTIONS) if (new RegExp(`\\b${s.label.toLowerCase().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`).test(text) || new RegExp(`\\b${s.id}\\b`).test(text)) views.add(s.id);
  const scored = all
    .map((t, i) => {
      const words = new Set(terms(`${t.id.replace(/-/g, " ")} ${t.what}`));
      let s = 0;
      for (const w of q) if (words.has(w)) s += 2;
      if (t.view && views.has(t.view)) s += 1;
      return { t, s, i };
    })
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || a.i - b.i);
  const out: typeof all = [];
  let chars = 0;
  for (const { t } of scored) {
    const n = t.id.length + t.what.length + 3;
    if (out.length >= MAX_TARGETS || chars + n > TARGETS_CHARS) break;
    out.push(t);
    chars += n;
  }
  return out;
}

export function systemPrompt(focus?: Focus): string {
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
  // Sidebar rows, view tabs and controls visible everywhere are always
  // listed; a view's own controls only when the question is about them.
  const usable = GUIDE_TARGETS.filter(targetOn);
  const always = usable.filter((t) => t.view === null || own(t.id));
  const picked = new Set(focusTargets(usable.filter((t) => !always.includes(t)), focus).map((t) => t.id));
  const targets = usable
    .filter((t) => always.includes(t) || picked.has(t.id))
    .map((t) => `${t.id}: ${t.what}`)
    .join("\n");
  const off = FEATURES.filter((f) => !on(f.id)).map((f) => f.label);
  const tours = TOURS.map((t) => `${t.id} (${t.what})`).join("; ");
  return `You are DRIFT, the built-in helper of Nightgrid, a desktop app that manages servers over SSH (terminals, files, Docker, monitoring, services, tunnels, system tasks, automation). You run locally on the user's computer.

How to answer:
- Be brief and practical: a few sentences or a short numbered list. Use markdown code blocks for commands and files.
- When CHECKS are given, they are exact results computed by Nightgrid (command risks, compose findings, error meanings, cron schedules). Base your answer on them, explain them in plain words, and never contradict or drop a DANGER item. Don't invent findings they don't list.
- When MY HOSTS facts are given, the question is about those saved hosts: answer from the facts (status, last seen, alerts, shell history) first, and say what they show and what they don't. Don't guess at events the facts don't contain.
- For questions about Nightgrid, answer only from the GUIDE sections below. If they don't cover it, say you're not sure rather than guessing.
- Stay grounded. Use only facts from the attached lines, CHECKS and MY HOSTS. Never invent file paths, environment variables, settings, config keys or causes. If something isn't shown, say "unknown". Never write commands with placeholders like /path/to/file. Only suggest a command if it works exactly as written (prefer the ones CHECKS list).
- When a triage CHECK gives a container state or a verdict, follow it. If the container is running (and healthy), don't call it failed or crashed. If the verdict is "likely harmless", say so plainly and don't invent fixes. WARN/INFO lines and one-off 404 / NOT_FOUND responses are usually harmless.
- For server problems (logs, errors, Docker, compose files, systemd), explain the likely cause from what is shown and suggest a fix only when the lines support it. Mark commands that change or delete things as risky. You can't run commands; the user runs them, and Nightgrid asks before anything destructive.
- Never ask for passwords, keys or tokens. Values shown as •••• were hidden on purpose.
- Speak in the first person, never about "DRIFT" in the third person.

The sidebar lists the user's hosts, then these rows: ${rows}. Related views share a row and switch with tabs beside the view title; a row opens the view used last in it. Settings is at the bottom of the sidebar; its Features tab turns views and checks on or off. Point at a row to see its shortcut. When you say where something is, use these names (for example "System → Fleet").

Showing the user where things are: when it helps, end your reply with action lines, each on its own line, exactly like:
[[show VIEW]]
[[point TARGET: short note]]
"show" switches to a view: ${views}.
"point" highlights a control (it never clicks it). Only use these TARGET ids (when none fits, use show alone):
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

// ── fitting the model's window ─────────────────────────────────────────

/** Tokens kept free for the reply (the backend's max_tokens). */
export const REPLY_TOKENS = 768;
/** Context window assumed until the backend reports the loaded model's. */
export const DEFAULT_CONTEXT = 8192;
/** Longest single guide, library or note section in the prompt. */
export const MAX_DOC = 1800;
/** Longest CHECKS block and longest MY HOSTS block per host (lookups can reuse it). */
export const MAX_LIVE = 3000;

/**
 * A safe upper estimate of the tokens `text` takes. Qwen's tokenizer averages
 * about 4 characters per token on English and 3 on code and markdown; 3 keeps
 * an estimate on the high side, and the backend trims anything still over.
 */
export const estimateTokens = (text: string) => Math.ceil(text.length / 3);

/** Chat-template tokens around each turn. */
const TURN_OVERHEAD = 8;

const group = (d: Chunk) => (d.noteId !== undefined ? 2 : (d as { articleId?: string }).articleId ? 1 : 0);

/** Docs in the order to keep them: the best of each kind first, round robin. */
export function byPriority(docs: Chunk[]): Chunk[] {
  const groups: Chunk[][] = [[], [], []];
  for (const d of docs) groups[group(d)].push(d);
  const out: Chunk[] = [];
  for (let i = 0; out.length < docs.length; i++) for (const g of groups) if (g[i]) out.push(g[i]);
  return out;
}

export interface Fitted {
  turns: Turn[];
  /** The sections that made it into the prompt, in their original order. */
  docs: Chunk[];
  /** Estimated prompt tokens. */
  tokens: number;
  /** What was left out to fit, for the "What I was given" list. */
  trimmed: string[];
}

/**
 * Builds the conversation and fits it into `contextTokens` (minus room for
 * the reply): long sections are cut, then the lowest-ranked sections, older
 * turns, the rest of the sections, the selection and the live facts are
 * dropped in that order. The first question always fits.
 */
export function fitTurns(history: Turn[], question: string, ctx: HelperContext, docs: Chunk[], contextTokens = DEFAULT_CONTEXT): Fitted {
  const budget = Math.max(1024, contextTokens - REPLY_TOKENS - 64);
  const trimmed: string[] = [];
  const sys = systemPrompt({ question, view: ctx.view, titles: docs.filter((d) => d.noteId === undefined).map((d) => d.title) });
  let c: HelperContext = {
    ...ctx,
    checks: ctx.checks ? clip(ctx.checks, MAX_LIVE, "\n[cut to fit]") : ctx.checks,
    hosts: ctx.hosts?.map((h) => clip(h, MAX_LIVE, "\n[cut to fit]")),
    devices: ctx.devices?.slice(0, 40),
  };
  const clipped = docs.map((d) => (d.text.length > MAX_DOC ? { ...d, text: clip(d.text, MAX_DOC, "\n[…]") } : d));
  const ranked = byPriority(clipped);
  let keep = ranked.length;
  let past = history.filter((t) => t.role !== "system").slice(-8);
  // A past turn starts with its question's role, so drop them in pairs.
  while (past.length && past[0].role !== "user") past = past.slice(1);
  const build = () => {
    const kept = new Set(ranked.slice(0, keep));
    const used = clipped.filter((d) => kept.has(d));
    const turns: Turn[] = [{ role: "system", content: sys }, ...past, { role: "user", content: `${contextBlock(c, used)}\n\nQuestion: ${redact(question.trim())}` }];
    const tokens = turns.reduce((a, t) => a + estimateTokens(t.content) + TURN_OVERHEAD, 0);
    return { turns, used, tokens };
  };
  let r = build();
  const step = (): boolean => {
    if (keep > 2) {
      trimmed.push(ranked[keep - 1].title);
      keep--;
    } else if (past.length) {
      past = past.slice(2);
      trimmed.push("earlier messages");
    } else if (keep > 0) {
      trimmed.push(ranked[keep - 1].title);
      keep--;
    } else if (c.selection && c.selection.length > 400) {
      c = { ...c, selection: c.selection.slice(-Math.floor(c.selection.length / 2)) };
      if (!trimmed.includes("part of the attached text")) trimmed.push("part of the attached text");
    } else if (c.hosts?.some((h) => h.length > 400) || (c.checks?.length ?? 0) > 400) {
      c = {
        ...c,
        hosts: c.hosts?.map((h) => clip(h, Math.max(400, Math.floor(h.length / 2)), "\n[cut to fit]")),
        checks: c.checks ? clip(c.checks, Math.max(400, Math.floor(c.checks.length / 2)), "\n[cut to fit]") : c.checks,
      };
      if (!trimmed.includes("part of the live facts")) trimmed.push("part of the live facts");
    } else return false;
    return true;
  };
  while (r.tokens > budget && step()) r = build();
  return { turns: r.turns, docs: r.used, tokens: r.tokens, trimmed };
}

/**
 * The full conversation sent to the model. Earlier turns are kept as plain
 * text; only the newest question carries the context and guide sections,
 * which keeps the prompt small.
 */
export function buildTurns(history: Turn[], question: string, ctx: HelperContext, docs: Chunk[], contextTokens = DEFAULT_CONTEXT): Turn[] {
  return fitTurns(history, question, ctx, docs, contextTokens).turns;
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
