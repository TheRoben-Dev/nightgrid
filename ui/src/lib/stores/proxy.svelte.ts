/** Reverse proxy routes on the Docker host (M10g), read on demand. */
import { api, errorText, type ProxyReport, type ProxyRoute } from "../api";
import { docker } from "./docker.svelte";

export const PROXY_LABEL = { caddy: "Caddy", traefik: "Traefik", npm: "Nginx Proxy Manager" } as const;

export type CertTone = "ok" | "warn" | "bad" | "none";

/** Days until the route's certificate expires (negative = expired). */
export function certDays(r: ProxyRoute, now: number): number | null {
  const t = r.cert?.notAfter;
  return t == null ? null : Math.floor((t - now) / 86400);
}

export function certTone(r: ProxyRoute, now: number, warnDays = 14): CertTone {
  if (!r.tls) return "none";
  if (r.cert?.error) return "bad";
  const d = certDays(r, now);
  if (d === null) return "none";
  if (d < 0) return "bad";
  return d <= warnDays ? "warn" : "ok";
}

class ProxyStore {
  report = $state<ProxyReport | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  private hostId: number | null = null;

  /** Loads once per host; `force` reads again. */
  async load(force = false) {
    const id = docker.hostId;
    if (id === null) return;
    if (!force && id === this.hostId && (this.report || this.loading)) return;
    if (id !== this.hostId) this.report = null;
    this.hostId = id;
    this.loading = true;
    try {
      const r = await api.dockerProxyRoutes(id);
      if (docker.hostId !== id) return;
      this.report = r;
      this.error = null;
    } catch (e) {
      if (docker.hostId === id) this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  get visible(): ProxyRoute[] {
    const q = docker.filter.trim().toLowerCase();
    const all = this.report?.routes ?? [];
    return q ? all.filter((r) => `${r.domains.join(" ")} ${r.upstream} ${r.container ?? ""}`.toLowerCase().includes(q)) : all;
  }
}

export const proxy = new ProxyStore();
