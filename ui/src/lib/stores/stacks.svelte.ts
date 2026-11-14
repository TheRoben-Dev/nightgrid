import { api, errorText, type ComposeEvent, type ComposeOp, type ImageChange, type Stack, type StackDir } from "../api";
import { docker } from "./docker.svelte";

/** Output lines kept per run. */
const MAX_LINES = 5000;

export const opLabel: Record<ComposeOp, string> = {
  up: "Up",
  pull: "Pull",
  update: "Update",
  restart: "Restart",
  down: "Down",
};

/** Needs a confirmation naming the stack and host (security rule 5). */
export const isDestructiveOp = (op: ComposeOp): boolean => op === "down" || op === "restart" || op === "update";

export const opConsequence: Record<ComposeOp, string> = {
  up: "Creates and starts the stack's services (docker compose up -d).",
  pull: "Downloads newer images. Running containers are not touched.",
  update: "Pulls newer images, then recreates every service whose image changed (pull + up -d). Those services restart.",
  restart: "Restarts every service in the stack (docker compose restart).",
  down: "Stops and removes the stack's containers and networks (docker compose down). Named volumes are kept.",
};

export interface ComposeRun {
  /** Local token, to drop events from an older run. */
  token: number;
  id: number | null;
  stack: Stack;
  op: ComposeOp;
  lines: string[];
  running: boolean;
  code: number | null;
  changes: ImageChange[] | null;
  error: string | null;
}

/** Default compose file to edit: the first listed, else compose.yaml in the dir. */
export const composeFile = (s: Stack): string => s.files[0] ?? `${s.dir}/compose.yaml`;
export const envFile = (s: Stack): string => `${s.dir}/.env`;

class StacksStore {
  stacks = $state<Stack[]>([]);
  dirs = $state<StackDir[]>([]);
  loading = $state(false);
  loaded = $state(false);
  error = $state<string | null>(null);
  run = $state<ComposeRun | null>(null);
  pending = $state<{ stack: Stack; op: ComposeOp } | null>(null);
  dirsOpen = $state(false);
  private hostId: number | null = null;
  private seq = 0;
  private runs = 0;

  get visible(): Stack[] {
    const q = docker.filter.trim().toLowerCase();
    return q ? this.stacks.filter((s) => s.name.includes(q) || s.dir.toLowerCase().includes(q)) : this.stacks;
  }

  get busy(): boolean {
    return !!this.run?.running;
  }

  async load(background = false) {
    const id = docker.hostId;
    if (id === null) return;
    if (id !== this.hostId) {
      this.hostId = id;
      this.stacks = [];
      this.dirs = [];
      this.loaded = false;
      if (!this.run?.running) this.run = null;
    }
    const seq = ++this.seq;
    if (!background) this.loading = true;
    try {
      const [stacks, dirs] = await Promise.all([api.dockerStacks(id), api.stackDirsList(id)]);
      if (seq !== this.seq || id !== docker.hostId) return;
      this.stacks = stacks;
      this.dirs = dirs;
      this.error = null;
      this.loaded = true;
    } catch (e) {
      if (seq !== this.seq) return;
      this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  request(stack: Stack, op: ComposeOp) {
    if (this.busy) return;
    if (isDestructiveOp(op)) this.pending = { stack, op };
    else void this.start(stack, op);
  }

  async start(stack: Stack, op: ComposeOp) {
    const hostId = docker.hostId;
    this.pending = null;
    if (hostId === null || this.busy) return;
    const token = ++this.runs;
    const run: ComposeRun = { token, id: null, stack, op, lines: [], running: true, code: null, changes: null, error: null };
    this.run = run;
    const on = (e: ComposeEvent) => {
      const r = this.run;
      if (!r || r.token !== token) return;
      if (e.kind === "lines") {
        const next = r.lines.concat(e.lines);
        r.lines = next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
      } else {
        r.running = false;
        r.code = e.code;
        r.changes = e.changes;
        r.error = e.error;
        void this.load(true);
        void docker.refresh(true);
      }
    };
    try {
      const id = await api.dockerComposeRun(hostId, stack, op, on);
      if (this.run?.token === token) this.run.id = id;
    } catch (e) {
      if (this.run?.token === token) {
        this.run.running = false;
        this.run.error = errorText(e);
      }
    }
  }

  async cancel() {
    if (this.run?.running && this.run.id !== null) await api.dockerComposeCancel(this.run.id);
  }

  async addDir(path: string): Promise<string | null> {
    if (docker.hostId === null) return null;
    try {
      await api.stackDirsAdd(docker.hostId, path);
      await this.load(true);
      return null;
    } catch (e) {
      return errorText(e);
    }
  }

  async removeDir(id: number) {
    await api.stackDirsDelete(id);
    await this.load(true);
  }
}

export const stacks = new StacksStore();
