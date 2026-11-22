/**
 * Profile sync between devices (D-205): what Settings → Sync shows, and
 * the reaction to `sync://event` (a device paired, pushed a profile here,
 * or automatic sync changed something here).
 */
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import { api, errorText, type FoundDevice, type ImportMode, type SecretCategory, type SyncInfo, type SyncJoined, type SyncMode, type SyncPeer, type SyncSettings } from "../api";
import { hosts } from "./hosts.svelte";
import { settings } from "./settings.svelte";

export const MODES: { id: SyncMode; label: string; what: string }[] = [
  { id: "manual", label: "MANUAL", what: "Push or pull a whole profile when you ask." },
  { id: "twoWay", label: "TWO-WAY", what: "Changes on either device go to the other within seconds. The latest edit of an item wins." },
  { id: "sendOnly", label: "SEND ONLY", what: "This device's changes go there automatically; changes made there don't come back." },
  { id: "receiveOnly", label: "RECEIVE ONLY", what: "Changes made there come here automatically; this device's changes stay here." },
];

export const SECRET_GROUPS: { id: SecretCategory; label: string; what: string }[] = [
  { id: "logins", label: "SSH logins", what: "saved SSH passwords, key passphrases and sudo passwords" },
  { id: "tokens", label: "Integration tokens", what: "integration API tokens and the alerts webhook" },
  { id: "vault", label: ".env vault keys", what: "the keys that unlock .env vault values (without them synced values can't be read here)" },
];

export const modeLabel = (m: SyncMode) => MODES.find((x) => x.id === m)?.label ?? m;

/** "online 2 min ago" style status of a paired device. */
export function peerState(p: SyncPeer, nowSecs: number): { tone: "ok" | "warn" | "bad" | ""; text: string } {
  if (p.lastError) return { tone: "bad", text: p.lastError };
  const seen = p.lastSeen ?? null;
  if (seen == null) return { tone: "", text: "not seen yet" };
  const ago = Math.max(0, nowSecs - seen);
  const words = ago < 60 ? "just now" : ago < 3600 ? `${Math.round(ago / 60)} min ago` : ago < 86_400 ? `${Math.round(ago / 3600)} h ago` : `${Math.round(ago / 86_400)} d ago`;
  return { tone: ago < 600 ? "ok" : "warn", text: `seen ${words}` };
}

/** Normalizes a typed pairing code to ABCDE-FGHJK (null while incomplete). */
export function formatCode(raw: string): string | null {
  const s = raw
    .toUpperCase()
    .replace(/[\s-]/g, "")
    .replace(/O/g, "0")
    .replace(/[IL]/g, "1");
  if (!/^[0-9A-HJKMNP-TV-Z]{10}$/.test(s)) return null;
  return `${s.slice(0, 5)}-${s.slice(5)}`;
}

class SyncStore {
  info = $state<SyncInfo | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);
  /** Nightgrid apps that answered the last LAN search (D-208). */
  found = $state<FoundDevice[]>([]);
  searching = $state(false);
  /** A device on the LAN asked this one for a pairing code: show it. */
  asked = $state<{ from: string; code: string; at: number } | null>(null);
  /** Last automatic change from another device, for a short note. */
  lastApplied = $state<{ from: string; count: number; at: number } | null>(null);
  private started = false;

  start() {
    if (this.started) return;
    this.started = true;
    void this.refresh();
    if (!isTauri()) return;
    void listen<{ kind: string; from?: string; code?: string; merged?: { applied: number; deleted: number; settings: boolean; tables: string[] } }>(
      "sync://event",
      (e) => {
        const p = e.payload;
        if (p.kind === "asked" && p.code) this.asked = { from: p.from ?? "", code: p.code, at: Date.now() };
        if (p.kind === "peers" && this.asked && this.info?.pairing == null) this.asked = null;
        if (p.kind === "applied" && p.merged) {
          this.lastApplied = { from: p.from ?? "", count: p.merged.applied + p.merged.deleted, at: Date.now() };
          if (p.merged.settings) void settings.load();
          if (p.merged.tables.some((t) => t === "hosts" || t === "host_groups" || t === "tags" || t === "host_tags")) void hosts.refresh();
        }
        void this.refresh();
      },
    );
  }

  async refresh() {
    try {
      this.info = await api.syncInfo();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async run<T>(f: () => Promise<T>): Promise<T | undefined> {
    this.busy = true;
    this.error = null;
    try {
      return await f();
    } catch (e) {
      this.error = errorText(e);
      return undefined;
    } finally {
      this.busy = false;
    }
  }

  async configure(change: Partial<SyncSettings>) {
    const cur = this.info?.settings ?? { enabled: false, port: 47920, deviceName: "", discoverable: false, passphrasePairing: false, keepSnapshots: 3 };
    const i = await this.run(() => api.syncConfigure({ ...cur, ...change }));
    if (i) this.info = i;
    return !!i;
  }

  /** Searches the LAN for other Nightgrid apps (quietly: errors don't show). */
  async discover() {
    if (this.searching) return this.found;
    this.searching = true;
    try {
      this.found = await api.syncDiscover();
    } catch {
      // A search that can't run (no network) just finds nothing.
      this.found = [];
    } finally {
      this.searching = false;
    }
    return this.found;
  }

  async askCode(d: FoundDevice) {
    return (await this.run(() => api.syncAskCode(d.addr, d.port))) !== undefined;
  }

  async joinFound(d: FoundDevice, how: { code?: string; passphrase?: string }, importMode: ImportMode | null, twoWay: boolean, secrets: SecretCategory[]): Promise<SyncJoined | undefined> {
    const r = await this.run(() => api.syncJoin(d.addr, d.port, how.code ?? null, how.passphrase ?? null, d.passphrase ? d.salt : null, importMode, twoWay, secrets));
    await this.refresh();
    if (r) this.found = this.found.map((f) => (f.id === d.id ? { ...f, paired: true } : f));
    return r;
  }

  async setPassphrase(passphrase: string | null) {
    const i = await this.run(() => api.syncPassphrase(passphrase));
    if (i) this.info = i;
    return !!i;
  }

  async pairStart(acceptChanges: boolean) {
    const i = await this.run(() => api.syncPairStart(acceptChanges));
    if (i) this.info = i;
  }

  async pairCancel() {
    const i = await this.run(() => api.syncPairCancel());
    if (i) this.info = i;
  }

  async join(addr: string, code: string, acceptChanges: boolean): Promise<SyncPeer | undefined> {
    const p = await this.run(() => api.syncPairJoin(addr, code, acceptChanges));
    await this.refresh();
    return p;
  }

  async setPeer(p: SyncPeer, change: Partial<Pick<SyncPeer, "name" | "mode" | "secrets" | "acceptChanges">>) {
    const n = { ...p, ...change };
    const r = await this.run(() => api.syncPeerSet(p.id, n.name, n.mode, n.secrets, n.acceptChanges));
    await this.refresh();
    return r;
  }

  async unpair(id: string) {
    await this.run(() => api.syncUnpair(id));
    await this.refresh();
  }

  async check(id: string) {
    await this.run(() => api.syncCheck(id));
    await this.refresh();
  }

  async syncNow(id: string) {
    const r = await this.run(() => api.syncNow(id));
    await this.refresh();
    return r;
  }
}

export const sync = new SyncStore();
