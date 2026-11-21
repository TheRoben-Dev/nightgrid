/**
 * DRIFT helper state: the side panel, the conversation, the attached view
 * context, the model download and the "show me" spotlight.
 */
import { healthText } from "./healthIgnores.svelte";
import { sound } from "./sound.svelte";
import { api, type Host, type HelperModels, type HelperStatus } from "../api";
import type { Lookup_Serialize as Lookup, LookupResult_Serialize as LookupResult } from "../bindings";
import { guideTarget } from "../helper/guide";
import { guideIndex, noteRefs, notesIndex } from "../helper/knowledge";
import { tour } from "./tour.svelte";
import { fitTurns, setFeatureCheck, parseReply, streamingText, type HelperAction, type HelperContext, type Turn } from "../helper/prompt";
import { railTarget, sectionById } from "../sections";
import { settingsTabFor } from "../settingsTabs";
import { labelLines } from "../netmap/graph";
import { keymap } from "../term/keyhints";
import { historyTerms, hostFacts, mentionedHosts } from "../helper/hostFacts";
import { followUpHint, lookupBlock, lookupLabel, notYetRun, planLookups, takeCheckRequests, wantsLookups } from "../helper/lookups";
import { asksAboutChanges } from "../journal";
import { quests } from "./quests.svelte";
import { checksText, runSkills, summaryCheck, type Check } from "../helper/skills";
import { wantsSummary, type SummaryInput } from "../helper/skills/summary";
import { diskRows, disksText, totals, updateRows, updatesText } from "../fleet";
import { backups, jobState } from "./backups.svelte";
import { fleet } from "./fleet.svelte";
import { holds } from "./holds.svelte";
import { healthOf, services } from "./services.svelte";
import { alerts } from "./alerts.svelte";
import { docker } from "./docker.svelte";
import { hosts } from "./hosts.svelte";
import { metrics } from "./metrics.svelte";
import { netmap } from "./netmap.svelte";
import { notes } from "./notes.svelte";
import { settings } from "./settings.svelte";
import { system } from "./system.svelte";
import { terms } from "./terms.svelte";
import { ui } from "./ui.svelte";
import { kbContent } from "./kbcontent.svelte";

setFeatureCheck((id) => settings.on(id));

export type DriftState = "idle" | "blink" | "think" | "talk" | "point" | "listen" | "success" | "error" | "confused" | "sleep";

export interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  /** What the user typed, or the reply without action lines. */
  text: string;
  actions: HelperAction[];
  streaming?: boolean;
  error?: string;
  /** Exactly what the model was given for this reply. */
  sent?: Turn[];
  sources?: string[];
  /** What was left out of the prompt to fit the model's window. */
  trimmed?: string[];
  /** Notes the answer was given, to open from the reply. */
  noteRefs?: { id: number; title: string }[];
  /** Library articles (basics and guides) the answer drew on. */
  articleRefs?: { id: string; title: string }[];
  stats?: string;
  /** What was attached to a question ("Docker logs: web · 150 lines"). */
  attached?: string;
  /** Exact skill results shown above the reply (and given to the model). */
  checks?: Check[];
  /** Read-only checks run on hosts for this reply ("What I checked", D-151). */
  lookups?: HostLookups[];
}

export interface HostLookups {
  host: string;
  results: LookupResult[];
  error?: string;
}

export interface Spotlight {
  id: string;
  note: string;
}

const errMsg = (e: unknown) => (e instanceof Error ? e.message : typeof e === "object" && e && "message" in e ? String(e.message) : String(e));

/** Text selected in the page or the focused terminal, if any. */
export function currentSelection(): string {
  const dom = typeof window !== "undefined" ? (window.getSelection()?.toString() ?? "") : "";
  if (dom.trim()) return dom;
  try {
    return terms.currentSession?.term.getSelection() ?? "";
  } catch {
    return "";
  }
}

