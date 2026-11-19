import { dnsSummary } from "../dnsfilter";
import {
  api,
  errorText,
  type AmpAction,
  type AppAction,
  type DnsAction,
  type Integration,
  type IntegrationInput,
  type IntegrationKind,
  type IntegrationStatus,
  type IntegrationView,
} from "../api";

/** Seconds between status reads while the Services view is open. */
const POLL_SECS = 30;

export type KindGroup = "Media" | "Media automation" | "Downloads" | "Files & photos" | "Storage" | "Network" | "Monitoring" | "Cameras" | "Docker" | "Games";
export const KIND_GROUPS: KindGroup[] = ["Media", "Media automation", "Downloads", "Files & photos", "Storage", "Network", "Monitoring", "Cameras", "Docker", "Games"];

export interface KindMeta {
  /** Upper-case badge text. */
  label: string;
  /** Default name, also the picker button text. */
  name: string;
  group: KindGroup;
  help: string;
  placeholder: string;
  /** null = the kind needs no secret. */
  secret: { label: string; hint: string; optional?: boolean } | null;
  /** The login field, when the kind uses one. */
  username?: { label: string; required: boolean; placeholder?: string };
  /** An extra text field kept in `slug` (UniFi site). */
  extra?: { label: string; hint: string; placeholder: string };
}

const apiKey = (where: string) => ({ label: "API key", hint: `Kept in the OS keychain only. ${where}` });
const arr = (name: string, what: string, port: number): KindMeta => ({
  label: name.toUpperCase(),
  name,
  group: "Media automation",
  help: `${what} Needs the API key from ${name}'s Settings → General.`,
  placeholder: `http://server01:${port}`,
  secret: apiKey(`${name}: Settings → General → API Key.`),
});

