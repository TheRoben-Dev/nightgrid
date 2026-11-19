/** The Security view (1.1): which tab is open and the posture data. */
import { api, errorText, type KeyHost, type ManualSecret, type PostureHost, type PostureRun, type SecretItem } from "../api";
import { nav } from "./nav.svelte";

export type SecurityTab = "posture" | "secrets";

class SecurityStore {
  tab = $state<SecurityTab>("posture");
  posture = $state<PostureHost[]>([]);
  loaded = $state(false);
  checking = $state<number[]>([]);
  error = $state<string | null>(null);
  /** Host whose checks are open. */
  open = $state<number | null>(null);
  history = $state<PostureRun[]>([]);

  async load() {
    try {
      this.posture = await api.postureList();
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async check(ids: number[]) {
    const todo = ids.filter((id) => !this.checking.includes(id));
    if (!todo.length) return;
    this.checking = [...this.checking, ...todo];
    this.error = null;
    try {
      await api.postureCheck(todo);
      await this.load();
      if (this.open !== null && todo.includes(this.open)) await this.loadHistory(this.open);
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.checking = this.checking.filter((id) => !todo.includes(id));
    }
  }

  async loadHistory(id: number) {
    try {
      this.history = await api.postureHistory(id);
    } catch {
      this.history = [];
    }
  }

  // ---- secrets
  secrets = $state<SecretItem[]>([]);
  secretsLoaded = $state(false);
  secretsError = $state<string | null>(null);
  /** Key whose detail is open. */
  secretOpen = $state<string | null>(null);
  /** Where an SSH key is authorized, by fingerprint. */
  keyHosts = $state<Record<string, KeyHost[] | "loading">>({});

  async loadSecrets() {
    try {
      this.secrets = await api.secretsList();
      this.secretsLoaded = true;
      this.secretsError = null;
    } catch (e) {
      this.secretsError = errorText(e);
    }
  }

  private async act(f: () => Promise<unknown>) {
    try {
      await f();
      await this.loadSecrets();
    } catch (e) {
      this.secretsError = errorText(e);
    }
  }

  markRotated = (key: string) => this.act(() => api.secretMarkRotated(key));
  setPolicy = (key: string, days: number | null) => this.act(() => api.secretPolicy(key, days));
  saveManual = (m: ManualSecret) => this.act(() => api.secretManualSave(m));
  deleteManual = (id: number) => this.act(() => api.secretManualDelete(id));

  async findKey(fingerprint: string) {
    this.keyHosts = { ...this.keyHosts, [fingerprint]: "loading" };
    try {
      this.keyHosts = { ...this.keyHosts, [fingerprint]: await api.secretKeyHosts(fingerprint) };
    } catch (e) {
      this.secretsError = errorText(e);
      const { [fingerprint]: _, ...rest } = this.keyHosts;
      this.keyHosts = rest;
    }
  }

  select(id: number | null) {
    this.open = id;
    this.history = [];
    if (id !== null) void this.loadHistory(id);
  }
}

export const security = new SecurityStore();

// Back/forward history (D-186): the tab is a step.
nav.track("security", "tab", () => security.tab, (v) => (security.tab = v as SecurityTab));
