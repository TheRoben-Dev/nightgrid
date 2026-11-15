import {
  api,
  errorText,
  type AutomationEvent,
  type HostNeeds,
  type OutLine,
  type Playbook,
  type PlaybookInput,
  type RunInspect,
  type RunSpec,
  type Snippet,
  type SnippetInput,
  type SnippetScope,
  type Step,
} from "../api";
import type { Group, Host } from "../api";
import { hosts } from "./hosts.svelte";
import { ui } from "./ui.svelte";

/** Lines kept per host; older ones are dropped and counted. */
export const MAX_LINES = 5000;

export type Source = { kind: "command" } | { kind: "snippet"; id: number } | { kind: "playbook"; id: number };

export type StepStatus = "running" | "ok" | "failed" | "stopped";
export interface StepState {
  label: string;
  status: StepStatus;
  code: number | null;
  error: string | null;
  ms: number;
}

export type HostStatus = "queued" | "running" | "ok" | "failed" | "stopped";
export interface HostRun {
  hostId: number;
  status: HostStatus;
  steps: StepState[];
  lines: OutLine[];
  /** Lines dropped from the front to stay under MAX_LINES. */
  dropped: number;
  code: number | null;
  error: string | null;
  needs: HostNeeds | null;
  ms: number;
}

export interface RunState {
  id: number | null;
  title: string;
  hostIds: number[];
  /** Step labels at start, so steps that never ran still show. */
  stepLabels: string[];
  hosts: Record<number, HostRun>;
  done: boolean;
  stopped: boolean;
  startedAt: number;
}

export interface PendingConfirm {
  title: string;
  destructive: string[];
  hostNames: string[];
  /** Text to type before running (several hosts), or null. */
  word: string | null;
}

export const newHostRun = (hostId: number): HostRun => ({
  hostId,
  status: "queued",
  steps: [],
  lines: [],
  dropped: 0,
  code: null,
  error: null,
  needs: null,
  ms: 0,
});

/** Applies one run event. Mutates `run` (a $state proxy in the store). */
export function applyEvent(run: RunState, e: AutomationEvent) {
  if (e.kind === "done") {
    run.done = true;
    run.stopped = e.stopped;
    for (const h of Object.values(run.hosts)) if (h.status === "queued" || h.status === "running") h.status = "stopped";
    return;
  }
  const h = (run.hosts[e.hostId] ??= newHostRun(e.hostId));
  switch (e.kind) {
    case "hostStart":
      h.status = "running";
      break;
    case "stepStart":
      h.steps[e.step] = { label: e.label, status: "running", code: null, error: null, ms: 0 };
      break;
    case "lines": {
      h.lines.push(...e.lines);
      const over = h.lines.length - MAX_LINES;
      if (over > 0) {
        h.lines.splice(0, over);
        h.dropped += over;
      }
      break;
    }
    case "stepEnd": {
      const s = (h.steps[e.step] ??= { label: `step ${e.step + 1}`, status: "running", code: null, error: null, ms: 0 });
      s.code = e.code;
      s.error = e.error;
      s.ms = e.ms;
      s.status = e.code === null && e.error === null ? "stopped" : e.error === null && e.code === 0 ? "ok" : "failed";
      break;
    }
    case "hostEnd":
      h.status = e.stopped ? "stopped" : e.ok ? "ok" : "failed";
      h.code = e.code;
      h.error = e.error;
      h.needs = e.needs;
      h.ms = e.ms;
      break;
  }
}

/**
 * What to type before a destructive run on several hosts (security rule 5):
 * the group's name when exactly one whole group is picked, otherwise the
 * number of hosts. One host needs no typing (the dialog names it).
 */
export function confirmWord(targets: number[], allHosts: Host[], groups: Group[], pickedGroup: number | null): string | null {
  if (targets.length <= 1) return null;
  if (pickedGroup !== null) {
    const members = allHosts.filter((h) => h.groupId === pickedGroup).map((h) => h.id);
    const same = members.length === targets.length && members.every((id) => targets.includes(id));
    const g = groups.find((x) => x.id === pickedGroup);
    if (same && g) return g.name;
  }
  return String(targets.length);
}