/** Everything the UI knows about each kind. A new kind needs a line here and a Rust module. */
export const KIND_META: Record<IntegrationKind, KindMeta> = {
  plex: {
    label: "PLEX",
    name: "Plex",
    group: "Media",
    help: "Now playing, transcodes and server version. Needs a Plex token (X-Plex-Token).",
    placeholder: "http://server01:32400",
    secret: { label: "Plex token", hint: "Kept in the OS keychain only. Find it via a media item's … → Get Info → View XML (X-Plex-Token in the URL)." },
  },
  jellyfin: {
    label: "JELLYFIN",
    name: "Jellyfin",
    group: "Media",
    help: "Now playing, transcodes, library counts and a library scan button. Needs an API key (Dashboard → API Keys).",
    placeholder: "http://server01:8096",
    secret: apiKey("Jellyfin: Dashboard → API Keys → +."),
  },
  emby: {
    label: "EMBY",
    name: "Emby",
    group: "Media",
    help: "Now playing, transcodes, library counts and a library scan button. Needs an API key (Settings → API Keys).",
    placeholder: "http://server01:8096",
    secret: apiKey("Emby: Settings → Advanced → API Keys."),
  },
  tautulli: {
    label: "TAUTULLI",
    name: "Tautulli",
    group: "Media",
    help: "Plex activity with quality, transcode decisions and bandwidth. Needs the API key from Settings → Web Interface.",
    placeholder: "http://server01:8181",
    secret: apiKey("Tautulli: Settings → Web Interface → API Key."),
  },
  sonarr: arr("Sonarr", "Download queue with progress, missing episodes, health warnings; RSS sync and search missing.", 8989),
  radarr: arr("Radarr", "Download queue with progress, missing movies, health warnings; RSS sync and search missing.", 7878),
  lidarr: arr("Lidarr", "Download queue with progress, missing albums, health warnings; RSS sync and search missing.", 8686),
  prowlarr: arr("Prowlarr", "Indexers, which ones are failing, health warnings; test all indexers.", 9696),
  bazarr: arr("Bazarr", "Missing subtitles for episodes and movies, throttled providers and health issues.", 6767),
  overseerr: {
    label: "OVERSEERR",
    name: "Overseerr",
    group: "Media automation",
    help: "Request counts and pending requests you can approve or decline. Needs the API key from Settings → General.",
    placeholder: "http://server01:5055",
    secret: apiKey("Overseerr: Settings → General → API Key."),
  },
  jellyseerr: {
    label: "JELLYSEERR",
    name: "Jellyseerr",
    group: "Media automation",
    help: "Request counts and pending requests you can approve or decline. Needs the API key from Settings → General.",
    placeholder: "http://server01:5055",
    secret: apiKey("Jellyseerr: Settings → General → API Key."),
  },
  qbittorrent: {
    label: "QBITTORRENT",
    name: "qBittorrent",
    group: "Downloads",
    help: "Speeds, active and failed torrents, pause or resume one or all. Uses the Web UI login; leave both empty if the Web UI skips login for your network.",
    placeholder: "http://server01:8080",
    secret: { label: "Password", hint: "Kept in the OS keychain only. The Web UI password (Tools → Options → Web UI).", optional: true },
    username: { label: "Login name", required: false },
  },
  transmission: {
    label: "TRANSMISSION",
    name: "Transmission",
    group: "Downloads",
    help: "Speeds, torrents with progress and errors, start or stop one or all. The login is only needed when RPC authentication is on.",
    placeholder: "http://server01:9091",
    secret: { label: "Password", hint: "Kept in the OS keychain only. Leave empty if RPC authentication is off.", optional: true },
    username: { label: "Login name", required: false },
  },
  sabnzbd: {
    label: "SABNZBD",
    name: "SABnzbd",
    group: "Downloads",
    help: "Speed, the queue with progress, free disk, recent failures; pause, resume or delete. Needs the API key from Config → General.",
    placeholder: "http://server01:8080",
    secret: apiKey("SABnzbd: Config → General → Security → API Key."),
  },
  nzbget: {
    label: "NZBGET",
    name: "NZBGet",
    group: "Downloads",
    help: "Speed, the queue with progress, free disk; pause, resume or delete. Uses the control login from Settings → Security.",
    placeholder: "http://server01:6789",
    secret: { label: "Control password", hint: "Kept in the OS keychain only. Settings → Security → ControlPassword." },
    username: { label: "Control login", required: true },
  },
  immich: {
    label: "IMMICH",
    name: "Immich",
    group: "Files & photos",
    help: "Photos, videos, storage used and background jobs (running, waiting, failed). Library size and jobs need an admin's API key.",
    placeholder: "http://server01:2283",
    secret: apiKey("Immich: Account Settings → API Keys → New API Key."),
  },
  nextcloud: {
    label: "NEXTCLOUD",
    name: "Nextcloud",
    group: "Files & photos",
    help: "Users, files, active users, shares, free space, load and pending app updates (serverinfo). Needs an admin login and an app password.",
    placeholder: "https://cloud.example.lan",
    secret: { label: "App password", hint: "Kept in the OS keychain only. Personal settings → Security → Devices & sessions → Create new app password." },
    username: { label: "Admin login name", required: true },
  },
  syncthing: {
    label: "SYNCTHING",
    name: "Syncthing",
    group: "Files & photos",
    help: "Folders with sync state, devices online and errors; rescan a folder or all, pause or resume syncing. Needs the API key.",
    placeholder: "http://server01:8384",
    secret: apiKey("Syncthing: Actions → Settings → General → API Key."),
  },
  gotify: {
    label: "GOTIFY",
    name: "Gotify",
    group: "Monitoring",
    help: "Health, apps and the latest messages with priority; delete one or all. Needs a client token (Clients → Create client).",
    placeholder: "http://server01:8070",
    secret: { label: "Client token", hint: "Kept in the OS keychain only. A client token, not an app token." },
  },
  frigate: {
    label: "FRIGATE",
    name: "Frigate",
    group: "Cameras",
    help: "Cameras with frame rates (offline ones in red), detector speed, recording storage and the latest detections. Port 5000 needs no login; port 8971 needs a Frigate login.",
    placeholder: "http://server01:5000",
    secret: { label: "Password", hint: "Kept in the OS keychain only. Only for the authenticated port (8971).", optional: true },
    username: { label: "Login name", required: false },
  },
  truenas: {
    label: "TRUENAS",
    name: "TrueNAS",
    group: "Storage",
    help: "Pools with health and how full they are, TrueNAS's own alerts, uptime, load and memory. Needs an API key (user menu → API Keys).",
    placeholder: "https://nas.example.lan",
    secret: apiKey("TrueNAS: top-right user menu → API Keys → Add."),
  },
  synology: {
    label: "SYNOLOGY",
    name: "Synology DSM",
    group: "Storage",
    help: "Volumes with status and how full they are, disks with SMART problems, CPU, memory and temperature. Needs an administrator account without 2-step verification (make a separate one for Nightgrid).",
    placeholder: "https://nas.example.lan:5001",
    secret: { label: "Password", hint: "Kept in the OS keychain only." },
    username: { label: "Login name", required: true },
  },
  unraid: {
    label: "UNRAID",
    name: "Unraid",
    group: "Storage",
    help: "Array state and capacity, every disk with status, temperature and errors, containers and VMs running. Unraid 7.2+ (or the Unraid Connect plugin) with an API key.",
    placeholder: "http://tower.local",
    secret: apiKey("Unraid: Settings → Management Access → API Keys."),
  },
  pbs: {
    label: "PROXMOX BACKUP",
    name: "Proxmox Backup Server",
    group: "Storage",
    help: "Datastores with usage and when they'll be full, and tasks that failed in the last day. Needs an API token with Datastore.Audit and Sys.Audit.",
    placeholder: "https://backup.example.lan:8007",
    secret: { label: "Token secret", hint: "Kept in the OS keychain only. Configuration → Access Control → API Token → Add." },
    username: { label: "Token id", required: true, placeholder: "root@pam!nightgrid" },
  },
  opnsense: {
    label: "OPNSENSE",
    name: "OPNsense",
    group: "Network",
    help: "Gateways with delay and loss, stopped services (start them from the card) and firmware updates. Needs an API key and secret (System → Access → Users → API keys).",
    placeholder: "https://192.168.1.1",
    secret: { label: "API secret", hint: "Kept in the OS keychain only. From the API key file OPNsense downloads." },
    username: { label: "API key", required: true },
  },
  unifi: {
    label: "UNIFI",
    name: "UniFi Network",
    group: "Network",
    help: "Devices (offline first) with model and address, clients connected, and restart for a device. UniFi Network 9.0+ with an API key (Settings → Control Plane → Integrations).",
    placeholder: "https://192.168.1.1",
    secret: apiKey("UniFi: Network → Settings → Control Plane → Integrations → Create API Key."),
    extra: { label: "Site", hint: "Leave empty for the first site.", placeholder: "Default" },
  },
  npm: {
    label: "NGINX PROXY MANAGER",
    name: "Nginx Proxy Manager",
    group: "Network",
    help: "Proxy hosts with where they point and nginx errors, certificates expiring within two weeks; enable or disable a host. Uses the NPM login (email and password).",
    placeholder: "http://server01:81",
    secret: { label: "Password", hint: "Kept in the OS keychain only." },
    username: { label: "Email", required: true },
  },
  grafana: {
    label: "GRAFANA",
    name: "Grafana",
    group: "Monitoring",
    help: "Health, dashboards, and alert rules that are firing or pending. Needs a service account token (Administration → Service accounts) with the Viewer role.",
    placeholder: "http://server01:3000",
    secret: { label: "Service account token", hint: "Kept in the OS keychain only. Starts with glsa_." },
  },
  prometheus: {
    label: "PROMETHEUS",
    name: "Prometheus",
    group: "Monitoring",
    help: "Scrape targets up and down with their errors, and alerts firing or pending. Login only if Prometheus sits behind basic auth; a password with no login is sent as a bearer token.",
    placeholder: "http://server01:9090",
    secret: { label: "Password or token", hint: "Kept in the OS keychain only.", optional: true },
    username: { label: "Login name", required: false },
  },
  amp: {
    label: "AMP",
    name: "AMP",
    group: "Games",
    help: "Game server instances with players; start, stop and restart. Needs an AMP login with permission to manage instances.",
    placeholder: "https://amp.example.lan",
    secret: { label: "Password", hint: "Kept in the OS keychain only." },
    username: { label: "Login name", required: true },
  },
  kuma: {
    label: "UPTIME KUMA",
    name: "Uptime Kuma",
    group: "Monitoring",
    help: "Monitors from a public status page. No API key needed: create a status page in Kuma and enter its slug.",
    placeholder: "http://server01:3001",
    secret: null,
  },
  portainer: {
    label: "PORTAINER",
    name: "Portainer",
    group: "Docker",
    help: 'Adds an "Open in Portainer" link to containers on the linked host.',
    placeholder: "https://server01:9443",
    secret: null,
  },
  pihole: {
    label: "PI-HOLE",
    name: "Pi-hole",
    group: "Network",
    help: "Queries, blocked share and top blocked domains; pause blocking; block and allow lists; local DNS records. Pi-hole v6: use the web password or an app password (Settings → Web interface / API).",
    placeholder: "http://192.168.1.2",
    secret: { label: "Password", hint: "Kept in the OS keychain only." },
  },
  adguard: {
    label: "ADGUARD HOME",
    name: "AdGuard Home",
    group: "Network",
    help: "Queries, blocked share and top blocked domains; pause protection; block and allow lists; DNS rewrites. Uses the AdGuard Home login.",
    placeholder: "http://192.168.1.2:3000",
    secret: { label: "Password", hint: "Kept in the OS keychain only." },
    username: { label: "Login name", required: true },
  },
};

