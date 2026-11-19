/**
 * Network map (M10i). Nothing is scanned until the user clicks: discovery runs
 * from the node selected on the map (a host or this computer), port audits
 * reuse System → Security.
 */
import { sound } from "./sound.svelte";
import { api, errorText, systemError, type DeviceLabel, type Discovery, type LayoutEntry, type NetDevice, type NetFinding, type NetWatchRun, type SavedNetwork, type ToolResult } from "../api";
import { buildGraph, type NetGraph } from "../netmap/graph";
import { overlayGraph, type Overlay } from "../netmap/networks";
import { buildTopology, canAttach, entryFor, expectedEntries, type Topology } from "../netmap/layout";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

class NetmapStore {
  sweep = $state(false);
  /** Routed subnets to sweep from the scanner, comma or space separated. */
  ranges = $state("");
  /** Every device any scan has seen (the inventory). */
  inventory = $state<NetDevice[]>([]);
  inventoryError = $state<string | null>(null);
  result = $state<Discovery | null>(null);
  /** Where the current result was scanned from. */
  resultHost = $state<number | "local" | null>(null);
  scannedAt = $state<number | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  selected = $state<string | null>(null);
  auditing = $state<number[]>([]);
  /** The user's names, kinds, tags and notes for devices that aren't hosts. */
  labels = $state<DeviceLabel[]>([]);
  labelsLoaded = false;
  labelError = $state<string | null>(null);

  readonly labelMap: ReadonlyMap<string, DeviceLabel> = $derived(new Map(this.labels.map((l) => [l.key, l])));

  /** Saved networks (D-154), most recently seen first. */
  networks = $state<SavedNetwork[]>([]);
  networksError = $state<string | null>(null);
  /** Saved networks drawn on the map beside the scan on show (ids; kept across restarts). */
  overlay = $state<number[]>(readOverlay());
  /** Their last scans, by network id. */
  overlayScans = $state<Record<number, Discovery>>({});
  /** The result on show is a saved network's last scan, not a scan run now. */
  fromSaved = $state(false);

  readonly overlays: Overlay[] = $derived(
    this.overlay.flatMap((id) => {
      const net = this.networks.find((n) => n.id === id);
      const disc = this.overlayScans[id];
      return net && disc ? [{ net, disc }] : [];
    }),
  );
  readonly graph: NetGraph = $derived(
    overlayGraph(
      buildGraph(hosts.hosts, system.portMap, this.result, this.resultHost, this.labelMap),
      this.result,
      hosts.hosts,
      system.portMap,
      this.overlays,
      this.labelMap,
    ),
  );
  /** The map as the user arranged it (positions, hand-set parents, expected picture). */
  layout = $state<LayoutEntry[]>([]);
  layoutLoaded = false;
  layoutError = $state<string | null>(null);
  /** Graph (topology) or list (the columns). */
  mode = $state<"graph" | "list">(readMode());
  readonly layoutMap: ReadonlyMap<string, LayoutEntry> = $derived(new Map(this.layout.map((e) => [e.key, e])));
  readonly topo: Topology = $derived(
    buildTopology({ graph: this.graph, disc: this.result, layout: this.layout, inventory: this.inventory, labels: this.labelMap }),
  );
  readonly current = $derived(this.topo.nodes.find((n) => n.id === this.selected)?.node ?? null);

  /**
   * Whether a map node can run a discovery: this computer, or a saved host
   * that answers (pinged or connected over SSH). Other devices can't.
   */
  canScan(id: string | null): boolean {
    if (id === "local") return true;
    const hid = id?.startsWith("h") ? Number(id.slice(1)) : NaN;
    if (!hosts.byId(hid)) return false;
    const p = hosts.pings[hid];
    return !p || p.connected || p.state !== "offline";
  }

  /** The scanner: the selected node when it can scan, else nothing (no default). */
  get from(): number | "local" | null {
    const id = this.selected;
    if (!this.canScan(id)) return null;
    return id === "local" ? "local" : Number(id!.slice(1));
  }

  async scan() {
    const id = this.from;
    if (this.loading || id === null) return;
    this.loading = true;
    this.error = null;
    try {
      const ranges = this.rangeList;
      this.result = id === "local" ? await api.netmapDiscoverLocal(this.sweep, ranges) : await api.netmapDiscover(id, this.sweep, ranges);
      this.resultHost = id;
      this.scannedAt = Date.now();
      this.fromSaved = false;
      void this.loadInventory();
      void this.loadNetworks();
      sound.play("taskDone");
    } catch (e) {
      sound.play("taskFail");
      const { connect } = systemError(e);
      this.error =
        connect && connect.kind !== "network" && connect.kind !== "other"
          ? "This host needs a login or a host key decision first. Open a terminal on it, then scan again."
          : errorText(e);
    } finally {
      this.loading = false;
    }
  }