export function scopeLabel(s: SnippetScope): string {
  if (s.kind === "global") return "ALL HOSTS";
  if (s.kind === "host") return hosts.byId(s.hostId)?.name ?? "HOST";
  return hosts.groups.find((g) => g.id === s.groupId)?.name ?? "GROUP";
}

export function scopeApplies(s: SnippetScope, h: Host): boolean {
  return s.kind === "global" || (s.kind === "host" && s.hostId === h.id) || (s.kind === "group" && s.groupId === h.groupId);
}

export const fmtMs = (ms: number) => (ms < 1000 ? `${ms} ms` : ms < 60_000 ? `${(ms / 1000).toFixed(1)} s` : `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`);

export const emptySnippet = (): SnippetInput => ({ name: "", command: "", description: "", scope: { kind: "global" }, sudo: false });
export const toSnippetInput = (s: Snippet): SnippetInput => ({
  name: s.name,
  command: s.command,
  description: s.description,
  scope: s.scope,
  sudo: s.sudo,
});
export const emptyPlaybook = (): PlaybookInput => ({ name: "", description: "", steps: [newStep("command")], stopOnError: true });
export const toPlaybookInput = (p: Playbook): PlaybookInput => ({
  name: p.name,
  description: p.description,
  steps: p.steps.map((s) => ({ ...s })),
  stopOnError: p.stopOnError,
});

export function newStep(type: Step["type"]): Step {
  switch (type) {
    case "command":
      return { type, command: "", sudo: false };
    case "upload":
      return { type, localPath: "", remotePath: "", mode: null };
    case "waitPort":
      return { type, host: "127.0.0.1", port: 80, timeoutSecs: 60 };
    case "docker":
      return { type, action: "restart", target: "" };
  }
}

export const STEP_LABEL: Record<Step["type"], string> = {
  command: "COMMAND",
  upload: "UPLOAD FILE",
  waitPort: "WAIT FOR PORT",
  docker: "DOCKER",
};

class AutomationStore {
  snippets = $state<Snippet[]>([]);
  playbooks = $state<Playbook[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);

  source = $state<Source>({ kind: "command" });
  command = $state("");
  commandSudo = $state(false);
  targets = $state<number[]>([]);
  /** Set when targets came from a group chip (for the confirmation word). */
  pickedGroup = $state<number | null>(null);
  values = $state<Record<string, string>>({});
  sudoPassword = $state("");

  inspect = $state<RunInspect | null>(null);
  inspectError = $state<string | null>(null);
  run = $state<RunState | null>(null);
  /** Host whose output is shown. */
  shown = $state<number | null>(null);
  confirm = $state<PendingConfirm | null>(null);
  starting = $state(false);
  runError = $state<string | null>(null);

  /** null = closed, 0 = new, id = edit. */
  editingSnippet = $state<number | null>(null);
  editingPlaybook = $state<number | null>(null);
  /** Pre-filled command for "Save as snippet". */
  snippetDraft = $state<SnippetInput | null>(null);

  private inspectSeq = 0;

  async load() {
    try {
      const [s, p] = await Promise.all([api.snippetsList(), api.playbooksList()]);
      this.snippets = s;
      this.playbooks = p;
      this.error = null;
      this.dropMissing();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loaded = true;
    }
  }

  private dropMissing() {
    const s = this.source;
    if (s.kind === "snippet" && !this.snippets.some((x) => x.id === s.id)) this.source = { kind: "command" };
    if (s.kind === "playbook" && !this.playbooks.some((x) => x.id === s.id)) this.source = { kind: "command" };
  }

  get spec(): RunSpec | null {
    const s = this.source;
    if (s.kind === "command") return this.command.trim() ? { kind: "command", command: this.command, sudo: this.commandSudo } : null;
    return s;
  }

  get running(): boolean {
    return !!this.run && !this.run.done;
  }

  snippetsFor(h: Host | undefined): Snippet[] {
    return h ? this.snippets.filter((s) => scopeApplies(s.scope, h)) : this.snippets;
  }

  pick(source: Source) {
    this.source = source;
    this.inspect = null;
    this.values = {};
  }

  /** From the palette: pick a snippet, aimed at `hostId` when given. */
  openSnippet(id: number, hostId?: number) {
    this.pick({ kind: "snippet", id });
    if (hostId !== undefined) this.setTargets([hostId]);
    ui.go("automation");
  }