export const KIND_LABEL = Object.fromEntries(Object.entries(KIND_META).map(([k, m]) => [k, m.label])) as Record<IntegrationKind, string>;
export const KIND_HELP = Object.fromEntries(Object.entries(KIND_META).map(([k, m]) => [k, m.help])) as Record<IntegrationKind, string>;

/** Kinds grouped for the picker, in KIND_GROUPS order. */
export function kindsByGroup(filter = ""): { group: KindGroup; kinds: IntegrationKind[] }[] {
  const f = filter.trim().toLowerCase();
  const all = (Object.keys(KIND_META) as IntegrationKind[]).filter((k) => {
    const m = KIND_META[k];
    return !f || m.name.toLowerCase().includes(f) || m.group.toLowerCase().includes(f) || m.help.toLowerCase().includes(f);
  });
  return KIND_GROUPS.map((group) => ({ group, kinds: all.filter((k) => KIND_META[k].group === group) })).filter((g) => g.kinds.length);
}

export interface Entry {
  status: IntegrationStatus | null;
  error: string | null;
  loading: boolean;
  /** Unix ms of the last read. */
  at: number;
}

/** An action from an app card waiting for its confirmation. */
export interface PendingAction {
  id: number;
  target: string;
  action: AppAction;
  /** The row or integration it applies to, for the notice. */
  subject: string;
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
    case "pihole":
    case "adguard":
      return dnsSummary(s);
    case "app":
      return s.summary;
  }
}

