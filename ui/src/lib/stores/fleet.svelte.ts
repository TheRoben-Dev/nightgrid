/**
 * Fleet health (M10m): the update center. Checks every host's packages and,
 * on request, container images; upgrades the hosts you pick in one batch.
 * Hosts that would need a login, a host key decision or a sudo password that
 * isn't remembered are reported, not prompted for (one prompt per host would
 * be easy to click through).
 */
import { api, errorText, systemError, type GrowthReport, type PoolsReport, type StorageLink } from "../api";
import { isGuest, type FleetTab, type HostDisks, type HostRun, type HostUpdates, type ImageRow } from "../fleet";
import { system } from "./system.svelte";
import { backups } from "./backups.svelte";
import { holds } from "./holds.svelte";
import { hosts } from "./hosts.svelte";
import { nav } from "./nav.svelte";

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
  disks = $state<Record<number, HostDisks>>({});
  disksChecking = $state(false);
  disksAt = $state<number | null>(null);
  /** "What's growing" scans by `hostId|path`. */
  growth = $state<Record<string, { busy: boolean; report: GrowthReport | null; error: string | null }>>({});
  /** Hosts whose disks are stored on another host's drive. */
  links = $state<StorageLink[]>([]);

  /** Scans a folder on a host; `elevate` reads as root (asks for sudo). */
  async scanGrowth(hostId: number, path: string, elevate = false) {
    const key = `${hostId}|${path}`;
    const prev = this.growth[key];
    this.growth = { ...this.growth, [key]: { busy: true, report: prev?.report ?? null, error: null } };
    try {
      const name = hosts.hosts.find((h) => h.id === hostId)?.name;
      const report = elevate
        ? await system.withSudo(`read every folder under ${path}`, (sudo) => api.diskGrowth(hostId, path, true, sudo), name ?? "this host")
        : await api.diskGrowth(hostId, path, false, null);
      this.growth = { ...this.growth, [key]: { busy: false, report, error: null } };
    } catch (e) {
      this.growth = { ...this.growth, [key]: { busy: false, report: prev?.report ?? null, error: hostError(e) } };
    }
  }

  async loadLinks() {
    this.links = await api.storageLinks().catch(() => this.links);
  }

  /** "`hostId`'s disks are stored on `owner`'s `mount`", then re-reads both hosts' forecasts. */
  async addLink(hostId: number, owner: number, mount: string) {
    await api.storageLinkAdd(hostId, owner, mount);
    await this.loadLinks();
    await this.refreshForecasts([hostId, owner]);
  }

  async removeLink(id: number) {
    const l = this.links.find((x) => x.id === id);
    await api.storageLinkDelete(id);
    await this.loadLinks();
    if (l) await this.refreshForecasts([l.hostId, l.ownerHostId]);
  }

  /** Forecasts only (local, no connection). */
  async refreshForecasts(ids: number[]) {
    for (const id of ids) {
      const cur = this.disks[id];
      if (!cur) continue;
      const forecasts = await api.diskForecasts(id).catch(() => cur.forecasts);
      this.disks = { ...this.disks, [id]: { ...this.disks[id], forecasts } };
    }
  }

  /** Opens the BACKUPS tab, reading the board once per session. */
  openBackups() {
    this.tab = "backups";
    if (!backups.loaded) void backups.load().then(() => backups.checkAll());
    void backups.loadTests();
  }

  /** Opening the DISKS tab: forecasts right away (local), pools and SMART when stale. */
  ensureDisks() {
    void this.loadLinks();
    if (this.disksChecking) return;
    if (this.disksAt === null || Date.now() - this.disksAt > FRESH_MS) void this.checkDisks();
  }

  /** Reads every host, or just `ids` (the Overview reads servers that are online). */
  async checkDisks(ids?: number[]) {
    if (this.disksChecking) return;
    this.disksChecking = true;
    try {
      await pool(
        ids ?? hosts.hosts.map((h) => h.id),
        (id) => this.checkDisksHost(id),
      );
      if (!ids) this.disksAt = Date.now();
    } finally {
      this.disksChecking = false;
    }
  }

  /**
   * Forecasts come from this computer's history; pools are read without
   * root; SMART only where sudo needs no prompt (never asks here).
   */
  async checkDisksHost(id: number) {
    const prev = this.disks[id];
    const forecasts = await api.diskForecasts(id).catch(() => prev?.forecasts ?? []);
    this.disks = {
      ...this.disks,
      [id]: { status: "checking", error: null, pools: prev?.pools ?? null, smart: prev?.smart ?? null, smartNote: prev?.smartNote ?? null, forecasts, checkedAt: prev?.checkedAt ?? null },
    };
    let pools: PoolsReport;
    try {
      pools = await api.systemPools(id);
    } catch (e) {
      this.disks = { ...this.disks, [id]: { ...this.disks[id], status: "error", error: hostError(e), checkedAt: Date.now() } };
      return;
    }
    let smart = null;
    let smartNote: string | null = null;
    try {
      smart = await api.systemDisks(id, null);
    } catch (e) {
      const { system } = systemError(e);
      smartNote =
        system?.kind === "sudoRequired"
          ? "needs a sudo password: read it once from System → DISKS and tick remember"
          : system?.kind === "missing"
            ? "smartctl isn't installed (smartmontools)"
            : system?.kind === "noSudo"
              ? "this login can't use sudo"
              : errorText(e);
      // WSL or a VM guest: there is no disk of its own to read.
      if (system?.kind !== "sudoRequired" && isGuest(forecasts)) smartNote = "not here: the disks belong to the machine this runs on, so read SMART there";
    }
    this.disks = { ...this.disks, [id]: { status: "ok", error: null, pools, smart, smartNote, forecasts, checkedAt: Date.now() } };
  }

  /** On opening the view: check unless a recent result is there. */
  ensure() {
    if (this.checking) return;
    if (this.checkedAt === null || Date.now() - this.checkedAt > FRESH_MS) void this.checkAll();
  }

  /** Checks every host, or just `ids` (the Overview checks servers that are online). */
  async checkAll(ids?: number[]) {
    if (this.checking) return;
    this.checking = true;
    holds.ensure();
    this.runs = Object.fromEntries(Object.entries(this.runs).filter(([, r]) => r.running));
    try {
      await pool(
        ids ?? hosts.hosts.map((h) => h.id),
        (id) => this.checkHost(id),
      );
      if (!ids) this.checkedAt = Date.now();
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
    this.selected = this.selected.filter((x) => x !== id || (this.updates[id]?.report?.updates.some((u) => !u.held) ?? false));
  }

  /** Asks registries whether any stack's image tags moved (rule 7: registries, on request). */
  async checkImages() {
    if (this.imagesChecking) return;
    this.imagesChecking = true;
    holds.ensure();
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
    if (holds.host(id)) {
      this.setRun(id, { running: false, skipped: "updates are held on this host" });
      return;
    }
    if (!manager || !this.updates[id]?.report?.updates.some((u) => !u.held)) {
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

// Back/forward history (D-186): the tab is a step.
nav.track("fleet", "tab", () => fleet.tab, (v) => (fleet.tab = v as FleetTab));