  setTargets(ids: number[], group: number | null = null) {
    this.targets = ids;
    this.pickedGroup = group;
  }

  toggleHost(id: number) {
    this.setTargets(this.targets.includes(id) ? this.targets.filter((x) => x !== id) : [...this.targets, id]);
  }

  /** Selects a whole group, or clears it if it is exactly what's selected. */
  toggleGroup(groupId: number) {
    const ids = hosts.hosts.filter((h) => h.groupId === groupId).map((h) => h.id);
    const all = ids.length > 0 && ids.every((id) => this.targets.includes(id)) && this.targets.length === ids.length;
    this.setTargets(all ? [] : ids, all ? null : groupId);
  }

  async refreshInspect() {
    const spec = this.spec;
    const seq = ++this.inspectSeq;
    if (!spec) {
      this.inspect = null;
      this.inspectError = null;
      return;
    }
    const first = this.targets[0] ?? null;
    try {
      const r = await api.automationInspect(spec, $state.snapshot(this.values), first, first !== null ? (hosts.byId(first)?.name ?? null) : null);
      if (seq === this.inspectSeq) {
        this.inspect = r;
        this.inspectError = null;
      }
    } catch (e) {
      if (seq === this.inspectSeq) {
        this.inspect = null;
        this.inspectError = errorText(e);
      }
    }
  }

  /** Validates and asks for confirmation when the plan is destructive. */
  async start() {
    this.runError = null;
    await this.refreshInspect();
    const i = this.inspect;
    if (!i || !this.spec) return;
    if (!this.targets.length) {
      this.runError = "Choose at least one host.";
      return;
    }
    if (i.missing.length) {
      this.runError = `Fill in: ${i.missing.join(", ")}`;
      return;
    }
    if (i.destructive.length) {
      this.confirm = {
        title: i.name,
        destructive: i.destructive,
        hostNames: this.targets.map((id) => hosts.byId(id)?.name ?? `host ${id}`),
        word: confirmWord(this.targets, hosts.hosts, hosts.groups, this.pickedGroup),
      };
      return;
    }
    await this.go();
  }

  async go() {
    this.confirm = null;
    const spec = this.spec;
    if (!spec || this.starting) return;
    const hostIds = [...this.targets];
    const title = this.inspect?.name === "command" ? (this.command.trim().split("\n")[0] ?? "command") : (this.inspect?.name ?? "run");
    const stepLabels = [...(this.inspect?.steps ?? [])];
    const run: RunState = { id: null, title, hostIds, stepLabels, hosts: {}, done: false, stopped: false, startedAt: Date.now() };
    for (const id of hostIds) run.hosts[id] = newHostRun(id);
    this.run = run;
    this.shown = hostIds[0] ?? null;
    this.starting = true;
    const pw = this.sudoPassword || null;
    try {
      const id = await api.automationRun(spec, hostIds, $state.snapshot(this.values), pw, (e) => {
        if (this.run) applyEvent(this.run, e);
      });
      if (this.run) this.run.id = id;
    } catch (e) {
      this.runError = errorText(e);
      if (this.run) {
        this.run.done = true;
        for (const h of Object.values(this.run.hosts)) h.status = "stopped";
      }
    } finally {
      this.starting = false;
      // Typed for this run only.
      this.sudoPassword = "";
    }
  }

  async stop() {
    if (this.run?.id != null) await api.automationStop(this.run.id);
  }

  /** Targets the hosts that failed last time (and keeps the same plan). */
  retryFailed() {
    if (!this.run) return;
    const failed = this.run.hostIds.filter((id) => this.run!.hosts[id]?.status === "failed" || this.run!.hosts[id]?.status === "stopped");
    if (failed.length) this.setTargets(failed);
  }

  async saveSnippet(id: number, input: SnippetInput): Promise<Snippet> {
    const s = await api.snippetSave(id || null, input);
    await this.load();
    return s;
  }

  async deleteSnippet(id: number) {
    await api.snippetDelete(id);
    await this.load();
  }

  async savePlaybook(id: number, input: PlaybookInput): Promise<Playbook> {
    const p = await api.playbookSave(id || null, input);
    await this.load();
    return p;
  }

  async deletePlaybook(id: number) {
    await api.playbookDelete(id);
    await this.load();
  }
}

export const automation = new AutomationStore();