export type Tone = "ok" | "warn" | "bad" | "idle";
export function toneOf(e: Entry | undefined): Tone {
  if (!e || (!e.status && !e.error)) return "idle";
  if (e.error) return "bad";
  const s = e.status!;
  if (s.kind === "kuma" && s.down) return "warn";
  if ((s.kind === "pihole" || s.kind === "adguard") && !s.blocking) return "warn";
  if (s.kind === "app") return s.health === "bad" ? "bad" : s.health === "warn" ? "warn" : "ok";
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
  pendingAction = $state<PendingAction | null>(null);
  /** "id:target:action" while it runs. */
  acting = $state<string | null>(null);
  /** The DNS filter whose dashboard is open. */
  dnsOpen = $state<number | null>(null);
  dnsBusy = $state(false);
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

  /** Card and row actions: those with a confirm question ask first. */
  requestAction(p: PendingAction) {
    if (p.action.confirm) this.pendingAction = p;
    else void this.runAction(p);
  }

  async runAction(p: PendingAction) {
    this.pendingAction = null;
    this.acting = `${p.id}:${p.target}:${p.action.id}`;
    try {
      await api.integrationAction(p.id, p.target, p.action.id);
      this.notice = { tone: "ok", text: `${p.action.label}: sent${p.subject ? ` (${p.subject})` : ""}` };
    } catch (e) {
      this.notice = { tone: "err", text: errorText(e) };
    } finally {
      this.acting = null;
    }
    setTimeout(() => void this.refresh(p.id), 1200);
  }

  /** Runs a DNS filter change and reads the status again. */
  async dns(id: number, action: DnsAction, done: string): Promise<boolean> {
    this.dnsBusy = true;
    try {
      await api.dnsFilterAction(id, action);
      this.notice = { tone: "ok", text: done };
      await this.refresh(id);
      return true;
    } catch (e) {
      this.notice = { tone: "err", text: errorText(e) };
      return false;
    } finally {
      this.dnsBusy = false;
    }
  }
}

export const integrations = new IntegrationsStore();