  async loadNetworks() {
    try {
      this.networks = await api.netmapNetworks();
      this.networksError = null;
      const ids = new Set(this.networks.map((n) => n.id));
      if (this.overlay.some((id) => !ids.has(id))) this.setOverlay(this.overlay.filter((id) => ids.has(id)));
      // A network's kept scan changes when it is scanned again: reload the overlaid ones.
      for (const id of this.overlay) void this.loadOverlayScan(id);
    } catch (e) {
      this.networksError = errorText(e);
    }
  }

  private async loadOverlayScan(id: number) {
    try {
      const d = await api.netmapNetworkScan(id);
      const next = { ...this.overlayScans };
      if (d) next[id] = d;
      else delete next[id];
      this.overlayScans = next;
    } catch (e) {
      this.networksError = errorText(e);
    }
  }

  private setOverlay(ids: number[]) {
    this.overlay = ids;
    try {
      localStorage.setItem("nightgrid.netmap.overlay", JSON.stringify(ids));
    } catch {
      /* private mode: just this session */
    }
  }

  /** Draws a saved network on the map beside the scan on show, or takes it off. */
  async toggleOverlay(id: number, on: boolean) {
    if (on && !this.overlay.includes(id)) {
      this.setOverlay([...this.overlay, id]);
      await this.loadOverlayScan(id);
    } else if (!on) this.setOverlay(this.overlay.filter((x) => x !== id));
  }

  /** Shows a saved network's last scan as the main map (no scan runs). */
  async showNetwork(id: number) {
    const net = this.networks.find((n) => n.id === id);
    if (!net || this.loading) return;
    try {
      const d = await api.netmapNetworkScan(id);
      if (!d) {
        this.networksError = `No scan of ${net.name} is kept yet: scan it once from a machine on it.`;
        return;
      }
      this.result = d;
      this.resultHost = net.origin === "local" ? "local" : hosts.byId(Number(net.origin)) ? Number(net.origin) : null;
      this.scannedAt = (net.snapshotAt ?? net.lastSeen ?? 0) * 1000;
      this.fromSaved = true;
      this.networksError = null;
      this.security = { running: false, findings: null, error: null, at: 0 };
      if (this.overlay.includes(id)) this.setOverlay(this.overlay.filter((x) => x !== id));
    } catch (e) {
      this.networksError = errorText(e);
    }
  }

  async renameNetwork(id: number, name: string): Promise<boolean> {
    try {
      const n = await api.netmapNetworkRename(id, name);
      this.networks = this.networks.map((x) => (x.id === id ? n : x));
      if (this.result?.network?.id === id) this.result = { ...this.result, network: { ...this.result.network, name: n.name } };
      if (this.overlayScans[id]) this.overlayScans = { ...this.overlayScans, [id]: { ...this.overlayScans[id], network: { id, name: n.name, isNew: false } } };
      this.networksError = null;
      return true;
    } catch (e) {
      this.networksError = errorText(e);
      return false;
    }
  }

  async forgetNetwork(id: number) {
    try {
      await api.netmapNetworkForget(id);
      this.setOverlay(this.overlay.filter((x) => x !== id));
      this.networks = this.networks.filter((x) => x.id !== id);
      void this.loadInventory();
    } catch (e) {
      this.networksError = errorText(e);
    }
  }

  /** Folds one saved network into another (its router was replaced). */
  async mergeNetwork(from: number, into: number) {
    try {
      await api.netmapNetworkMerge(from, into);
      this.setOverlay(this.overlay.filter((x) => x !== from));
      if (this.result?.network?.id === from) {
        const n = this.networks.find((x) => x.id === into);
        if (n) this.result = { ...this.result, network: { id: into, name: n.name, isNew: false } };
      }
      await this.loadNetworks();
      void this.loadInventory();
    } catch (e) {
      this.networksError = errorText(e);
    }
  }

  get rangeList(): string[] {
    return this.ranges.split(/[\s,]+/).map((r) => r.trim()).filter(Boolean);
  }

  /** Devices this scan saw for the first time and nobody has marked trusted. */
  get newDevices() {
    return this.result?.devices.filter((d) => d.isNew && !d.trusted) ?? [];
  }

  async loadInventory() {
    try {
      this.inventory = await api.netmapInventory();
      this.inventoryError = null;
    } catch (e) {
      this.inventoryError = errorText(e);
    }
  }

