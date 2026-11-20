import { sound } from "./sound.svelte";
import {
  api,
  dockerError,
  errorText,
  type ConnectError,
  type ContainerStats,
  type ContainerView,
  type DockerAction,
  type DockerError,
  type EngineInfo,
  type PromptKind,
} from "../api";
import { hosts } from "./hosts.svelte";
import { nav } from "./nav.svelte";
import { system } from "./system.svelte";

export interface ContainerGroup {
  /** Compose project name, or "" for containers outside compose. */
  key: string;
  label: string;
  dir: string | null;
  containers: ContainerView[];
  running: number;
}

const POLL_MS = 5000;

export const actionLabel = (a: DockerAction): string =>
  a.kind === "remove" ? (a.force ? "Force remove" : "Remove") : a.kind[0].toUpperCase() + a.kind.slice(1);

/** Needs a confirmation naming the targets (security rule 5). */
export const isDestructive = (a: DockerAction): boolean => ["stop", "restart", "kill", "remove"].includes(a.kind);

export const isUp = (c: ContainerView): boolean => c.state === "running" || c.state === "restarting";

class DockerStore {
  hostId = $state<number | null>(null);
  /** Containers list or compose stacks. */
  tab = $state<"containers" | "stacks" | "routes">("containers");
  info = $state<EngineInfo | null>(null);
  containers = $state<ContainerView[]>([]);
  stats = $state<Record<string, ContainerStats>>({});
  loading = $state(false);
  loaded = $state(false);
  error = $state<string | null>(null);
  /** Docker-level problem with a fix the user can apply (docker group etc.). */
  problem = $state<DockerError | null>(null);
  /** The NO ACCESS fix is running. */
  fixing = $state(false);
  /** SSH prompt to answer before connecting (host key, password). */
  prompt = $state<ConnectError | null>(null);
  filter = $state("");
  showStopped = $state(true);
  selected = $state<string[]>([]);
  collapsed = $state<string[]>([]);
  detailId = $state<string | null>(null);
  /** Containers with an action in flight. */
  busy = $state<string[]>([]);
  /** Outcome of the last action, shown in the toolbar. */
  notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  /** Action waiting for confirmation, with the exact targets. */
  pending = $state<{ action: DockerAction; targets: ContainerView[] } | null>(null);

  private timer: ReturnType<typeof setInterval> | null = null;
  private seq = 0;
  private statsInFlight = false;

  get host() {
    return this.hostId === null ? undefined : hosts.byId(this.hostId);
  }

  get detail(): ContainerView | undefined {
    return this.containers.find((c) => c.id === this.detailId);
  }

  get visible(): ContainerView[] {
    const q = this.filter.trim().toLowerCase();
    return this.containers.filter(
      (c) =>
        (this.showStopped || isUp(c)) &&
        (!q || [c.name, c.image, c.project ?? "", c.service ?? ""].some((s) => s.toLowerCase().includes(q))),
    );
  }

  /** Compose projects A–Z, then containers outside compose. */
  get groups(): ContainerGroup[] {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- local, rebuilt on each read
    const by = new Map<string, ContainerGroup>();
    for (const c of this.visible) {
      const key = c.project ?? "";
      let g = by.get(key);
      if (!g) {
        g = { key, label: key || "standalone", dir: c.composeDir, containers: [], running: 0 };
        by.set(key, g);
      }
      g.containers.push(c);
      if (isUp(c)) g.running++;
    }
    return [...by.values()].sort((a, b) => (a.key === "") !== (b.key === "") ? (a.key === "" ? 1 : -1) : a.key.localeCompare(b.key));
  }

  get selectedViews(): ContainerView[] {
    return this.containers.filter((c) => this.selected.includes(c.id));
  }

  get running(): number {
    return this.containers.filter(isUp).length;
  }

  /** Shows `hostId` (remembered as this view's pick). */
  select(hostId: number | null) {
    if (hostId === this.hostId) return;
    if (hostId !== null) hosts.remember("docker", hostId);
    this.hostId = hostId;
    this.info = null;
    this.containers = [];
    this.stats = {};
    this.selected = [];
    this.detailId = null;
    this.loaded = false;
    this.notice = null;
    void this.refresh();
  }

  /** Polls while the Docker view is on screen. */
  start(): () => void {
    const first = this.hostId === null ? hosts.preferred("docker") : undefined;
    if (first) this.select(first.id);
    else void this.refresh();
    this.timer = setInterval(() => void this.refresh(true), POLL_MS);
    return () => {
      if (this.timer) clearInterval(this.timer);
      this.timer = null;
    };
  }

