import { leaf, leaves, neighbour, removeLeaf, setRatio, splitLeaf, type Dir, type LayoutNode } from "../term/layout";
import { TermSession, type Origin } from "../term/session.svelte";
import { hosts } from "./hosts.svelte";
import { ui } from "./ui.svelte";

export interface TermTab {
  id: number;
  root: LayoutNode;
  /** Focused pane in this tab. */
  active: number;
}

class TermsStore {
  tabs = $state<TermTab[]>([]);
  activeTab = $state<number | null>(null);
  private sessions = new Map<number, TermSession>();
  private nextId = 1;

  session(pane: number): TermSession | undefined {
    return this.sessions.get(pane);
  }

  get current(): TermTab | undefined {
    return this.tabs.find((t) => t.id === this.activeTab);
  }

  /** Open panes and live SSH shells, for the status strip and top bar. */
  get stats(): { panes: number; ssh: number } {
    let panes = 0;
    let ssh = 0;
    for (const t of this.tabs) {
      for (const p of leaves(t.root)) {
        panes++;
        const s = this.sessions.get(p);
        if (s?.origin.kind === "ssh" && s.state === "connected") ssh++;
      }
    }
    return { panes, ssh };
  }

  get currentSession(): TermSession | undefined {
    const t = this.current;
    return t ? this.sessions.get(t.active) : undefined;
  }

  private newSession(origin: Origin): number {
    const pane = this.nextId++;
    const s = new TermSession(pane, origin);
    this.sessions.set(pane, s);
    void s.start();
    return pane;
  }

  private originFor(hostId: number | null): Origin {
    if (hostId === null) return { kind: "local" };
    return { kind: "ssh", hostId, name: hosts.byId(hostId)?.name ?? `host ${hostId}` };
  }

  private openTab(origin: Origin) {
    const pane = this.newSession(origin);
    const tab: TermTab = { id: this.nextId++, root: leaf(pane), active: pane };
    this.tabs = [...this.tabs, tab];
    this.activeTab = tab.id;
    ui.go("terminal");
  }

  openLocal() {
    this.openTab({ kind: "local" });
  }

  openHost(hostId: number) {
    this.openTab(this.originFor(hostId));
  }

  /** Opens a tab with a shell inside a container on `hostId`. */
  openContainer(hostId: number, container: { id: string; name: string }) {
    this.openTab({ ...this.originFor(hostId), container } as Origin);
  }

  /** Splits the focused pane, opening the same kind of session next to it. */
  split(dir: Dir) {
    const tab = this.current;
    if (!tab) return this.openLocal();
    const src = this.sessions.get(tab.active);
    const origin = src?.origin ?? { kind: "local" as const };
    const pane = this.newSession(origin);
    this.update(tab.id, { root: splitLeaf(tab.root, tab.active, pane, dir), active: pane });
  }

  /** Marks a pane active without moving keyboard focus. */
  markActive(tabId: number, pane: number) {
    const t = this.tabs.find((x) => x.id === tabId);
    if (t && t.active !== pane) this.update(tabId, { active: pane });
  }

  focusPane(tabId: number, pane: number) {
    this.update(tabId, { active: pane });
    this.sessions.get(pane)?.focus();
  }

  resize(tabId: number, path: string, ratio: number) {
    const tab = this.tabs.find((t) => t.id === tabId);
    if (tab) this.update(tabId, { root: setRatio(tab.root, path, ratio) });
  }

  closePane(pane: number) {
    const tab = this.tabs.find((t) => leaves(t.root).includes(pane));
    if (!tab) return;
    const next = neighbour(tab.root, pane);
    const root = removeLeaf(tab.root, pane);
    this.sessions.get(pane)?.dispose();
    this.sessions.delete(pane);
    if (!root) return this.dropTab(tab.id);
    this.update(tab.id, { root, active: next ?? leaves(root)[0] });
    if (next !== null) queueMicrotask(() => this.sessions.get(next)?.focus());
  }

  closeTab(tabId: number) {
    const tab = this.tabs.find((t) => t.id === tabId);
    if (!tab) return;
    for (const pane of leaves(tab.root)) {
      this.sessions.get(pane)?.dispose();
      this.sessions.delete(pane);
    }
    this.dropTab(tabId);
  }

  private dropTab(tabId: number) {
    const i = this.tabs.findIndex((t) => t.id === tabId);
    this.tabs = this.tabs.filter((t) => t.id !== tabId);
    if (this.activeTab === tabId) this.activeTab = this.tabs[Math.min(i, this.tabs.length - 1)]?.id ?? null;
  }

  cycle(delta: number) {
    if (!this.tabs.length) return;
    const i = this.tabs.findIndex((t) => t.id === this.activeTab);
    this.activeTab = this.tabs[(i + delta + this.tabs.length) % this.tabs.length].id;
  }

  /** Re-applies terminal settings (font, theme, scrollback) to every pane. */
  applySettings() {
    for (const s of this.sessions.values()) s.applySettings();
  }

  private update(tabId: number, patch: Partial<TermTab>) {
    this.tabs = this.tabs.map((t) => (t.id === tabId ? { ...t, ...patch } : t));
  }
}

export const terms = new TermsStore();
