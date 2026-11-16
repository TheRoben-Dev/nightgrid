/**
 * DRIFT helper state: the side panel, the conversation, the attached view
 * context, the model download and the "show me" spotlight.
 */
import { api, type HelperStatus } from "../api";
import { guideTarget } from "../helper/guide";
import { guideIndex, noteRefs, notesIndex } from "../helper/knowledge";
import { tour } from "./tour.svelte";
import { buildTurns, setFeatureCheck, parseReply, streamingText, type HelperAction, type HelperContext, type Turn } from "../helper/prompt";
import { sectionById } from "../sections";
import { labelLines } from "../netmap/graph";
import { keymap } from "../term/keyhints";
import { historyTerms, hostFacts, mentionedHosts } from "../helper/hostFacts";
import { checksText, runSkills, summaryCheck, type Check } from "../helper/skills";
import { wantsSummary, type SummaryInput } from "../helper/skills/summary";
import { diskRows, totals, updateRows } from "../fleet";
import { backups, jobState } from "./backups.svelte";
import { fleet } from "./fleet.svelte";
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
  /** Notes the answer was given, to open from the reply. */
  noteRefs?: { id: number; title: string }[];
  stats?: string;
  /** What was attached to a question ("Docker logs: web · 150 lines"). */
  attached?: string;
  /** Exact skill results shown above the reply (and given to the model). */
  checks?: Check[];
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
  /** Context attached to the next question (the user can drop parts). */
  context = $state<HelperContext>({ view: "overview" });
  spotlight = $state<Spotlight | null>(null);
  /** A brief mood after something happened (success / confused). */
  mood = $state<DriftState | null>(null);
  typing = $state(false);
  /** Text put in the question box for the user to send or edit. */
  draft = $state("");
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

  async download() {
    this.downloadError = null;
    this.progress = { done: this.status?.have ?? 0, total: this.status?.bytes ?? 0 };
    try {
      await api.helperDownload((p) => (this.progress = { done: p.done ?? 0, total: p.total ?? 0 }));
    } catch (e) {
      this.downloadError = errMsg(e);
    } finally {
      this.progress = null;
      await this.refresh();
    }
  }

  cancelDownload() {
    void api.helperCancelDownload();
  }

  async deleteModel() {
    try {
      await api.helperDeleteModel();
    } catch (e) {
      this.downloadError = errMsg(e);
    }
    await this.refresh();
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
    if (!found.length) return [];
    const now = Math.floor(Date.now() / 1000);
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
        return hostFacts({ host: h, ping: hosts.pings[h.id], metrics: metrics.byHost[h.id], alerts: mine, history, historyMatches, historyError, now });
      }),
    );
  }

  async ask(question: string) {
    const q = question.trim();
    if (!q || this.busy) return;
    const ctx = { ...this.context, view: this.context.view ?? ui.section };
    if (!notes.loaded) await notes.load();
    if (!netmap.labelsLoaded) await netmap.loadLabels();
    ctx.devices = labelLines(netmap.labels, netmap.result?.devices ?? [], q, ctx.view === "network");
    ctx.hosts = await this.hostContext(q);
    const checks = [...this.pendingChecks, ...runSkills(q, ctx.selection)];
    this.pendingChecks = [];
    if (wantsSummary(q)) checks.unshift(summaryCheck(this.summaryInput()));
    ctx.checks = checks.length ? checksText(checks) : null;
    // Questions about the user's own hosts (or with exact checks) lean on those, not the guide.
    const guideN = ctx.hosts.length || checks.length ? 2 : 4;
    const docs = [...guideIndex().search(`${q} ${sectionById(ctx.view).label}`, guideN), ...notesIndex(notes.notes).search(q, 2)];
    const history: Turn[] = this.messages
      .filter((m) => !m.error && m.text)
      .map((m) => ({ role: m.role, content: m.text }));
    const sent = buildTurns(history, q, ctx, docs);
    const sel = ctx.selection?.trim();
    const n = sel ? sel.split("\n").length : 0;
    const attached = sel ? `${ctx.source ?? "selection"} · ${n} line${n === 1 ? "" : "s"}` : undefined;
    this.messages.push({ id: ++this.seq, role: "user", text: q, actions: [], attached });
    this.messages.push({ id: ++this.seq, role: "assistant", text: "", actions: [], streaming: true, sent, sources: docs.map((d) => d.title), noteRefs: noteRefs(docs), checks });
    const msg = this.messages[this.messages.length - 1];
    // Selection is used once; the view context stays.
    this.context = { ...this.context, selection: null, source: null };
    this.busy = true;
    this.mood = null;
    this.clearSpotlight();
    let raw = "";
    try {
      const reply = await api.helperAsk(sent, (e) => {
        if (e.kind === "loading") this.loading = true;
        else if (e.kind === "thinking") this.loading = false;
        else if (e.kind === "text") {
          raw += e.text;
          msg.text = streamingText(raw);
        }
      });
      const parsed = parseReply(reply.text || raw);
      msg.text = parsed.text;
      msg.actions = parsed.actions;
      const secs = reply.seconds ?? 0;
      const tps = secs > 0 ? ` · ${(reply.tokens / secs).toFixed(1)} tok/s` : "";
      msg.stats = `${reply.promptTokens} in · ${reply.tokens} out${tps}${reply.finish === "length" ? " · cut at the length limit" : reply.finish === "stopped" ? " · stopped" : ""}`;
      if (!parsed.text && !parsed.actions.length) {
        msg.text = "I don't have an answer for that.";
        this.flash("confused");
      }
    } catch (e) {
      msg.error = errMsg(e);
      if (raw) msg.text = parseReply(raw).text;
    } finally {
      msg.streaming = false;
      this.busy = false;
      this.loading = false;
      void this.refresh();
    }
  }

  stop() {
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
    const upd = updateRows(hosts.hosts, fleet.updates);
    const t = totals(upd);
    const disks = diskRows(hosts.hosts, fleet.disks, settings.value.alerts.forecastDays).filter((r) => r.tone === "bad" || r.tone === "warn");
    return {
      now,
      active: (snap?.active ?? []).map((a) => ({ title: a.title, detail: a.detail, since: a.since, host: name(a.hostId), severity: a.severity })),
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
    if (t.view && ui.section !== t.view) ui.go(t.view);
    // Settings has two tabs; the feature controls live on the second.
    if (t.view === "settings") ui.settingsTab = id.startsWith("settings-feature") ? "features" : id === "settings-tab-features" ? ui.settingsTab : "general";
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
