/**
 * File browser state: two panes (each local or a host), open editor tabs
 * and the transfer queue mirror.
 */
import {
  api,
  errorText,
  filesError,
  type Bookmark,
  type ConflictPolicy,
  type ConnectError,
  type Entry,
  type JobView,
  type PromptKind,
  type Side,
  type Stamp,
} from "../api";
import type { EditorHandle } from "../files/editor";
import { isHidden, langFor, type LangKey } from "../files/util";
import { hosts } from "./hosts.svelte";

export const sameSide = (a: Side, b: Side) => a.kind === b.kind && (a.kind === "local" || a.id === (b as { id: number }).id);

export function sideLabel(side: Side): string {
  if (side.kind === "local") return "LOCAL";
  return hosts.hosts.find((h) => h.id === side.id)?.name ?? `host ${side.id}`;
}

export class Pane {
  side = $state<Side>({ kind: "local" });
  path = $state("");
  parent = $state<string | null>(null);
  entries = $state<Entry[]>([]);
  selected = $state<string[]>([]);
  anchor = $state<string | null>(null);
  cursor = $state<string | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  prompt = $state<ConnectError | null>(null);
  showHidden = $state(false);
  filter = $state("");
  bookmarks = $state<Bookmark[]>([]);
  /** Path history for Back. */
  private history: string[] = [];
  private seq = 0;

  constructor(readonly id: "left" | "right") {}

  readonly visible = $derived(
    this.entries.filter(
      (e) =>
        (this.showHidden || !isHidden(e.name)) &&
        (!this.filter || e.name.toLowerCase().includes(this.filter.toLowerCase())),
    ),
  );
  readonly selectedEntries = $derived(this.entries.filter((e) => this.selected.includes(e.path)));
  readonly hostId = $derived(this.side.kind === "host" ? this.side.id : null);
  readonly label = $derived(sideLabel(this.side));

  /** Switches the pane to a side and opens `path` (home by default). */
  async open(side: Side, path = "~") {
    this.side = side;
    this.entries = [];
    this.history = [];
    this.path = "";
    this.prompt = null;
    void this.loadBookmarks();
    await this.load(path, false);
  }

