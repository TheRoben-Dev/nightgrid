/**
 * The .env vault (M10w): a stack's variables kept encrypted in Nightgrid,
 * shown masked, deployed to the host's `.env` after a preview.
 */
import { api, errorText, type EnvDiff, type EnvVar, type Stack } from "../api";
import { docker } from "./docker.svelte";

class EnvVaultStore {
  /** Stack whose vault is open. */
  stack = $state<Stack | null>(null);
  vars = $state<EnvVar[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  /** Values revealed this session, by id. Dropped when the dialog closes. */
  revealed = $state<Record<number, string>>({});
  busy = $state(false);
  /** Deploy preview, while the deploy step is showing. */
  preview = $state<EnvDiff | null>(null);
  previewing = $state(false);
  /** The result of the last deploy, offering "Up now". */
  deployed = $state<{ path: string; snapshot: boolean; snapshotError: string | null } | null>(null);
  private hostId: number | null = null;

  async open(s: Stack) {
    this.stack = s;
    this.hostId = docker.hostId;
    this.revealed = {};
    this.preview = null;
    this.deployed = null;
    this.error = null;
    await this.load();
  }

  close() {
    this.stack = null;
    this.vars = [];
    this.revealed = {};
    this.preview = null;
    this.deployed = null;
  }

  async load() {
    const s = this.stack;
    if (!s || this.hostId === null) return;
    this.loading = true;
    try {
      const list = await api.envVarsList(this.hostId, s.name);
      if (this.stack !== s) return;
      this.vars = list;
      this.error = null;
      // Plain variables aren't hidden: fetch their values right away.
      for (const v of list) if (!v.secret && !v.locked && !(v.id in this.revealed)) void this.reveal(v.id);
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  async reveal(id: number) {
    try {
      const value = await api.envVarReveal(id);
      this.revealed = { ...this.revealed, [id]: value };
    } catch (e) {
      this.error = errorText(e);
    }
  }

  hide(id: number) {
    const { [id]: _, ...rest } = this.revealed;
    this.revealed = rest;
  }

  /** Adds or changes a variable. `value: null` keeps the stored value. Returns an error text. */
  async save(oldName: string | null, name: string, value: string | null, secret: boolean): Promise<string | null> {
    const s = this.stack;
    if (!s || this.hostId === null) return "no stack";
    this.busy = true;
    try {
      const v = await api.envVarSet(this.hostId, s.name, oldName, { name, value, secret });
      if (value !== null && !secret) this.revealed = { ...this.revealed, [v.id]: value };
      else this.hide(v.id);
      this.preview = null;
      await this.load();
      return null;
    } catch (e) {
      return errorText(e);
    } finally {
      this.busy = false;
    }
  }

  async remove(id: number) {
    this.busy = true;
    try {
      await api.envVarDelete(id);
      this.hide(id);
      this.preview = null;
      await this.load();
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = false;
    }
  }

  async move(id: number, dir: -1 | 1) {
    const s = this.stack;
    if (!s || this.hostId === null) return;
    const ids = this.vars.map((v) => v.id);
    const i = ids.indexOf(id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    this.vars = ids.map((x) => this.vars.find((v) => v.id === x)!);
    try {
      await api.envVarsOrder(this.hostId, s.name, ids);
    } catch (e) {
      this.error = errorText(e);
      await this.load();
    }
  }

  /** Reads the host's `.env` into the vault. Returns the count, or null on error. */
  async importHost(): Promise<number | null> {
    const s = this.stack;
    if (!s || this.hostId === null) return null;
    this.busy = true;
    try {
      const n = await api.envImportHost(this.hostId, s);
      this.revealed = {};
      this.preview = null;
      await this.load();
      return n;
    } catch (e) {
      this.error = errorText(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  async loadPreview() {
    const s = this.stack;
    if (!s || this.hostId === null) return;
    this.previewing = true;
    this.deployed = null;
    try {
      this.preview = await api.envDeployPreview(this.hostId, s);
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.previewing = false;
    }
  }

  async deploy() {
    const s = this.stack;
    if (!s || this.hostId === null) return;
    this.busy = true;
    try {
      this.deployed = await api.envDeploy(this.hostId, s);
      this.preview = null;
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = false;
    }
  }
}

export const envVault = new EnvVaultStore();
