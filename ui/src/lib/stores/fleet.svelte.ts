/**
 * Fleet health (M10m): the update center. Checks every host's packages and,
 * on request, container images; upgrades the hosts you pick in one batch.
 * Hosts that would need a login, a host key decision or a sudo password that
 * isn't remembered are reported, not prompted for (one prompt per host would
 * be easy to click through).
 */
import { api, errorText, systemError } from "../api";
import type { FleetTab, HostRun, HostUpdates, ImageRow } from "../fleet";
import { hosts } from "./hosts.svelte";

/** Hosts read at once. */
const WORKERS = 3;
/** Opening the view re-checks when the last check is older than this. */
const FRESH_MS = 30 * 60_000;

function hostError(e: unknown): string {
  const { connect, system } = systemError(e);
  if (connect && connect.kind !== "network" && connect.kind !== "other") return "needs a login first (open a terminal on it)";
  if (system?.kind === "sudoRequired") return "needs a sudo password: upgrade it from System → UPDATES (and tick remember)";
  if (system?.kind === "noSudo") return "this login can't use sudo";
  return errorText(e);
}

async function pool<T>(items: T[], fn: (x: T) => Promise<void>) {
  const queue = items.slice();
  const worker = async () => {
    for (let x = queue.shift(); x !== undefined; x = queue.shift()) await fn(x);
  };
  await Promise.all(Array.from({ length: Math.min(WORKERS, queue.length) }, worker));
}

class FleetStore {
  tab = $state<FleetTab>("updates");
  updates = $state<Record<number, HostUpdates>>({});
  checking = $state(false);
  checkedAt = $state<number | null>(null);
  images = $state<ImageRow[] | null>(null);
  /** hostId → why its images couldn't be checked (no Docker, offline, ...). */
  imageSkipped = $state<Record<number, string>>({});
  imagesChecking = $state(false);
  selected = $state<number[]>([]);
  confirm = $state(false);
  runs = $state<Record<number, HostRun>>({});
  /** Host whose package list / run log is open. */
  open = $state<number | null>(null);

  /** On opening the view: check unless a recent result is there. */
  ensure() {
    if (this.checking) return;
    if (this.checkedAt === null || Date.now() - this.checkedAt > FRESH_MS) void this.checkAll();
  }

  async checkAll() {
    if (this.checking) return;
    this.checking = true;
    this.runs = Object.fromEntries(Object.entries(this.runs).filter(([, r]) => r.running));
    try {
      await pool(
        hosts.hosts.map((h) => h.id),
        (id) => this.checkHost(id),
      );
      this.checkedAt = Date.now();
    } finally {
      this.checking = false;
    }
  }

  async checkHost(id: number) {
    const prev = this.updates[id];
    this.updates = { ...this.updates, [id]: { status: "checking", report: prev?.report ?? null, error: null, checkedAt: prev?.checkedAt ?? null } };
    try {
      const report = await api.systemUpdates(id);
      this.updates = { ...this.updates, [id]: { status: "ok", report, error: null, checkedAt: Date.now() } };
    } catch (e) {
      this.updates = { ...this.updates, [id]: { status: "error", report: null, error: hostError(e), checkedAt: Date.now() } };
    }
    this.selected = this.selected.filter((x) => x !== id || (this.updates[id]?.report?.updates.length ?? 0) > 0);
  }

  /** Asks registries whether any stack's image tags moved (rule 7: registries, on request). */
  async checkImages() {
    if (this.imagesChecking) return;
    this.imagesChecking = true;
    const rows: ImageRow[] = [];
    const skipped: Record<number, string> = {};
    try {
      await pool(
        hosts.hosts.map((h) => h.id),
        async (hostId) => {
          let stacks;
          try {
            stacks = await api.dockerStacks(hostId);
          } catch (e) {
            skipped[hostId] = hostError(e);
            return;
          }
          for (const stack of stacks) {
            try {
              for (const update of await api.dockerStackCheckUpdates(hostId, stack)) rows.push({ hostId, stack, update });
            } catch (e) {
              skipped[hostId] = `${stack.name}: ${errorText(e)}`;
            }
          }
        },
      );
    } finally {
      this.images = rows;
      this.imageSkipped = skipped;
      this.imagesChecking = false;
    }
  }

  toggle(id: number) {
    this.selected = this.selected.includes(id) ? this.selected.filter((x) => x !== id) : [...this.selected, id];
  }

  get running(): boolean {
    return Object.values(this.runs).some((r) => r.running);
  }

  /** Upgrades the selected hosts (confirmed by typing, rule 5), three at a time. */
  async upgradeSelected() {
    this.confirm = false;
    const ids = this.selected.slice();
    this.selected = [];
    const next: Record<number, HostRun> = { ...this.runs };
    for (const id of ids) next[id] = { lines: [], running: true, code: null, error: null, skipped: null };
    this.runs = next;
    await pool(ids, (id) => this.upgradeHost(id));
  }

  private setRun(id: number, patch: Partial<HostRun>) {
    const r = this.runs[id];
    if (r) this.runs = { ...this.runs, [id]: { ...r, ...patch } };
  }

  private async upgradeHost(id: number) {
    const manager = this.updates[id]?.report?.manager;
    if (!manager || !this.updates[id]?.report?.updates.length) {
      this.setRun(id, { running: false, skipped: "nothing to upgrade" });
      return;
    }
    try {
      await new Promise<void>((resolve, reject) => {
        api
          .systemUpgradeRun(id, manager, "upgrade", null, (e) => {
            const r = this.runs[id];
            if (!r) return;
            if (e.kind === "lines") this.setRun(id, { lines: [...r.lines, ...e.lines].slice(-2000) });
            else {
              this.setRun(id, { running: false, code: e.code ?? null, error: e.error ?? null });
              resolve();
            }
          })
          .catch(reject);
      });
    } catch (e) {
      const { system } = systemError(e);
      const sudo = system?.kind === "sudoRequired" || system?.kind === "noSudo";
      this.setRun(id, { running: false, skipped: sudo ? hostError(e) : null, error: sudo ? null : hostError(e) });
      return;
    }
    await this.checkHost(id);
  }

  dismissRun(id: number) {
    const { [id]: _gone, ...rest } = this.runs;
    this.runs = rest;
  }
}

export const fleet = new FleetStore();
