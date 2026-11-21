/**
 * Browser-only stand-in for profile sync (D-205), so Settings → Sync can be
 * exercised in `pnpm dev`, tests and screenshots. Nothing leaves the page.
 */
import type {
  BackupImported,
  ImportMode,
  SecretCategory,
  SyncInfo_Serialize as SyncInfo,
  SyncMode,
  SyncPeer_Serialize as SyncPeer,
  SyncPushed,
  SyncRun,
  SyncSettings_Serialize as SyncSettings,
} from "./bindings";

const now = () => Date.now() / 1000;
const wait = (ms = 250) => new Promise((r) => setTimeout(r, ms));

let settings: SyncSettings = { enabled: false, port: 47920, deviceName: "" };
let pairing: { code: string; until: number; acceptChanges: boolean } | null = null;
let peers: SyncPeer[] = [
  {
    id: "3f9a0c11d2e4b577",
    name: "ROG Ally",
    publicKey: "",
    addrs: ["192.168.1.40", "ally.tail1234.ts.net"],
    port: 47920,
    mode: "twoWay",
    secrets: ["tokens"],
    acceptChanges: true,
    appVersion: "1.22.0",
    pairedAt: now() - 86_400 * 3,
    lastSync: now() - 40,
    lastSeen: now() - 40,
    lastError: null,
  },
];

function info(): SyncInfo {
  if (pairing && pairing.until < now()) pairing = null;
  return {
    settings,
    deviceName: settings.deviceName || "Robens-MacBook",
    fingerprint: "a1b2-c3d4-e5f6-0718",
    listening: settings.enabled,
    addresses: settings.enabled ? ["192.168.1.20", "100.101.102.103", "mac.tail1234.ts.net"] : [],
    error: null,
    peers,
    pairing: pairing ? { code: pairing.code, expiresIn: Math.round(pairing.until - now()), acceptChanges: pairing.acceptChanges } : null,
    receivedFrom: null,
    tailnet: true,
  };
}

function find(id: string): SyncPeer {
  const p = peers.find((x) => x.id === id);
  if (!p) throw new Error("That device isn't paired any more.");
  return p;
}

export const mockSync = {
  async info(): Promise<SyncInfo> {
    return info();
  },
  async configure(s: SyncSettings): Promise<SyncInfo> {
    if (s.port < 1024) throw new Error("Use a port from 1024 up.");
    settings = { ...s, deviceName: s.deviceName.trim() };
    if (!s.enabled) pairing = null;
    return info();
  },
  async pairStart(acceptChanges: boolean): Promise<SyncInfo> {
    if (!settings.enabled) throw new Error('Turn on "Let paired devices reach this one" first.');
    pairing = { code: "7KQ2M-XW4PD", until: now() + 300, acceptChanges };
    return info();
  },
  async pairCancel(): Promise<SyncInfo> {
    pairing = null;
    return info();
  },
  async pairJoin(addr: string, code: string, acceptChanges: boolean): Promise<SyncPeer> {
    await wait(600);
    if (!addr.trim()) throw new Error("Enter the other device's address.");
    if (code.replace(/[\s-]/g, "").length !== 10) throw new Error("A pairing code is 10 letters and digits, like ABCDE-12345.");
    const p: SyncPeer = {
      id: Math.random().toString(16).slice(2, 18).padEnd(16, "0"),
      name: addr.split(".")[0] || "Nightgrid",
      publicKey: "",
      addrs: [addr.trim()],
      port: 47920,
      mode: "manual",
      secrets: [],
      acceptChanges,
      appVersion: "1.22.0",
      pairedAt: now(),
      lastSync: null,
      lastSeen: now(),
      lastError: null,
    };
    peers = [...peers, p];
    return p;
  },
  async peerSet(id: string, name: string, mode: SyncMode, secrets: SecretCategory[], acceptChanges: boolean): Promise<SyncPeer> {
    const p = { ...find(id), name: name.trim() || "Nightgrid", mode, secrets, acceptChanges };
    peers = peers.map((x) => (x.id === id ? p : x));
    return p;
  },
  async unpair(id: string): Promise<null> {
    find(id);
    peers = peers.filter((x) => x.id !== id);
    return null;
  },
  async check(id: string): Promise<SyncPeer> {
    await wait();
    const p = { ...find(id), lastSeen: now(), lastError: null };
    peers = peers.map((x) => (x.id === id ? p : x));
    return p;
  },
  async push(id: string, mode: ImportMode): Promise<SyncPushed> {
    find(id);
    await wait(500);
    return mode === "replace" ? { added: 31, skipped: 0, secrets: 1 } : { added: 4, skipped: 27, secrets: 0 };
  },
  async pull(id: string, mode: ImportMode): Promise<BackupImported> {
    find(id);
    await wait(500);
    return mode === "replace"
      ? { added: 29, skipped: 0, secrets: 1, settings: true, secretsError: null }
      : { added: 3, skipped: 26, secrets: 0, settings: false, secretsError: null };
  },
  async now(id: string): Promise<SyncRun> {
    const p = find(id);
    await wait(400);
    peers = peers.map((x) => (x.id === id ? { ...p, lastSync: now(), lastSeen: now() } : x));
    return { merged: { applied: 2, deleted: 0, skipped: 0, settings: false, tables: ["hosts"] }, sent: true };
  },
};
