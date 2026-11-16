/**
 * Stacks from git (M10w): repositories checked out on the host, checked for
 * new commits, reviewed as a diff and deployed to one chosen commit.
 */
import { api, errorText, type GitStack, type GitStackInput, type GitStatus } from "../api";
import { docker } from "./docker.svelte";
import { stacks } from "./stacks.svelte";

export interface GitCheck {
  loading: boolean;
  status: GitStatus | null;
  error: string | null;
}

/** A deploy waiting for confirmation, with its diff. */
export interface PendingDeploy {
  git: GitStack;
  target: string;
  /** "update" moves forward to the remote tip; "rollback" goes back to the previous deploy. */
  mode: "update" | "rollback";
  subject: string;
  diff: string | null;
  diffError: string | null;
}

class GitStacksStore {
  open = $state(false);
  list = $state<GitStack[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  checks = $state<Record<number, GitCheck>>({});
  busy = $state(false);
  pending = $state<PendingDeploy | null>(null);
  /** One-line result to show after an action. */
  notice = $state<string | null>(null);
  private hostId: number | null = null;

  async show() {
    this.open = true;
    this.notice = null;
    await this.load();
    for (const g of this.list) if (!this.checks[g.id]) void this.check(g);
  }

  close() {
    this.open = false;
    this.pending = null;
  }

  async load() {
    const id = docker.hostId;
    if (id === null) return;
    if (id !== this.hostId) {
      this.hostId = id;
      this.checks = {};
    }
    this.loading = true;
    try {
      this.list = await api.gitStacksList(id);
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  async check(g: GitStack) {
    this.checks = { ...this.checks, [g.id]: { loading: true, status: this.checks[g.id]?.status ?? null, error: null } };
    try {
      const status = await api.gitStackCheck(g.id);
      this.checks = { ...this.checks, [g.id]: { loading: false, status, error: null } };
    } catch (e) {
      this.checks = { ...this.checks, [g.id]: { loading: false, status: null, error: errorText(e) } };
    }
  }

  /** Clones a repository on the host. Returns an error text or null. */
  async add(input: GitStackInput): Promise<string | null> {
    const id = docker.hostId;
    if (id === null) return "no host selected";
    this.busy = true;
    try {
      const r = await api.gitStackAdd(id, input);
      this.notice = r.adopted
        ? `Tracking the existing checkout in ${r.git.dir}.`
        : `Cloned into ${r.git.dir}.${r.stack ? ` Stack ${r.stack.name} is ready: deploy or press Up.` : " No compose file in that folder yet."}`;
      await this.load();
      void this.check(r.git);
      void stacks.load(true);
      return null;
    } catch (e) {
      return errorText(e);
    } finally {
      this.busy = false;
    }
  }

  async prepare(git: GitStack, target: string, mode: PendingDeploy["mode"], subject: string) {
    this.pending = { git, target, mode, subject, diff: null, diffError: null };
    try {
      const diff = await api.gitStackDiff(git.id, target);
      if (this.pending?.target === target) this.pending = { ...this.pending, diff };
    } catch (e) {
      if (this.pending?.target === target) this.pending = { ...this.pending, diffError: errorText(e) };
    }
  }

  /** Moves the checkout, then brings the stack up. */
  async deploy() {
    const p = this.pending;
    if (!p) return;
    this.busy = true;
    try {
      const r = await api.gitStackDeploy(p.git.id, p.target);
      this.pending = null;
      const snap = r.snapshot ? "Snapshot saved first." : `No snapshot (${r.snapshotError ?? "unknown"}).`;
      if (r.stack) {
        this.notice = `${p.mode === "rollback" ? "Rolled back" : "Updated"} to ${r.to.slice(0, 8)}. ${snap} Bringing ${r.stack.name} up…`;
        this.open = false;
        void stacks.start(r.stack, "up");
      } else {
        this.notice = `Checkout moved to ${r.to.slice(0, 8)}. ${snap} No compose file in the folder, so nothing was started.`;
      }
      await this.load();
      void this.check(r.git);
    } catch (e) {
      if (this.pending) this.pending = { ...this.pending, diffError: errorText(e) };
    } finally {
      this.busy = false;
    }
  }

  async remove(g: GitStack) {
    try {
      await api.gitStackRemove(g.id);
      this.notice = `Stopped tracking ${g.dir}. The files and the stack stay on the host.`;
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }
}

export const gitStacks = new GitStacksStore();
