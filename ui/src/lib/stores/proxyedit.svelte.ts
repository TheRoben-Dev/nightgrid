/** Reverse proxy route editor (M10v): Caddyfiles and Nightgrid's Traefik routes. */
import { api, errorText, type CaddyEdit, type CaddyPreview, type CaddyTarget, type ProxyEditTargets, type TraefikRoute, type TraefikTarget } from "../api";
import { editSummary, targetKey } from "../proxyedit";
import { docker } from "./docker.svelte";
import { hosts } from "./hosts.svelte";
import { proxy } from "./proxy.svelte";
import { system } from "./system.svelte";

export type Pending = { target: CaddyTarget; edit: CaddyEdit; summary: string; preview: CaddyPreview };

class ProxyEditStore {
  targets = $state<ProxyEditTargets | null>(null);
  loading = $state(false);
  busy = $state(false);
  error = $state<string | null>(null);
  notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  selected = $state<string | null>(null);
  pending = $state<Pending | null>(null);

  get hostName(): string {
    return hosts.hosts.find((h) => h.id === docker.hostId)?.name ?? "this host";
  }

  async load() {
    const id = docker.hostId;
    if (id === null) return;
    this.loading = true;
    this.error = null;
    try {
      const t = await api.proxyEditTargets(id);
      if (docker.hostId !== id) return;
      this.targets = t;
      const keys = [...t.caddy.map((c) => targetKey("caddy", c.proxyName)), ...t.traefik.map((x) => targetKey("traefik", x.proxyName))];
      if (!this.selected || !keys.includes(this.selected)) this.selected = keys[0] ?? null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  get caddy(): CaddyTarget | null {
    return this.targets?.caddy.find((c) => targetKey("caddy", c.proxyName) === this.selected) ?? null;
  }
  get traefik(): TraefikTarget | null {
    return this.targets?.traefik.find((t) => targetKey("traefik", t.proxyName) === this.selected) ?? null;
  }

  /** Works out the new Caddyfile and its diff; nothing is written. */
  async propose(target: CaddyTarget, edit: CaddyEdit): Promise<boolean> {
    this.notice = null;
    try {
      const preview = await api.caddyEditPreview(target.text, edit);
      if (preview.text === target.text) {
        this.notice = { text: "Nothing changed.", tone: "ok" };
        return false;
      }
      this.pending = { target, edit, summary: editSummary(edit), preview };
      return true;
    } catch (e) {
      this.notice = { text: errorText(e), tone: "err" };
      return false;
    }
  }

  async saveCaddy(): Promise<boolean> {
    const p = this.pending;
    const id = docker.hostId;
    if (!p || id === null) return false;
    this.busy = true;
    try {
      const run = (sudo: Parameters<typeof api.caddySave>[5]) => api.caddySave(id, p.target.proxyName, p.preview.text, p.target.hash, p.summary, sudo);
      const r = p.target.proxyName === "host" ? await system.withSudo(`change ${p.target.path}`, run, this.hostName) : await run(null);
      this.notice = r.ok ? { text: `Caddy loaded it: ${p.summary}.`, tone: "ok" } : { text: r.message, tone: "err" };
      if (r.ok) this.pending = null;
      await this.load();
      if (r.ok) void proxy.load(true);
      return r.ok;
    } catch (e) {
      this.notice = { text: errorText(e), tone: "err" };
      return false;
    } finally {
      this.busy = false;
    }
  }

  async saveTraefik(target: TraefikTarget, routes: TraefikRoute[], summary: string): Promise<boolean> {
    const id = docker.hostId;
    if (id === null) return false;
    this.busy = true;
    this.notice = null;
    try {
      const t = await api.traefikSave(id, target.proxyName, routes, target.hash);
      if (this.targets) this.targets = { ...this.targets, traefik: this.targets.traefik.map((x) => (x.proxyName === t.proxyName ? t : x)) };
      this.notice = { text: `Saved: ${summary}. Traefik picks the file up by itself.`, tone: "ok" };
      void proxy.load(true);
      return true;
    } catch (e) {
      this.notice = { text: errorText(e), tone: "err" };
      return false;
    } finally {
      this.busy = false;
    }
  }
}

export const proxyEdit = new ProxyEditStore();
