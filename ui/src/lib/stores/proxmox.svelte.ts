/**
 * Proxmox VE (M10r): one host's view of its cluster through `pvesh`, read as
 * root (the sudo prompt asks when needed). Power actions start Proxmox tasks;
 * the task's log is followed until it ends, then the view reloads.
 */
import { api, errorText, type PveOverview, type PveTaskDetail } from "../api";
import { ACTION_WORD, type PveAction, type PveGuest, type PveTask } from "../proxmox";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

export type PveTab = "summary" | "guests" | "storage" | "tasks";
const HOST_KEY = "nightgrid.proxmox.host";

function remembered(): number | null {
  try {
    const v = Number(localStorage.getItem(HOST_KEY));
    return Number.isFinite(v) && v > 0 ? v : null;
  } catch {
    return null;
  }
}

export interface OpenTask {
  node: string;
  upid: string;
  title: string;
  detail: PveTaskDetail | null;
  error: string | null;
}

class ProxmoxStore {
  hostId = $state<number | null>(remembered());
  overview = $state<PveOverview | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  tab = $state<PveTab>("summary");
  query = $state("");
  node = $state<string | null>(null);
  pending = $state<{ guest: PveGuest; action: PveAction } | null>(null);
  busy = $state<number[]>([]);
  notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  task = $state<OpenTask | null>(null);
  private seq = 0;
  private follow: ReturnType<typeof setTimeout> | null = null;

  get host() {
    return hosts.hosts.find((h) => h.id === this.hostId) ?? null;
  }

  pick(id: number | null) {
    if (id === this.hostId) return;
    this.hostId = id;
    this.overview = null;
    this.error = null;
    this.notice = null;
    this.node = null;
    this.closeTask();
    try {
      if (id) localStorage.setItem(HOST_KEY, String(id));
    } catch {
      /* per-viewer convenience only */
    }
    if (id) void this.load();
  }

  private sudo<T>(reason: string, f: (s: Parameters<typeof api.pveOverview>[1]) => Promise<T>): Promise<T> {
    return system.withSudo(reason, f, this.host?.name);
  }

  async load() {
    const id = this.hostId;
    if (id === null) return;
    const seq = ++this.seq;
    this.loading = true;
    try {
      const o = await this.sudo("read Proxmox", (s) => api.pveOverview(id, s));
      if (seq !== this.seq) return;
      this.overview = o;
      this.error = null;
    } catch (e) {
      if (seq === this.seq) this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  /** Destructive actions wait for confirmation (rule 5); start and resume run at once. */
  request(guest: PveGuest, action: PveAction) {
    if (action === "start" || action === "resume") void this.act(guest, action);
    else this.pending = { guest, action };
  }

  async act(guest: PveGuest, action: PveAction) {
    const id = this.hostId;
    if (id === null) return;
    this.pending = null;
    this.busy = [...this.busy, guest.vmid];
    const what = `${ACTION_WORD[action]} ${guest.name || guest.vmid}`;
    try {
      const upid = await this.sudo(what.toLowerCase(), (s) => api.pveGuestAction(id, guest.node, guest.kind, guest.vmid, action, s));
      this.notice = { text: `${what}: started.`, tone: "ok" };
      void this.openTask(guest.node, upid, what);
    } catch (e) {
      this.notice = { text: `${what}: ${errorText(e)}`, tone: "err" };
    } finally {
      this.busy = this.busy.filter((v) => v !== guest.vmid);
    }
  }

  /** Shows a task's log and follows it while it runs. */
  async openTask(node: string, upid: string, title: string) {
    this.closeTask();
    this.task = { node, upid, title, detail: null, error: null };
    await this.poll();
  }

  openListed(t: PveTask, title: string) {
    void this.openTask(t.node, t.upid, title);
  }

  private async poll() {
    const t = this.task;
    const id = this.hostId;
    if (!t || id === null) return;
    try {
      const d = await this.sudo("read a Proxmox task", (s) => api.pveTask(id, t.node, t.upid, s));
      if (this.task?.upid !== t.upid) return;
      this.task = { ...this.task, detail: d, error: null };
      if (d.running) this.follow = setTimeout(() => void this.poll(), 1500);
      else void this.load();
    } catch (e) {
      if (this.task?.upid === t.upid) this.task = { ...this.task, error: errorText(e) };
    }
  }

  closeTask() {
    if (this.follow) clearTimeout(this.follow);
    this.follow = null;
    this.task = null;
  }
}

export const proxmox = new ProxmoxStore();