  /** Marks a device as known and expected (or not). */
  async trust(key: string, trusted: boolean) {
    try {
      await api.netmapDeviceTrust(key, trusted);
      if (this.result) {
        this.result = {
          ...this.result,
          devices: this.result.devices.map((d) => ((d.mac ?? `ip:${d.ip}`).toLowerCase() === key ? { ...d, trusted } : d)),
          missing: this.result.missing.map((m) => (m.key === key ? { ...m, trusted } : m)),
        };
      }
      this.inventory = this.inventory.map((d) => (d.key === key ? { ...d, trusted } : d));
    } catch (e) {
      this.inventoryError = errorText(e);
    }
  }

  /** Forgets a device: the next scan that sees it calls it new. */
  async forget(key: string) {
    try {
      await api.netmapDeviceForget(key);
      this.inventory = this.inventory.filter((d) => d.key !== key);
      if (this.result) this.result = { ...this.result, missing: this.result.missing.filter((m) => m.key !== key) };
    } catch (e) {
      this.inventoryError = errorText(e);
    }
  }

  async loadLabels() {
    try {
      this.labels = await api.deviceLabelsList();
      this.labelsLoaded = true;
    } catch (e) {
      this.labelError = errorText(e);
    }
  }

  /** Saves a label (an empty one removes it). Returns false on a validation error. */
  async saveLabel(l: DeviceLabel): Promise<boolean> {
    this.labelError = null;
    try {
      const saved = await api.deviceLabelSave(l);
      const key = l.key.trim().toLowerCase();
      this.labels = [...this.labels.filter((x) => x.key !== key && x.key !== saved?.key), ...(saved ? [saved] : [])];
      return true;
    } catch (e) {
      this.labelError = errorText(e);
      return false;
    }
  }

  async deleteLabel(key: string) {
    try {
      await api.deviceLabelDelete(key);
      this.labels = this.labels.filter((x) => x.key !== key);
    } catch (e) {
      this.labelError = errorText(e);
    }
  }

  setMode(m: "graph" | "list") {
    this.mode = m;
    try {
      localStorage.setItem("nightgrid.netmap.mode", m);
    } catch {
      /* private mode: just this session */
    }
  }

  async loadLayout() {
    try {
      this.layout = await api.netmapLayoutGet();
      this.layoutLoaded = true;
      this.layoutError = null;
    } catch (e) {
      this.layoutError = errorText(e);
    }
  }

  private async saveEntries(entries: LayoutEntry[], replace = false) {
    const before = this.layout;
    const keys = new Set(entries.map((e) => e.key));
    this.layout = replace ? entries : [...this.layout.filter((e) => !keys.has(e.key)), ...entries];
    try {
      await api.netmapLayoutSave(entries, replace);
      this.layoutError = null;
    } catch (e) {
      this.layout = before;
      this.layoutError = errorText(e);
    }
  }

  /** Remembers where the user dropped a node. */
  async move(id: string, x: number, y: number) {
    const t = this.topo.nodes.find((n) => n.id === id);
    if (!t) return;
    await this.saveEntries([entryFor(t, this.layoutMap, { x: Math.round(x), y: Math.round(y) })]);
  }

  /** Sets (or clears, with null) what a node connects through. Returns false on a loop. */
  async setParent(id: string, parentId: string | null, place = false): Promise<boolean> {
    const t = this.topo.nodes.find((n) => n.id === id);
    const p = parentId ? this.topo.nodes.find((n) => n.id === parentId) : null;
    if (!t || (parentId && (!p || !canAttach(this.topo, id, parentId)))) return false;
    // Dropped onto its new parent: let it fall in under it.
    await this.saveEntries([entryFor(t, this.layoutMap, { parent: p?.key ?? null, ...(place ? { x: null, y: null } : {}) })]);
    return true;
  }

  /** Forgets every position (parents and the expected picture stay). */
  async autoLayout() {
    await this.saveEntries(
      this.layout.filter((e) => e.parent || e.expected).map((e) => ({ ...e, x: null, y: null })),
      true,
    );
  }

  /** Saves the map as it is now as the expected picture. */
  async saveExpected() {
    const keep = this.layout.filter((e) => e.parent && !this.topo.nodes.some((t) => t.key === e.key && !t.ghost));
    await this.saveEntries([...expectedEntries(this.topo, this.layoutMap), ...keep.map((e) => ({ ...e, expected: false }))], true);
  }

  /** Adds a node to the expected picture where it is now, or takes it out. */
  async expect(id: string, on: boolean) {
    const t = this.topo.nodes.find((n) => n.id === id);
    if (!t) return;
    await this.saveEntries([entryFor(t, this.layoutMap, on ? { expected: true, x: Math.round(t.x), y: Math.round(t.y) } : { expected: false })]);
  }