/** The known program in front of the active terminal, with its keys (key hints). */
export function currentProgram(): HelperContext["program"] {
  if (ui.section !== "terminal") return null;
  const m = keymap(terms.currentSession?.hintId ?? null);
  return m ? { name: m.name, keys: m.hints.map((h) => `${h.keys}: ${h.does}`).join("; ") } : null;
}

/** The host the current view is about, if it has one. */
export function currentHost(): string | null {
  try {
    switch (ui.section) {
      case "terminal": {
        const o = terms.currentSession?.origin;
        return o && o.kind === "ssh" ? o.name : null;
      }
      case "docker":
        return docker.host?.name ?? null;
      case "system":
        return system.host?.name ?? null;
      default:
        return null;
    }
  } catch {
    return null;
  }
}

class HelperStore {
  open = $state(false);
  messages = $state<ChatMessage[]>([]);
  busy = $state(false);
  /** The model is loading for the first question. */
  loading = $state(false);
  status = $state<HelperStatus | null>(null);
  progress = $state<{ done: number; total: number } | null>(null);
  downloadError = $state<string | null>(null);
  /** The models on offer and how each fits this computer (Settings → DRIFT). */
  models = $state<HelperModels | null>(null);
  /** The model being downloaded, and whether to switch to it once ready. */
  downloadingId = $state<string | null>(null);
  private switchWhenReady = false;
  /** Context attached to the next question (the user can drop parts). */
  context = $state<HelperContext>({ view: "overview" });
  spotlight = $state<Spotlight | null>(null);
  /** A brief mood after something happened (success / confused). */
  mood = $state<DriftState | null>(null);
  typing = $state(false);
  /** Text put in the question box for the user to send or edit. */
  draft = $state("");
  /** What DRIFT is checking on a host right now ("server01: disk, storage"). */
  checking = $state<string | null>(null);
  /** Hosts the last question named (for read-only checks). */
  private named: Host[] = [];
  /** Checks a view attached to the next question (alert triage). */
  private pendingChecks: Check[] = [];
  private seq = 0;
  private spotTimer: ReturnType<typeof setTimeout> | undefined;

  get enabled(): boolean {
    return settings.value.helperEnabled;
  }

  get ready(): boolean {
    return this.status?.state === "ready";
  }

  get drift(): DriftState {
    if (!this.enabled) return "sleep";
    const last = this.messages[this.messages.length - 1];
    if (last?.error) return "error";
    if (this.busy) return last?.role === "assistant" && last.text ? "talk" : "think";
    if (this.spotlight) return "point";
    if (this.mood) return this.mood;
    if (this.typing) return "listen";
    return "idle";
  }

  async refresh() {
    try {
      this.status = await api.helperStatus();
    } catch {
      /* keep the last status */
    }
  }

  /** Re-reads what the current view offers as context. */
  captureContext() {
    const selection = currentSelection();
    this.context = { view: ui.section, host: currentHost(), selection: selection.trim() ? selection : null, source: null, program: currentProgram() };
  }

  /**
   * Opens DRIFT with text a view attached (logs, a journal, compose output,
   * a file) and asks `question` about it once the model is ready. If it
   * isn't, the question waits in the box.
   */
  async explain(o: { source: string; text: string; question: string; host?: string | null; checks?: Check[] }) {
    const text = o.text.trim();
    if (!text) return;
    this.pendingChecks = o.checks ?? [];
    this.context = { view: ui.section, host: o.host ?? currentHost(), selection: text, source: o.source, program: currentProgram() };
    this.open = true;
    await this.refresh();
    if (this.enabled && this.ready && !this.busy) await this.ask(o.question);
    else this.draft = o.question;
  }

  toggle(force?: boolean) {
    const next = force ?? !this.open;
    if (next && !this.open) {
      this.captureContext();
      void this.refresh();
    }
    this.open = next;
  }

