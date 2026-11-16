/** Login & access audit (M10u): Fleet → ACCESS. Reports live in memory only. */
import { api, errorText, type AccessReport } from "../api";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

/** Hosts audited at once by "Audit all". */
const PARALLEL = 3;

export interface AuditState {
  busy: boolean;
  error: string | null;
}

class AccessStore {
  reports = $state<Record<number, AccessReport>>({});
  state = $state<Record<number, AuditState>>({});
  auditingAll = $state(false);

  private set(id: number, s: AuditState) {
    this.state = { ...this.state, [id]: s };
  }

  /** `elevate` asks for the sudo password when it isn't remembered. */
  async audit(id: number, elevate = false) {
    if (this.state[id]?.busy) return;
    this.set(id, { busy: true, error: null });
    try {
      const name = hosts.hosts.find((h) => h.id === id)?.name;
      const r = elevate ? await system.withSudo("read the access audit as root", (sudo) => api.accessAudit(id, true, sudo), name) : await api.accessAudit(id, false, null);
      this.reports = { ...this.reports, [id]: r };
      this.set(id, { busy: false, error: null });
    } catch (e) {
      this.set(id, { busy: false, error: errorText(e) });
    }
  }

  /** Every host, a few at a time, never prompting for sudo. */
  async auditAll() {
    if (this.auditingAll) return;
    this.auditingAll = true;
    const queue = hosts.hosts.map((h) => h.id);
    const worker = async () => {
      for (let id = queue.shift(); id !== undefined; id = queue.shift()) await this.audit(id);
    };
    await Promise.all(Array.from({ length: Math.min(PARALLEL, queue.length) }, worker));
    this.auditingAll = false;
  }
}

export const access = new AccessStore();
