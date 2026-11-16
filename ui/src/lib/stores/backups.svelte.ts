/**
 * Backup status board (M10g): jobs across every host and their status.
 * Each host is read with one script; hosts that would need a password or a
 * host key decision are skipped with a note (no prompts from the board).
 */
import { api, errorText, systemError, type BackupJob, type BackupJobInput, type BackupStatus, type BackupSuggestion, type LastTest, type RestoreTest } from "../api";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

export const DEFAULT_STALE_HOURS = 26;

export const TOOL_LABEL: Record<BackupJob["tool"], string> = {
  restic: "restic",
  borg: "borg",
  rsync: "rsync",
  rclone: "rclone",
  other: "other",
};

/** "1.2 GB" (decimal, like the tools print). */
export function bytes(n: number | null | undefined): string {
  if (n == null) return "";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

/** "26 hours", "3 days". */
export function staleWords(h: number): string {
  if (h % 24 === 0 && h >= 48) return `${h / 24} days`;
  return h === 1 ? "1 hour" : `${h} hours`;
}

export type JobState = "ok" | "failed" | "stale" | "running" | "unknown" | "error";

/** One word for a job's row. */
export function jobState(s: BackupStatus | undefined): JobState {
  if (!s) return "unknown";
  if (s.error) return "error";
  if (s.result === "running") return "running";
  if (s.result === "failed") return "failed";
  if (s.stale) return "stale";
  return s.result === "ok" ? "ok" : "unknown";
}

/** Why a job can't be test-restored (mirrors the core's rule), or null. */
export function testUnsupported(j: BackupJob): string | null {
  if (j.tool === "rclone") return "rclone remotes can't be test-restored yet";
  if ((j.tool === "rsync" || j.tool === "other") && j.source === "unit") return "add a path job for the copy's folder to test it";
  return null;
}

export const emptyInput = (hostId: number): BackupJobInput => ({
  hostId,
  name: "",
  tool: "restic",
  source: "unit",
  target: "",
  staleHours: DEFAULT_STALE_HOURS,
});

class BackupsStore {
  jobs = $state<BackupJob[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  /** jobId → status. */
  status = $state<Record<number, BackupStatus>>({});
  /** hostId → why it couldn't be read. */
  hostErrors = $state<Record<number, string>>({});
  checking = $state(false);
  checkedAt = $state<number | null>(null);
  /** When any read (all hosts or some) last finished (unix ms). */
  readAt = $state<number | null>(null);
  /** The job being edited (id null = new), or null when the editor is closed. */
  editing = $state<{ id: number | null; input: BackupJobInput } | null>(null);
  saveError = $state<string | null>(null);
  suggestions = $state<BackupSuggestion[] | null>(null);
  detecting = $state(false);
  detectHost = $state<number | null>(null);
  pendingDelete = $state<BackupJob | null>(null);
  /** jobId → its last test restore. */
  tests = $state<Record<number, LastTest>>({});
  /** Jobs with a test restore running. */
  testing = $state<number[]>([]);
  /** The test whose details are open. */
  testResult = $state<RestoreTest | null>(null);
  /** Asking before a test restore runs (it uses root on the host). */
  confirmTest = $state<BackupJob | null>(null);

  async loadTests() {
    try {
      const list = await api.backupTestsList();
      this.tests = Object.fromEntries(list.map((t) => [t.jobId, t]));
    } catch {
      // Leave the last known results.
    }
  }

  /**
   * Restores one file from the job's latest backup into a temporary folder
   * on the host, checks it and deletes it (root, with the unit's own
   * environment). Asks for sudo when the host wants it.
   */
  async testRestore(job: BackupJob) {
    this.confirmTest = null;
    if (this.testing.includes(job.id)) return;
    this.testing = [...this.testing, job.id];
    const host = hosts.byId(job.hostId)?.name ?? "the host";
    try {
      const t = await system.withSudo(`test-restore ${job.name}`, (sudo) => api.backupTestRestore(job.id, sudo), host);
      this.tests = { ...this.tests, [job.id]: { jobId: job.id, at: t.at, ok: t.ok, note: t.ok ? `restored ${t.file ?? "a file"}${t.snapshot ? ` from ${t.snapshot}` : ""}` : (t.error ?? "failed") } };
      this.testResult = t;
    } catch (e) {
      if (errorText(e) === "Cancelled.") return;
      const at = Math.floor(Date.now() / 1000);
      this.testResult = { jobId: job.id, ok: false, file: null, size: null, sha256: null, snapshot: null, snapshotTime: null, steps: [], error: errorText(e), at };
    } finally {
      this.testing = this.testing.filter((x) => x !== job.id);
    }
  }

  async load() {
    try {
      this.jobs = await api.backupJobsList();
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Reads every host that has jobs (or those of them in `only`), three at a time. */
  async checkAll(only?: number[]) {
    if (this.checking) return;
    this.checking = true;
    const ids = [...new Set(this.jobs.map((j) => j.hostId))].filter((id) => !only || only.includes(id));
    const worker = async () => {
      for (let id = ids.shift(); id !== undefined; id = ids.shift()) await this.checkHost(id);
    };
    try {
      await Promise.all([worker(), worker(), worker()]);
      if (!only) this.checkedAt = Date.now();
      this.readAt = Date.now();
    } finally {
      this.checking = false;
    }
  }

  async checkHost(hostId: number) {
    try {
      const list = await api.backupJobsCheck(hostId);
      const next = { ...this.status };
      for (const s of list) next[s.jobId] = s;
      this.status = next;
      const { [hostId]: _gone, ...rest } = this.hostErrors;
      this.hostErrors = rest;
    } catch (e) {
      const { connect } = systemError(e);
      const msg = connect && connect.kind !== "network" && connect.kind !== "other" ? "Needs a login first (open a terminal on it)." : errorText(e);
      this.hostErrors = { ...this.hostErrors, [hostId]: msg };
    }
  }

  edit(job: BackupJob | null, hostId?: number) {
    this.saveError = null;
    if (job) {
      const { id, ...input } = job;
      this.editing = { id, input };
    } else {
      this.editing = { id: null, input: emptyInput(hostId ?? hosts.preferred("backups")?.id ?? 0) };
    }
  }

  async save() {
    const e = this.editing;
    if (!e) return;
    try {
      const job = await api.backupJobSave(e.id, e.input);
      this.editing = null;
      if (this.suggestions) this.suggestions = this.suggestions.filter((x) => !(x.target && x.target === job.target && this.detectHost === job.hostId));
      await this.load();
      void this.checkHost(job.hostId);
    } catch (err) {
      this.saveError = errorText(err);
    }
  }

  async remove(job: BackupJob) {
    this.pendingDelete = null;
    try {
      await api.backupJobDelete(job.id);
      const { [job.id]: _gone, ...rest } = this.status;
      this.status = rest;
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async detect(hostId: number) {
    this.detectHost = hostId;
    this.detecting = true;
    this.suggestions = null;
    try {
      const found = await api.backupJobsDetect(hostId);
      // Leave out what is already on the board.
      this.suggestions = found.filter((s) => !this.jobs.some((j) => j.hostId === hostId && j.target === s.target && s.target));
    } catch (e) {
      this.suggestions = [];
      this.hostErrors = { ...this.hostErrors, [hostId]: errorText(e) };
    } finally {
      this.detecting = false;
    }
  }

  /** Opens the editor pre-filled from a suggestion. */
  adopt(s: BackupSuggestion) {
    if (this.detectHost === null) return;
    this.saveError = null;
    this.editing = {
      id: null,
      input: { hostId: this.detectHost, name: s.name, tool: s.tool, source: s.source, target: s.target, staleHours: DEFAULT_STALE_HOURS },
    };
  }
}

export const backups = new BackupsStore();
