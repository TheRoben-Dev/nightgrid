/** SSH key manager state (M10k): Settings → SSH KEYS. */
import { api, errorText, type Host, type SshKey } from "../api";
import { homeOf, keyLabel, rotationSteps, switchedInput, type HostKeys, type RotateStep } from "../sshkeys";
import { hosts } from "./hosts.svelte";

/** At most this many hosts are read at once. */
const PARALLEL = 4;

export type StepState = "pending" | "running" | "done" | "failed" | "skipped";
export interface RotateRow {
  hostId: number;
  name: string;
  steps: { step: RotateStep; state: StepState }[];
  error: string | null;
}

class SshKeysStore {
  dir = $state("");
  keys = $state<SshKey[]>([]);
  agentError = $state<string | null>(null);
  error = $state<string | null>(null);
  busy = $state(false);
  /** host id → what its authorized_keys held at the last check. */
  results = $state<Record<number, HostKeys>>({});
  checking = $state(false);
  /** Hosts with a push or removal running. */
  working = $state<number[]>([]);
  rotation = $state<RotateRow[] | null>(null);

  get home(): string {
    return homeOf(this.dir);
  }

  async load() {
    try {
      const l = await api.keysList();
      this.dir = l.dir;
      this.keys = l.keys;
      this.agentError = l.agentError;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Generates a key; returns it, or null with `error` set. */
  async generate(name: string, comment: string, passphrase: string | null): Promise<SshKey | null> {
    this.busy = true;
    try {
      const k = await api.keysGenerate(name, comment, passphrase || null);
      await this.load();
      this.error = null;
      return k;
    } catch (e) {
      this.error = errorText(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  async readHost(hostId: number) {
    try {
      const entries = await api.keysAuthorized(hostId);
      this.results = { ...this.results, [hostId]: { hostId, entries, error: null } };
    } catch (e) {
      this.results = { ...this.results, [hostId]: { hostId, entries: null, error: errorText(e) } };
    }
  }

  /** Reads authorized_keys on every saved host (a few at a time). */
  async checkAll(list: Host[] = hosts.hosts) {
    this.checking = true;
    const queue = list.map((h) => h.id);
    const worker = async () => {
      for (let id = queue.shift(); id !== undefined; id = queue.shift()) await this.readHost(id);
    };
    await Promise.all(Array.from({ length: Math.min(PARALLEL, queue.length) }, worker));
    this.checking = false;
  }

  private async on(hostId: number, f: () => Promise<unknown>) {
    this.working = [...this.working, hostId];
    try {
      await f();
      this.error = null;
    } catch (e) {
      this.error = `${hosts.byId(hostId)?.name ?? "host"}: ${errorText(e)}`;
    } finally {
      this.working = this.working.filter((x) => x !== hostId);
      await this.readHost(hostId);
    }
  }

  push(hostId: number, fingerprint: string) {
    return this.on(hostId, () => api.keysPush(hostId, fingerprint));
  }

  remove(hostId: number, fingerprint: string) {
    return this.on(hostId, () => api.keysRemove(hostId, fingerprint));
  }

  /**
   * Rotates `from` → `to` on each host, one host at a time: add the new key,
   * log in with it, optionally point Nightgrid at it, and only then remove
   * the old key. A failed step stops that host with the old key in place.
   */
  async rotate(from: SshKey, to: SshKey, hostIds: number[], opts: { switchHosts: boolean; removeOld: boolean; passphrase: string | null }) {
    if (!to.file) return;
    const plan = rotationSteps(opts);
    this.rotation = hostIds.map((id) => ({
      hostId: id,
      name: hosts.byId(id)?.name ?? `#${id}`,
      steps: plan.map((step) => ({ step, state: "pending" as StepState })),
      error: null,
    }));
    const file = to.file;
    for (let i = 0; i < hostIds.length; i++) {
      const id = hostIds[i];
      const run: Record<RotateStep, () => Promise<unknown>> = {
        push: () => api.keysPush(id, to.fingerprint),
        verify: () => api.keysVerify(id, file, opts.passphrase),
        switch: async () => {
          const h = hosts.byId(id);
          if (h) await api.hostUpdate(id, switchedInput(h, file));
        },
        remove: () => api.keysRemove(id, from.fingerprint),
      };
      for (let s = 0; s < plan.length; s++) {
        this.setStep(i, s, "running");
        try {
          await run[plan[s]]();
          this.setStep(i, s, "done");
        } catch (e) {
          this.setStep(i, s, "failed", `${keyLabel(to)}: ${errorText(e)}`);
          for (let r = s + 1; r < plan.length; r++) this.setStep(i, r, "skipped");
          break;
        }
      }
      await this.readHost(id);
    }
    if (opts.switchHosts) await hosts.refresh();
  }

  private setStep(row: number, step: number, state: StepState, error?: string) {
    if (!this.rotation) return;
    const rows = this.rotation.slice();
    const r = { ...rows[row], steps: rows[row].steps.slice() };
    r.steps[step] = { ...r.steps[step], state };
    if (error) r.error = error;
    rows[row] = r;
    this.rotation = rows;
  }
}

export const sshKeys = new SshKeysStore();