  dropContext(part: "host" | "selection") {
    this.context = part === "selection" ? { ...this.context, selection: null, source: null } : { ...this.context, host: null };
  }

  clear() {
    if (this.busy) return;
    this.messages = [];
    this.clearSpotlight();
  }

  async loadModels() {
    try {
      this.models = await api.helperModels();
    } catch (e) {
      this.downloadError = errMsg(e);
    }
  }

  /**
   * Downloads a model (the one in use when no id is given). The model in use
   * keeps answering meanwhile; with `use`, DRIFT switches once it's ready.
   */
  async download(id?: string, use = false) {
    const target = id ?? this.status?.id;
    if (!target) return;
    const m = this.models?.models.find((x) => x.id === target);
    this.downloadError = null;
    this.downloadingId = target;
    this.switchWhenReady = use;
    this.progress = m ? { done: m.have ?? 0, total: m.bytes ?? 0 } : { done: this.status?.have ?? 0, total: this.status?.bytes ?? 0 };
    try {
      const done = await api.helperDownload(target, (p) => (this.progress = { done: p.done ?? 0, total: p.total ?? 0 }));
      if (done && this.switchWhenReady) await this.useModel(target);
    } catch (e) {
      this.downloadError = errMsg(e);
    } finally {
      this.progress = null;
      this.downloadingId = null;
      this.switchWhenReady = false;
      await this.refresh();
      if (this.models) await this.loadModels();
    }
  }

  /** Switches DRIFT to a downloaded model; it loads on the next question. */
  async useModel(id: string) {
    await settings.update({ helperModel: id });
    await api.helperUnload();
    await this.refresh();
    if (this.models) await this.loadModels();
  }

  cancelDownload() {
    void api.helperCancelDownload();
  }

  async deleteModel(id?: string) {
    const target = id ?? this.status?.id;
    if (!target) return;
    try {
      await api.helperDeleteModel(target);
    } catch (e) {
      this.downloadError = errMsg(e);
    }
    await this.refresh();
    if (this.models) await this.loadModels();
  }

  async setEnabled(on: boolean) {
    await settings.update({ helperEnabled: on });
    if (!on) {
      this.open = false;
      this.clearSpotlight();
      void api.helperUnload();
    }
    await this.refresh();
  }

  /** Frees the model's memory; it loads again on the next question. */
  async unload() {
    await api.helperUnload();
    await this.refresh();
  }

  /**
   * Facts about the saved hosts the question names. A follow-up that names
   * none ("check its history") uses the hosts the previous question named.
   */
  private async hostContext(q: string): Promise<string[]> {
    if (!hosts.loaded) await hosts.refresh();
    let found = mentionedHosts(hosts.hosts, q);
    if (!found.length) {
      const prev = this.messages.filter((m) => m.role === "user").slice(-2).reverse();
      for (const m of prev) {
        found = mentionedHosts(hosts.hosts, m.text);
        if (found.length) break;
      }
    }
    this.named = found;
    const now = Math.floor(Date.now() / 1000);
    if (!found.length) {
      // "What changed before it broke?" without a host: the whole journal.
      if (!asksAboutChanges(q) || !settings.on("fleet.journal")) return [];
      const changes = await api.journalForDrift([], now - 14 * 86_400, 25).catch(() => "");
      return [`### Changes made from Nightgrid across all hosts, last 14 days (journal, newest first)\n${changes || "None recorded."}`];
    }
    const all = [...(alerts.snap?.active ?? []), ...(alerts.snap?.history ?? [])];
    return Promise.all(
      found.map(async (h) => {
        let history = null;
        let historyMatches = null;
        let historyError = null;
        if (h.driftHistory) {
          try {
            const got = await api.helperHostHistory(h.id, historyTerms(q, found.map((x) => x.name)));
            history = got.recent;
            historyMatches = got.matches;
          } catch (e) {
            historyError = errMsg(e);
          }
        }
        const mine = all.filter((a, i) => a.hostId === h.id && all.findIndex((b) => b.id === a.id) === i);
        const changes = settings.on("fleet.journal") ? await api.journalForDrift([h.id], now - 14 * 86_400, 12).catch(() => null) : null;
        return hostFacts({ host: h, ping: hosts.pings[h.id], metrics: metrics.byHost[h.id], alerts: mine, history, historyMatches, historyError, known: this.knownAbout(h), changes, now });
      }),
    );
  }

