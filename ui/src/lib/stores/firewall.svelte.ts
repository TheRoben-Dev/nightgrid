/** Firewall manager (M10v): System → FIREWALL on the current host. */
import { api, errorText, type ChangeCheck, type FirewallView, type FwChange } from "../api";
import { system } from "./system.svelte";

export interface Proposal {
  change: FwChange;
  check: ChangeCheck;
}

class FirewallStore {
  view = $state<FirewallView | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  /** A change waiting for the user to confirm it (dialog open). */
  proposal = $state<Proposal | null>(null);
  busy = $state(false);
  /** Result of the fresh-connection test after a change. */
  test = $state<{ state: "running" | "ok" | "failed"; text: string } | null>(null);
  /** Shown after the host rolled back by itself. */
  notice = $state<string | null>(null);
  private host: number | null = null;

  async load() {
    const id = system.hostId;
    if (id === null) return;
    if (this.host !== id) {
      this.view = null;
      this.test = null;
      this.notice = null;
    }
    this.host = id;
    this.loading = true;
    try {
      const v = await system.withSudo("read the firewall", (sudo) => api.firewallRead(id, sudo));
      if (this.host === id) {
        this.view = v;
        this.error = null;
      }
    } catch (e) {
      if (this.host === id) this.error = errorText(e);
    }
    this.loading = false;
  }

  /** Checks a change and opens the confirmation. */
  async propose(change: FwChange) {
    const id = system.hostId;
    if (id === null) return;
    try {
      this.proposal = { change, check: await api.firewallCheck(id, change) };
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Applies the open proposal; returns an error text or null. */
  async apply(secs: number, typed: string | null): Promise<string | null> {
    const id = system.hostId;
    const p = this.proposal;
    if (id === null || !p) return "nothing to apply";
    this.busy = true;
    try {
      const pending = await system.withSudo("change the firewall", (sudo) => api.firewallApply(id, p.change, secs, typed, sudo));
      this.proposal = null;
      this.notice = null;
      if (this.view) this.view = { ...this.view, pending };
      void this.runTest();
      void this.load();
      return null;
    } catch (e) {
      return errorText(e);
    } finally {
      this.busy = false;
    }
  }

  async runTest() {
    const id = system.hostId;
    if (id === null) return;
    this.test = { state: "running", text: "Opening a new SSH connection…" };
    try {
      const ms = await api.firewallTest(id);
      this.test = { state: "ok", text: `A new SSH connection works (${ms} ms).` };
    } catch (e) {
      this.test = { state: "failed", text: `A new SSH connection failed: ${errorText(e)}. Don't keep this change.` };
    }
  }

  async finish(keep: boolean) {
    const id = system.hostId;
    const p = this.view?.pending;
    if (id === null || !p) return;
    this.busy = true;
    try {
      await (keep ? api.firewallKeep(id, p.token) : api.firewallUndo(id, p.token));
      this.notice = keep ? `Kept: ${p.change}.` : `Undone: ${p.change}. The old rules are back.`;
      this.test = null;
      if (this.view) this.view = { ...this.view, pending: null };
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    }
    this.busy = false;
  }

  /** The countdown ran out: the host has put the old rules back. */
  expired() {
    const p = this.view?.pending;
    if (!p || !this.view) return;
    this.view = { ...this.view, pending: null };
    this.test = null;
    this.notice = `Not kept in time, so the host put the old rules back (${p.change}).`;
    setTimeout(() => void this.load(), 4000);
  }
}

export const firewall = new FirewallStore();