  /** Drops the expected picture (positions and parents stay). */
  async clearExpected() {
    await this.saveEntries(
      this.layout.filter((e) => e.parent || e.x !== null).map((e) => ({ ...e, expected: false })),
      true,
    );
  }

  /** Service scans of devices this session, by device key. */
  svc = $state<Record<string, { running: boolean; result: ToolResult | null; error: string | null; from: string }>>({});

  /** The worst risk a service scan found on a device. */
  svcRisk(key: string): "bad" | "warn" | null {
    const f = this.svc[key]?.result?.facts.filter((x) => x.label.startsWith("Port ")) ?? [];
    return f.some((x) => x.tone === "bad") ? "bad" : f.some((x) => x.tone === "warn") ? "warn" : null;
  }

  /** Scans a device's common ports from the machine that ran the map scan (or this computer). */
  async scanServices(key: string, ip: string) {
    if (this.svc[key]?.running) return;
    const from = this.resultHost ?? "local";
    const name = from === "local" ? "this computer" : (hosts.byId(from)?.name ?? "?");
    this.svc = { ...this.svc, [key]: { running: true, result: null, error: null, from: name } };
    let next: { running: boolean; result: ToolResult | null; error: string | null; from: string };
    try {
      const result = await api.netTool(from === "local" ? null : from, { tool: "services", target: ip, option: "", server: "" });
      next = { running: false, result, error: null, from: name };
    } catch (e) {
      next = { running: false, result: null, error: errorText(e), from: name };
    }
    this.svc = { ...this.svc, [key]: next };
  }

  /** The web security check of the scanned devices (D-145). */
  security = $state<{ running: boolean; findings: NetFinding[] | null; error: string | null; at: number }>({ running: false, findings: null, error: null, at: 0 });

  /** Everything the scan and the web check found, worst first. */
  readonly findings: NetFinding[] = $derived(
    [...(this.result?.findings ?? []), ...(this.security.findings ?? [])].sort((a, b) => TONE_RANK[b.level] - TONE_RANK[a.level]),
  );

  /** The worst finding about one address. */
  findingRisk(ip: string | null | undefined): "bad" | "warn" | null {
    if (!ip) return null;
    const f = this.findings.filter((x) => x.ip === ip);
    return f.some((x) => x.level === "bad") ? "bad" : f.some((x) => x.level === "warn") ? "warn" : null;
  }

  /** Reads one page or certificate on each web port of the scanned devices. */
  async checkSecurity() {
    if (this.security.running || !this.result) return;
    const ips = this.result.devices.filter((d) => /^\d+\.\d+\.\d+\.\d+$/.test(d.ip)).map((d) => d.ip).slice(0, 64);
    const from = this.resultHost ?? "local";
    this.security = { ...this.security, running: true, error: null };
    try {
      const findings = await api.netmapSecurity(from === "local" ? null : from, ips);
      this.security = { running: false, findings, error: null, at: Math.floor(Date.now() / 1000) };
    } catch (e) {
      this.security = { ...this.security, running: false, error: errorText(e) };
    }
  }

  /** The last scheduled watch scan (Settings → Alerts → network watch). */
  watchLast = $state<NetWatchRun | null>(null);
  async loadWatch() {
    try {
      this.watchLast = await api.netmapWatchLast();
    } catch {
      this.watchLast = null;
    }
  }

  /** Port audit for one host (the same check as System → Security). */
  async audit(id: number) {
    if (this.auditing.includes(id)) return;
    this.auditing = [...this.auditing, id];
    try {
      const r = await api.systemSecurity(id);
      system.portMap = { ...system.portMap, [id]: r };
    } catch (e) {
      system.portMap = { ...system.portMap, [id]: errorText(e) };
    } finally {
      this.auditing = this.auditing.filter((x) => x !== id);
    }
  }
}

const TONE_RANK = { info: 0, ok: 1, warn: 2, bad: 3 } as const;

function readOverlay(): number[] {
  try {
    const v: unknown = JSON.parse(localStorage.getItem("nightgrid.netmap.overlay") ?? "[]");
    return Array.isArray(v) ? v.filter((x): x is number => Number.isInteger(x)) : [];
  } catch {
    return [];
  }
}

function readMode(): "graph" | "list" {
  try {
    return localStorage.getItem("nightgrid.netmap.mode") === "list" ? "list" : "graph";
  } catch {
    return "graph";
  }
}

export const netmap = new NetmapStore();