  /**
   * Hosts DRIFT may run read-only checks on for this question: the ones it
   * names, else the host the current view is about (each only when allowed).
   */
  private lookupHosts(viewHost: string | null | undefined): Host[] {
    const named = this.named.filter((h) => h.driftLookups);
    if (named.length || this.named.length) return named.slice(0, 2);
    const v = viewHost ? hosts.hosts.find((h) => h.name === viewHost) : undefined;
    return v?.driftLookups ? [v] : [];
  }

  /** Runs read-only checks on each host (D-151). Never throws. */
  private async runLookups(targets: Host[], plan: Lookup[], terms: string[]): Promise<HostLookups[]> {
    const out: HostLookups[] = [];
    for (const h of targets) {
      this.checking = `${h.name}: ${plan.map(lookupLabel).join(", ") || "looking around"}`;
      try {
        out.push({ host: h.name, results: await api.helperHostLookups(h.id, plan, terms) });
      } catch (e) {
        out.push({ host: h.name, results: [], error: errMsg(e) });
      }
    }
    this.checking = null;
    return out;
  }

  private static blocks(all: HostLookups[]): string[] {
    return all.filter((x) => x.results.length || x.error).map((x) => (x.error ? `### ${x.host}\nCouldn't run checks: ${x.error}` : lookupBlock(x.host, x.results)));
  }