  async refresh(background = false) {
    const id = this.hostId;
    if (id === null || this.prompt || (background && (this.problem || this.error))) return;
    const seq = ++this.seq;
    if (!background) this.loading = true;
    try {
      const list = await api.dockerContainers(id);
      if (seq !== this.seq || id !== this.hostId) return;
      this.containers = list;
      this.error = null;
      this.problem = null;
      this.loaded = true;
      const ids = new Set(list.map((c) => c.id));
      this.selected = this.selected.filter((s) => ids.has(s));
      if (!this.info) void api.dockerInfo(id).then((i) => id === this.hostId && (this.info = i)).catch(() => {});
      void this.refreshStats();
    } catch (e) {
      if (seq !== this.seq || id !== this.hostId) return;
      const { connect, docker } = dockerError(e);
      if (connect && connect.kind !== "network" && connect.kind !== "other") this.prompt = connect;
      else if (docker && (docker.kind === "permissionDenied" || docker.kind === "notInstalled")) this.problem = docker;
      else this.error = errorText(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  private async refreshStats() {
    const id = this.hostId;
    const ids = this.containers.filter((c) => c.state === "running").map((c) => c.id);
    if (id === null || this.statsInFlight || !ids.length) return;
    this.statsInFlight = true;
    try {
      const list = await api.dockerStats(id, ids);
      if (id !== this.hostId) return;
      const next: Record<string, ContainerStats> = {};
      for (const s of list) next[s.id] = s;
      this.stats = next;
    } catch {
      // Stats are best effort; the list shows the real state.
    } finally {
      this.statsInFlight = false;
    }
  }

  /** Retries after the user fixed something (e.g. joined the docker group). */
  async retry() {
    this.problem = null;
    this.error = null;
    if (this.hostId !== null) await api.dockerForget(this.hostId);
    await this.refresh();
  }

  /**
   * Runs the NO ACCESS card's steps as root on the host (asks for the sudo
   * password), then reconnects and retries.
   */
  async fixAccess() {
    const p = this.problem;
    const id = this.hostId;
    if (!p || p.kind !== "permissionDenied" || id === null || this.fixing) return;
    const name = hosts.hosts.find((h) => h.id === id)?.name;
    this.fixing = true;
    try {
      await system.withSudo(`give ${p.user} access to Docker`, (sudo) => api.dockerFixAccess(id, p.steps, p.relogin, sudo), name);
      this.notice = { text: `${p.user} can use Docker on ${name ?? "the host"} now.`, tone: "ok" };
      await this.retry();
    } catch (e) {
      if (errorText(e) !== "Cancelled.") this.notice = { text: `Couldn't fix Docker access: ${errorText(e)}`, tone: "err" };
    } finally {
      this.fixing = false;
    }
  }

  toggle(id: string) {
    this.selected = this.selected.includes(id) ? this.selected.filter((s) => s !== id) : [...this.selected, id];
  }

  toggleGroup(g: ContainerGroup) {
    const ids = g.containers.map((c) => c.id);
    const all = ids.every((i) => this.selected.includes(i));
    this.selected = all ? this.selected.filter((s) => !ids.includes(s)) : [...new Set([...this.selected, ...ids])];
  }

  toggleCollapsed(key: string) {
    this.collapsed = this.collapsed.includes(key) ? this.collapsed.filter((k) => k !== key) : [...this.collapsed, key];
  }

  /** Runs now, or asks first for destructive actions. */
  request(action: DockerAction, targets: ContainerView[]) {
    if (!targets.length) return;
    if (isDestructive(action)) this.pending = { action, targets };
    else void this.run(action, targets);
  }

  async run(action: DockerAction, targets: ContainerView[]) {
    const id = this.hostId;
    if (id === null) return;
    this.pending = null;
    const ids = targets.map((t) => t.id);
    this.busy = [...this.busy, ...ids];
    this.notice = null;
    try {
      const results = await api.dockerAction(id, ids, action);
      const failed = results.filter((r) => r.error);
      const name = (cid: string) => targets.find((t) => t.id === cid)?.name ?? cid;
      if (failed.length) {
        this.notice = { tone: "err", text: failed.map((f) => `${name(f.id)}: ${f.error}`).join(" · ") };
        sound.play("fail");
      } else {
        sound.play("ok");
        const what = targets.length === 1 ? targets[0].name : `${targets.length} containers`;
        this.notice = { tone: "ok", text: `${actionLabel(action)}: ${what} done` };
      }
      if (action.kind === "remove") {
        this.selected = this.selected.filter((s) => !ids.includes(s) || failed.some((f) => f.id === s));
        if (this.detailId && ids.includes(this.detailId)) this.detailId = null;
      }
    } catch (e) {
      this.notice = { tone: "err", text: errorText(e) };
      sound.play("fail");
    } finally {
      this.busy = this.busy.filter((b) => !ids.includes(b));
      await this.refresh(true);
    }
  }

  async acceptHostKey() {
    const p = this.prompt;
    if (!p || (p.kind !== "hostKeyUnknown" && p.kind !== "hostKeyChanged")) return;
    await api.sshTrustHostKey(p.host, p.port, p.key.openssh, p.kind === "hostKeyChanged");
    this.prompt = null;
    await this.refresh();
  }

  async provideSecret(secret: string, remember: boolean) {
    const p = this.prompt;
    if (!p || (p.kind !== "passwordRequired" && p.kind !== "passphraseRequired")) return;
    const kind: PromptKind = p.kind === "passwordRequired" ? "password" : "passphrase";
    await api.sshProvideSecret(p.target, kind, secret, remember);
    this.prompt = null;
    await this.refresh();
  }

  cancelPrompt() {
    this.prompt = null;
    this.error = "Connection cancelled.";
  }
}

export const docker = new DockerStore();

// Back/forward history (D-186): the tab and the host are steps.
nav.track("docker", "tab", () => docker.tab, (v) => (docker.tab = v as DockerStore["tab"]));
nav.track("docker", "host", () => docker.hostId, (v) => typeof v === "number" && hosts.byId(v) && docker.select(v), true);
