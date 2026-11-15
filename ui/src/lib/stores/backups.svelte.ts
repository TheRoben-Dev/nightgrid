/**
 * Backup status board (M10g): jobs across every host and their status.
 * Each host is read with one script; hosts that would need a password or a
 * host key decision are skipped with a note (no prompts from the board).
 */
import { api, errorText, systemError, type BackupJob, type BackupJobInput, type BackupStatus, type BackupSuggestion } from "../api";
import { hosts } from "./hosts.svelte";

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
  /** The job being edited (id null = new), or null when the editor is closed. */
  editing = $state<{ id: number | null; input: BackupJobInput } | null>(null);
  saveError = $state<string | null>(null);
  suggestions = $state<BackupSuggestion[] | null>(null);
  detecting = $state(false);
  detectHost = $state<number | null>(null);
  pendingDelete = $state<BackupJob | null>(null);

  async load() {
    try {
      this.jobs = await api.backupJobsList();
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Reads every host that has jobs, three at a time. */
  async checkAll() {
    if (this.checking) return;
    this.checking = true;
    const ids = [...new Set(this.jobs.map((j) => j.hostId))];
    const worker = async () => {
      for (let id = ids.shift(); id !== undefined; id = ids.shift()) await this.checkHost(id);
    };
    try {
      await Promise.all([worker(), worker(), worker()]);
      this.checkedAt = Date.now();
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
      this.editing = { id: null, input: emptyInput(hostId ?? hosts.hosts[0]?.id ?? 0) };
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
