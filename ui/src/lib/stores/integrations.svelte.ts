import {
  api,
  errorText,
  type AmpAction,
  type Integration,
  type IntegrationInput,
  type IntegrationKind,
  type IntegrationStatus,
  type IntegrationView,
} from "../api";

/** Seconds between status reads while the Services view is open. */
const POLL_SECS = 30;

export const KIND_LABEL: Record<IntegrationKind, string> = {
  plex: "PLEX",
  amp: "AMP",
  kuma: "UPTIME KUMA",
  portainer: "PORTAINER",
};

export const KIND_HELP: Record<IntegrationKind, string> = {
  plex: "Now playing, transcodes and server version. Needs a Plex token (X-Plex-Token).",
  amp: "Game server instances with players; start, stop and restart. Needs an AMP login with permission to manage instances.",
  kuma: "Monitors from a public status page. No API key needed: create a status page in Kuma and enter its slug.",
  portainer: "Adds an \"Open in Portainer\" link to containers on the linked host.",
};

export interface Entry {
  status: IntegrationStatus | null;
  error: string | null;
  loading: boolean;
  /** Unix ms of the last read. */
  at: number;
}

export interface PendingAmp {
  id: number;
  instance: string;
  friendly: string;
  action: AmpAction;
}

export const emptyIntegrationInput = (kind: IntegrationKind = "plex"): IntegrationInput => ({
  kind,
  name: "",
  baseUrl: "",
  username: "",
  slug: "",
  endpointId: kind === "portainer" ? 1 : 0,
  hostId: null,
  allowUntrusted: false,
});

export const toIntegrationInput = (i: Integration): IntegrationInput => ({
  kind: i.kind,
  name: i.name,
  baseUrl: i.baseUrl,
  username: i.username,
  slug: i.slug,
  endpointId: i.endpointId,
  hostId: i.hostId,
  allowUntrusted: i.allowUntrusted,
});

/** One line for the card header: "2 streams · 1 transcoding", "3/4 up", ... */
export function summary(s: IntegrationStatus | null): string {
  if (!s) return "";
  switch (s.kind) {
    case "plex": {
      const n = s.streams.length;
      if (!n) return "Nothing playing";
      return `${n} stream${n === 1 ? "" : "s"}${s.transcodes ? ` · ${s.transcodes} transcoding` : ""}`;
    }
    case "amp": {
      const up = s.instances.filter((i) => i.running).length;
      const players = s.instances.reduce((a, i) => a + (i.users ?? 0), 0);
      return `${up}/${s.instances.length} running · ${players} player${players === 1 ? "" : "s"}`;
    }
    case "kuma":
      return `${s.up}/${s.monitors.length} up${s.down ? ` · ${s.down} down` : ""}`;
    case "portainer":
      return s.version ? `v${s.version}` : "Reachable";
  }
}

export type Tone = "ok" | "warn" | "bad" | "idle";
export function toneOf(e: Entry | undefined): Tone {
  if (!e || (!e.status && !e.error)) return "idle";
  if (e.error) return "bad";
  const s = e.status!;
  if (s.kind === "kuma" && s.down) return "warn";
  return "ok";
}

/** Portainer container page, or null when it isn't linked to `hostId`. */
export function portainerUrl(list: IntegrationView[], hostId: number | null, containerId: string): string | null {
  const p = list.find((v) => v.integration.kind === "portainer" && v.integration.hostId === hostId);
  if (!p || hostId === null) return null;
  const i = p.integration;
  return `${i.baseUrl}/#!/${i.endpointId}/docker/containers/${containerId}`;
}

class IntegrationsStore {
  list = $state<IntegrationView[]>([]);
  entries = $state<Record<number, Entry>>({});
  loaded = $state(false);
  /** null = closed, 0 = new, id = edit. */
  editing = $state<number | null>(null);
  pendingAmp = $state<PendingAmp | null>(null);
  notice = $state<{ tone: "ok" | "err"; text: string } | null>(null);
  private timer: ReturnType<typeof setInterval> | null = null;
  private watchers = 0;

  byId(id: number): IntegrationView | undefined {
    return this.list.find((v) => v.integration.id === id);
  }

  async load() {
    try {
      this.list = await api.integrationsList();
    } catch (e) {
      this.notice = { tone: "err", text: errorText(e) };
    } finally {
      this.loaded = true;
    }
  }

  /** Starts polling while a view shows integrations; returns the stop function. */
  watch(): () => void {
    this.watchers++;
    if (this.watchers === 1) {
      void this.load().then(() => this.refreshAll());
      this.timer = setInterval(() => {
        if (typeof document === "undefined" || document.visibilityState !== "hidden") void this.refreshAll();
      }, POLL_SECS * 1000);
    }
    return () => {
      this.watchers = Math.max(0, this.watchers - 1);
      if (!this.watchers && this.timer) {
        clearInterval(this.timer);
        this.timer = null;
      }
    };
  }

  async refreshAll() {
    await Promise.all(this.list.map((v) => this.refresh(v.integration.id)));
  }

  async refresh(id: number) {
    const prev = this.entries[id];
    this.entries[id] = { status: prev?.status ?? null, error: prev?.error ?? null, loading: true, at: prev?.at ?? 0 };
    try {
      const status = await api.integrationStatus(id);
      this.entries[id] = { status, error: null, loading: false, at: Date.now() };
    } catch (e) {
      this.entries[id] = { status: null, error: errorText(e), loading: false, at: Date.now() };
    }
  }

  async save(id: number, input: IntegrationInput, secret: string | null): Promise<IntegrationView> {
    const v = id ? await api.integrationUpdate(id, input, secret) : await api.integrationCreate(input, secret);
    await this.load();
    void this.refresh(v.integration.id);
    return v;
  }

  async remove(id: number) {
    await api.integrationDelete(id);
    delete this.entries[id];
    await this.load();
  }

  /** Runs an AMP action (stop/restart come here after the confirmation). */
  async amp(p: PendingAmp) {
    this.pendingAmp = null;
    try {
      await api.ampInstanceAction(p.id, p.instance, p.action);
      this.notice = { tone: "ok", text: `${p.action.toUpperCase()} sent to ${p.friendly || p.instance}` };
    } catch (e) {
      this.notice = { tone: "err", text: errorText(e) };
    }
    setTimeout(() => void this.refresh(p.id), 1500);
  }

  /** Start runs at once; stop and restart ask first. */
  request(p: PendingAmp) {
    if (p.action === "start") void this.amp(p);
    else this.pendingAmp = p;
  }
}

export const integrations = new IntegrationsStore();