  async ask(question: string) {
    const q = question.trim();
    if (!q || this.busy) return;
    const ctx = { ...this.context, view: this.context.view ?? ui.section };
    if (!notes.loaded) await notes.load();
    if (!netmap.labelsLoaded) await netmap.loadLabels();
    ctx.devices = labelLines(netmap.labels, netmap.result?.devices ?? [], q, ctx.view === "network");
    ctx.hosts = await this.hostContext(q);
    // Read-only checks on hosts that allow them (D-151).
    const lookTargets = wantsLookups(q) ? this.lookupHosts(ctx.host) : [];
    let looked: HostLookups[] = [];
    if (lookTargets.length) {
      this.busy = true;
      try {
        looked = await this.runLookups(lookTargets, planLookups(q), historyTerms(q, lookTargets.map((h) => h.name)));
      } finally {
        this.busy = false;
      }
      ctx.lookups = HelperStore.blocks(looked);
      ctx.lookupHint = ctx.lookups.length ? followUpHint() : null;
    }
    const checks = [...this.pendingChecks, ...runSkills(q, ctx.selection)];
    this.pendingChecks = [];
    if (wantsSummary(q)) checks.unshift(summaryCheck(this.summaryInput()));
    // The Library's error reference: entries whose message is in the question or the attached text.
    // Newer downloaded Library content first, so the index below is rebuilt from it (D-174).
    await kbContent.ensure().catch(() => undefined);
    const errRef = await import("../kb/errorref");
    errRef.addErrorRefs(checks, errRef.matchErrors(`${q}\n${ctx.selection ?? ""}`, 3));
    ctx.checks = checks.length ? checksText(checks) : null;
    // Questions about the user's own hosts (or with exact checks) lean on those, not the guide.
    const guideN = ctx.hosts.length || checks.length || looked.length ? 2 : 4;
    // The library (M11) beyond its "Using Nightgrid" shelf, which is the guide above.
    const lib = await import("../kb/library");
    const libDocs = (lib.libraryIndex().search(q, 6) as { title: string; text: string; articleId?: string }[])
      .filter((c) => c.articleId && !c.articleId.startsWith("nightgrid/"))
      .slice(0, ctx.hosts.length || checks.length ? 1 : 2);
    const found = [...guideIndex().search(`${q} ${sectionById(ctx.view).label}`, guideN), ...libDocs, ...notesIndex(notes.notes).search(q, 2)];
    const history: Turn[] = this.messages
      .filter((m) => !m.error && m.text)
      .map((m) => ({ role: m.role, content: m.text }));
    // Fit everything into the loaded model's window (8k until it reports one);
    // only the sections that made it in are listed as sources.
    const fit = fitTurns(history, q, ctx, found, this.status?.context || undefined);
    const sent = fit.turns;
    const docs = fit.docs;
    const usedLib = libDocs.filter((c) => docs.some((d) => d.title === c.title && (d as { articleId?: string }).articleId === c.articleId));
    const articleRefs = [...new Map(usedLib.map((c) => [c.articleId!, lib.article(c.articleId!)?.title ?? c.title])).entries()].map(([id, title]) => ({ id, title }));
    const sel = ctx.selection?.trim();
    const n = sel ? sel.split("\n").length : 0;
    const attached = sel ? `${ctx.source ?? "selection"} · ${n} line${n === 1 ? "" : "s"}` : undefined;
    this.messages.push({ id: ++this.seq, role: "user", text: q, actions: [], attached });
    this.messages.push({ id: ++this.seq, role: "assistant", text: "", actions: [], streaming: true, sent, sources: docs.map((d) => d.title), trimmed: fit.trimmed, noteRefs: noteRefs(docs), articleRefs, checks, lookups: looked.length ? looked : undefined });
    const msg = this.messages[this.messages.length - 1];
    // Selection is used once; the view context stays.
    this.context = { ...this.context, selection: null, source: null };
    this.busy = true;
    this.stopped = false;
    this.mood = null;
    this.clearSpotlight();
    let raw = "";
    const stream = (turns: Turn[]) =>
      api.helperAsk(turns, (e) => {
        if (e.kind === "loading") this.loading = true;
        else if (e.kind === "thinking") this.loading = false;
        else if (e.kind === "text") {
          raw += e.text;
          msg.text = streamingText(raw);
        }
      });
    try {
      let reply = await stream(sent);
      // The model asked for more checks: run them once and answer again (D-151).
      const more = takeCheckRequests(reply.text || raw);
      const fresh = lookTargets.length ? notYetRun(more.asked, looked[0]?.results ?? []) : [];
      if (fresh.length && !this.stopped) {
        const extra = await this.runLookups(lookTargets.slice(0, 1), fresh, []);
        looked = looked.map((x, i) => (i === 0 && extra[0] ? { ...x, results: [...x.results, ...extra[0].results], error: x.error ?? extra[0].error } : x));
        msg.lookups = looked;
        const again = { ...ctx, lookups: HelperStore.blocks(looked), lookupHint: null };
        const refit = fitTurns(history, q, again, docs, this.status?.context || undefined);
        const turns = refit.turns;
        msg.sent = turns;
        msg.trimmed = refit.trimmed;
        raw = "";
        msg.text = "";
        reply = await stream(turns);
      }
      const parsed = parseReply(takeCheckRequests(reply.text || raw).text);
      msg.text = parsed.text;
      msg.actions = parsed.actions;
      const secs = reply.seconds ?? 0;
      const tps = secs > 0 ? ` · ${(reply.tokens / secs).toFixed(1)} tok/s` : "";
      msg.stats = `${reply.promptTokens} in · ${reply.tokens} out${tps}${reply.finish === "length" ? " · cut at the length limit" : reply.finish === "stopped" ? " · stopped" : ""}`;
      if (!parsed.text && !parsed.actions.length) {
        msg.text = "I don't have an answer for that.";
        this.flash("confused");
      } else if (!this.stopped) sound.play("driftReply");
    } catch (e) {
      msg.error = errMsg(e);
      if (raw) msg.text = parseReply(takeCheckRequests(raw).text).text;
    } finally {
      msg.streaming = false;
      this.busy = false;
      this.loading = false;
      void this.refresh();
    }
  }

