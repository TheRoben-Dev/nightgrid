import { leaf, leaves, neighbour, removeLeaf, setRatio, splitLeaf, type Dir, type LayoutNode } from "../term/layout";
import { TermSession, type Origin } from "../term/session.svelte";
import type { VmKind } from "../api";
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
    s.reveal = () => {
      const tab = this.tabs.find((t) => leaves(t.root).includes(pane));
      if (!tab) return;
      this.activeTab = tab.id;
      this.update(tab.id, { active: pane });
      ui.go("terminal");
      queueMicrotask(() => s.focus());
    };
    this.sessions.set(pane, s);
    void s.start();
    return pane;
  }

  private originFor(hostId: number | null): Origin {
    if (hostId === null) return { kind: "local" };
    return {
      kind: "ssh",
      hostId,
      name: hosts.byId(hostId)?.name ?? `host ${hostId}`,
    };
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

  /** The newest plain shell (not a container or console) on `hostId` that is still usable. */
  shellFor(hostId: number): TermSession | undefined {
    const open = [...this.sessions.values()].filter(
      (s) => s.origin.kind === "ssh" && s.origin.hostId === hostId && !s.origin.container && !s.origin.vm,
    );
    const live = open.filter((s) => s.state !== "exited" && s.state !== "failed");
    return (live.length ? live : open).at(-1);
  }

  /** Jumps to the host's open terminal, or opens one (`fresh`: always a new tab). */
  goToHost(hostId: number, fresh = false) {
    const s = fresh ? undefined : this.shellFor(hostId);
    if (s?.reveal) s.reveal();
    else this.openHost(hostId);
  }

  /** Opens a tab with a shell inside a container on `hostId`. */
  openContainer(hostId: number, container: { id: string; name: string }) {
    this.openTab({ ...this.originFor(hostId), container } as Origin);
  }

  /** Opens a tab on a VM / container guest's console on `hostId`. */
  openVmConsole(hostId: number, vm: { kind: VmKind; id: string; name: string; asRoot: boolean }) {
    this.openTab({ ...this.originFor(hostId), vm } as Origin);
  }

  /**
   * Types `text` at the prompt of a shell on `hostId` (null = whatever shell
   * is focused, else a new local one): the focused pane if it fits, else
   * another open shell on that host, else a new tab once it has connected.
   * Nothing runs until the user presses Enter. Returns the pane's title.
   */
  send(text: string, hostId: number | null): string {
    const fits = (s: TermSession | undefined): s is TermSession =>
      !!s && s.state !== "exited" && s.state !== "failed" && (hostId === null || (s.origin.kind === "ssh" && s.origin.hostId === hostId && !s.origin.container && !s.origin.vm));
    let target = this.currentSession;
    if (!fits(target)) target = [...this.sessions.values()].find(fits);
    if (target) {
      target.reveal?.();
      target.paste(text);
      return target.title;
    }
    if (hostId === null) this.openLocal();
    else this.openHost(hostId);
    const fresh = this.currentSession;
    if (!fresh) return "";
    const started = Date.now();
    const tick = setInterval(() => {
      if (fresh.state === "connected") {
        clearInterval(tick);
        fresh.paste(text);
      } else if (Date.now() - started > 60_000 || fresh.state === "failed" || fresh.state === "exited") clearInterval(tick);
    }, 250);
    return fresh.title;
  }

  /**
   * Splits the focused pane. `target` picks the new pane's shell (null: this
   * computer, a number: that host); left out, the same kind of session.
   */
  split(dir: Dir, target?: number | null) {
    const tab = this.current;
    if (!tab) return target ? this.openHost(target) : this.openLocal();
    const src = this.sessions.get(tab.active);
    const origin = target === undefined ? (src?.origin ?? { kind: "local" as const }) : this.originFor(target);
    const pane = this.newSession(origin);
    this.update(tab.id, {
      root: splitLeaf(tab.root, tab.active, pane, dir),
      active: pane,
    });
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