  async load(path: string, remember = true) {
    const seq = ++this.seq;
    this.loading = true;
    this.error = null;
    try {
      const l = await api.fsList(this.side, path);
      if (seq !== this.seq) return;
      if (remember && this.path && this.path !== l.path) this.history.push(this.path);
      const sameDir = l.path === this.path;
      this.path = l.path;
      this.parent = l.parent;
      this.entries = l.entries;
      this.prompt = null;
      if (!sameDir) {
        this.selected = [];
        this.anchor = null;
        this.cursor = null;
        this.filter = "";
      } else {
        const paths = l.entries.map((e) => e.path);
        this.selected = this.selected.filter((p) => paths.includes(p));
      }
    } catch (e) {
      if (seq !== this.seq) return;
      const { connect } = filesError(e);
      if (connect && connect.kind !== "network" && connect.kind !== "other") this.prompt = connect;
      else this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  refresh = () => this.load(this.path || "~", false);
  up = () => this.parent && this.load(this.parent);
  back() {
    const p = this.history.pop();
    if (p) void this.load(p, false);
  }

  async loadBookmarks() {
    try {
      this.bookmarks = await api.bookmarksList(this.hostId);
    } catch {
      this.bookmarks = [];
    }
  }

  readonly bookmarked = $derived(this.bookmarks.find((b) => b.path === this.path) ?? null);

  async toggleBookmark() {
    if (!this.path) return;
    const b = this.bookmarked;
    if (b) await api.bookmarkDelete(b.id);
    else await api.bookmarkAdd(this.hostId, this.path, null);
    await this.loadBookmarks();
  }

  async acceptHostKey() {
    const p = this.prompt;
    if (!p || (p.kind !== "hostKeyUnknown" && p.kind !== "hostKeyChanged")) return;
    await api.sshTrustHostKey(p.host, p.port, p.key.openssh, p.kind === "hostKeyChanged");
    await this.load(this.path || "~", false);
  }

  async provideSecret(secret: string, remember: boolean) {
    const p = this.prompt;
    if (!p || (p.kind !== "passwordRequired" && p.kind !== "passphraseRequired")) return;
    const kind: PromptKind = p.kind === "passwordRequired" ? "password" : "passphrase";
    await api.sshProvideSecret(p.target, kind, secret, remember);
    await this.load(this.path || "~", false);
  }

  cancelPrompt() {
    this.prompt = null;
    this.error = "Connection cancelled.";
  }
}

let nextDoc = 1;

export class EditorDoc {
  readonly id = nextDoc++;
  content = "";
  stamp = $state<Stamp | null>(null);
  dirty = $state(false);
  saving = $state(false);
  loading = $state(true);
  error = $state<string | null>(null);
  /** Set when the file changed on disk since it was opened. */
  conflict = $state(false);
  lastSave = $state<{ atomic: boolean; at: number } | null>(null);
  editor: EditorHandle | null = null;
  readonly lang: LangKey;

  constructor(
    readonly side: Side,
    public path: string,
    readonly name: string,
  ) {
    this.lang = langFor(name);
  }

  get where(): string {
    return sideLabel(this.side);
  }

  async load() {
    this.loading = true;
    this.error = null;
    try {
      const f = await api.fsReadText(this.side, this.path);
      this.path = f.path;
      this.content = f.content;
      this.stamp = { size: f.stat.size, mtime: f.stat.mtime };
      this.dirty = false;
      this.conflict = false;
      this.editor?.reset(f.content);
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  /** Saves; with `force` overwrites even if the file changed on disk. */
  async save(force = false) {
    if (!this.editor || this.saving) return;
    this.saving = true;
    this.error = null;
    const text = this.editor.text();
    try {
      const r = await api.fsSaveText(this.side, this.path, text, force ? null : this.stamp);
      this.stamp = { size: r.stat.size, mtime: r.stat.mtime };
      this.content = text;
      this.dirty = this.editor.text() !== text;
      this.conflict = false;
      this.lastSave = { atomic: r.atomic, at: Date.now() };
    } catch (e) {
      const { fs } = filesError(e);
      if (fs?.kind === "changed") this.conflict = true;
      else this.error = errorText(e);
    } finally {
      this.saving = false;
    }
  }
}

export type FilesTab = "browser" | number;

class FilesStore {
  panes = [new Pane("left"), new Pane("right")];
  active = $state<0 | 1>(0);
  docs = $state<EditorDoc[]>([]);
  tab = $state<FilesTab>("browser");
  jobs = $state<JobView[]>([]);
  queueOpen = $state(true);
  private started = false;

  readonly activePane = $derived(this.panes[this.active]);
  readonly activeTransfers = $derived(this.jobs.filter((j) => j.state === "running" || j.state === "queued").length);
  readonly conflicts = $derived(this.jobs.filter((j) => j.state === "conflict").length);
  readonly currentDoc = $derived(typeof this.tab === "number" ? (this.docs.find((d) => d.id === this.tab) ?? null) : null);

  other(p: Pane): Pane {
    return p === this.panes[0] ? this.panes[1] : this.panes[0];
  }

  /** First visit: left pane local home, right pane the first favourite host (or local). */
  start() {
    if (this.started) return;
    this.started = true;
    // Not reactive state: only used to spot transitions.
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const seen = new Map<number, string>();
    void api.transfersSubscribe((jobs) => {
      // Refresh panes showing a destination when a job lands there.
      for (const j of jobs) {
        if (j.state === "done" && seen.get(j.id) !== "done") this.refreshWhere(j.dst, j.dstDir);
        seen.set(j.id, j.state);
      }
      this.jobs = jobs;
    });
    void this.panes[0].open({ kind: "local" });
    const fav = hosts.hosts.find((h) => h.favorite) ?? hosts.hosts[0];
    void this.panes[1].open(fav ? { kind: "host", id: fav.id } : { kind: "local" });
  }

  /** Opens a host in the right pane (from the palette / host list). */
  async browseHost(hostId: number) {
    this.tab = "browser";
    this.start();
    this.active = 1;
    await this.panes[1].open({ kind: "host", id: hostId });
  }

  /** Copies `entries` from pane `from` into the other pane's directory (or `dstDir`). */
  async copy(from: Pane, entries: Entry[], dstDir?: string, policy: ConflictPolicy = "ask") {
    const to = this.other(from);
    const dir = dstDir ?? to.path;
    if (!entries.length || !dir) return;
    await api.transfersAdd(
      entries.map((e) => ({ src: from.side, srcPath: e.path, dst: to.side, dstDir: dir, policy })),
    );
    this.queueOpen = true;
  }

  /** Copies within/between panes given explicit sides (drag & drop onto a folder). */
  async copyTo(src: Side, paths: string[], dst: Side, dstDir: string) {
    if (!paths.length) return;
    await api.transfersAdd(paths.map((p) => ({ src, srcPath: p, dst, dstDir, policy: "ask" as ConflictPolicy })));
    this.queueOpen = true;
  }

  openDoc(side: Side, e: Entry) {
    const existing = this.docs.find((d) => sameSide(d.side, side) && d.path === e.path);
    if (existing) {
      this.tab = existing.id;
      return;
    }
    const doc = new EditorDoc(side, e.path, e.name);
    this.docs = [...this.docs, doc];
    this.tab = doc.id;
    void doc.load();
  }

  closeDoc(id: number) {
    const i = this.docs.findIndex((d) => d.id === id);
    if (i < 0) return;
    this.docs[i].editor?.view.destroy();
    this.docs = this.docs.filter((d) => d.id !== id);
    if (this.tab === id) this.tab = this.docs[Math.min(i, this.docs.length - 1)]?.id ?? "browser";
  }

  /** Refreshes panes showing `side`/`dir` after a transfer lands there. */
  refreshWhere(side: Side, dir: string) {
    for (const p of this.panes) if (sameSide(p.side, side) && p.path === dir) void p.refresh();
  }

  resolve(id: number, policy: ConflictPolicy, all: boolean) {
    return api.transferResolve(id, policy, all);
  }
}

export const files = new FilesStore();