  private stopped = false;

  stop() {
    this.stopped = true;
    void api.helperStop();
  }

  /**
   * Opens a new note with a reply as a runbook draft: the question, what
   * was attached, the checks and the answer. The user edits and saves it.
   */
  saveAsNote(m: ChatMessage) {
    const i = this.messages.indexOf(m);
    const q = i > 0 ? this.messages[i - 1] : null;
    const host = this.context.host ? (hosts.hosts.find((h) => h.name === this.context.host) ?? null) : null;
    const parts: string[] = [];
    if (q) parts.push(`## Question\n\n${q.text}${q.attached ? `\n\n_Attached: ${q.attached}_` : ""}`);
    if (m.checks?.length) parts.push(`## Checks\n\n${checksText(m.checks).replace(/^## /gm, "### ")}`);
    parts.push(`## Answer\n\n${m.text}`);
    const title = (q?.text ?? "DRIFT answer").replace(/\s+/g, " ").trim();
    notes.draft = { id: null, input: { title: title.length > 70 ? `${title.slice(0, 67)}…` : title, body: parts.join("\n\n"), hostId: host?.id ?? null, tags: ["runbook"], pinned: false, hideFromDrift: false, runbook: false, alertKinds: [], serviceId: null } };
    ui.go("notes");
  }

  /** What Nightgrid's other views already know about a host (no new requests). */
  private knownAbout(h: Host): string[] {
    const out: string[] = [];
    const clip = (xs: string[], n: number) => (xs.length > n ? [...xs.slice(0, n), `…and ${xs.length - n} more`] : xs);
    const svc = services.services.filter((v) => v.hostId === h.id).map((v) => `${v.name}: ${healthOf(v, services.status[v.id])}`);
    if (svc.length) out.push(`Service checks (Services view): ${clip(svc, 12).join("; ")}`);
    if (fleet.updates[h.id]) {
      const t = updatesText(updateRows([h], fleet.updates, (id) => holds.host(id)), []).trim();
      if (t) out.push(`Updates (Fleet → UPDATES): ${t.split("\n").slice(0, 6).join(" / ")}`);
    }
    if (fleet.disks[h.id]) {
      const t = disksText(diskRows([h], fleet.disks, settings.value.alerts.forecastDays)).trim();
      if (t) out.push(`Disks (Fleet → DISKS):\n${t.split("\n").slice(0, 10).join("\n")}`);
    }
    if (backups.loaded) {
      const jobs = backups.jobs.filter((j) => j.hostId === h.id).map((j) => `${j.name}: ${jobState(backups.status[j.id])}`);
      if (jobs.length) out.push(`Backup jobs (Fleet → BACKUPS): ${clip(jobs, 8).join("; ")}`);
    }
    if (docker.hostId === h.id && docker.containers.length) {
      const cs = docker.containers.map((c) => `${c.name} ${c.state}${healthText(c) ? ` (${healthText(c)})` : ""}`);
      out.push(`Containers (Docker view, ${cs.length}): ${clip(cs, 20).join("; ")}`);
    }
    for (const l of quests.linesFor(h.id)) out.push(`Library guide checked on this host (read-only, Library → Verify): ${l}`);
    return out;
  }

  /** What the app knows about the last day, for the daily summary skill. */
  private summaryInput(): SummaryInput {
    const now = Math.floor(Date.now() / 1000);
    const name = (id: number | null) => (id == null ? null : (hosts.hosts.find((h) => h.id === id)?.name ?? null));
    const snap = alerts.snap;
    const offline = hosts.hosts.filter((h) => metrics.byHost[h.id]?.state === "offline").map((h) => h.name);
    const restarted = hosts.hosts.flatMap((h) => {
      const up = metrics.byHost[h.id]?.sample?.uptimeSecs;
      return up != null && up < 86400 ? [{ host: h.name, agoSecs: up }] : [];
    });
    const svc = services.services.map((v) => ({ name: v.name, h: healthOf(v, services.status[v.id]) }));
    const upd = updateRows(hosts.hosts, fleet.updates, (id) => holds.host(id));
    const t = totals(upd);
    const disks = diskRows(hosts.hosts, fleet.disks, settings.value.alerts.forecastDays).filter((r) => r.tone === "bad" || r.tone === "warn");
    return {
      now,
      active: (snap?.active ?? []).filter((a) => a.silenced == null).map((a) => ({ title: a.title, detail: a.detail, since: a.since, host: name(a.hostId), severity: a.severity })),
      resolved: (snap?.history ?? []).filter((a) => a.resolved != null).map((a) => ({ title: a.title, since: a.since, resolved: a.resolved ?? a.since, host: name(a.hostId) })),
      hostsTotal: hosts.hosts.length,
      offline,
      restarted,
      services: { total: svc.length, down: svc.filter((x) => x.h === "down").map((x) => x.name), warn: svc.filter((x) => x.h === "warn").map((x) => x.name) },
      updates: Object.keys(fleet.updates).length ? { packages: t.packages, hosts: t.hosts, security: t.security, reboot: upd.filter((r) => r.reboot).map((r) => r.host.name) } : null,
      disks: fleet.disksAt ? disks.map((r) => `${r.host.name}: ${r.tone === "bad" ? "needs attention" : "watch"}`) : null,
      backups: backups.loaded ? backups.jobs.filter((j) => ["failed", "error", "stale"].includes(jobState(backups.status[j.id]))).map((j) => j.name) : null,
    };
  }

  private flash(m: DriftState, ms = 1500) {
    this.mood = m;
    setTimeout(() => {
      if (this.mood === m) this.mood = null;
    }, ms);
  }

  /** Runs a reply's actions in order: switch view, then spotlight targets one after another. */
  async run(actions: HelperAction[]) {
    const t = actions.find((a) => a.kind === "tour");
    if (t && t.kind === "tour") {
      tour.start(t.id);
      return;
    }
    const show = actions.find((a) => a.kind === "show");
    if (show && show.kind === "show") ui.go(show.view);
    const points = actions.filter((a): a is Extract<HelperAction, { kind: "point" }> => a.kind === "point");
    if (points.length) await this.point(points[0].id, points[0].note);
  }

  /** Spotlights a control (switching to its view first). Never clicks it. */
  async point(id: string, note: string) {
    const t = guideTarget(id);
    if (!t) return;
    // Views that share a sidebar entry: point at the entry, and make a click
    // on it open the view the guide is about.
    const rt = railTarget(id);
    if (rt) {
      id = rt.id;
      ui.lastInRail = { ...ui.lastInRail, [rt.row]: rt.view };
    }
    if (t.view && ui.section !== t.view) ui.go(t.view);
    // Settings has tabs: open the one that holds the control.
    if (t.view === "settings") ui.settingsTab = settingsTabFor(id) ?? ui.settingsTab;
    // Let the view render before looking for the control.
    await new Promise((r) => setTimeout(r, 60));
    const el = document.querySelector<HTMLElement>(`[data-guide="${id}"]`);
    if (!el) {
      this.flash("confused");
      return;
    }
    el.scrollIntoView({ block: "nearest", inline: "nearest" });
    this.spotlight = { id, note: note || t.what };
    clearTimeout(this.spotTimer);
    this.spotTimer = setTimeout(() => this.clearSpotlight(), 8000);
  }

  clearSpotlight() {
    clearTimeout(this.spotTimer);
    this.spotlight = null;
  }
}

export const helper = new HelperStore();
