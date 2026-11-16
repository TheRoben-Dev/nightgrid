/**
 * Network map (M10i). Nothing is scanned until the user clicks: discovery runs
 * from the node selected on the map (a host or this computer), port audits
 * reuse System → Security.
 */
import { api, errorText, systemError, type DeviceLabel, type Discovery } from "../api";
import { buildGraph, type NetGraph } from "../netmap/graph";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

class NetmapStore {
  sweep = $state(false);
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
  readonly graph: NetGraph = $derived(buildGraph(hosts.hosts, system.portMap, this.result, this.resultHost, this.labelMap));
  readonly current = $derived(this.graph.nodes.find((n) => n.id === this.selected) ?? null);

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
      this.result = id === "local" ? await api.netmapDiscoverLocal(this.sweep) : await api.netmapDiscover(id, this.sweep);
      this.resultHost = id;
      this.scannedAt = Date.now();
    } catch (e) {
      const { connect } = systemError(e);
      this.error =
        connect && connect.kind !== "network" && connect.kind !== "other"
          ? "This host needs a login or a host key decision first. Open a terminal on it, then scan again."
          : errorText(e);
    } finally {
      this.loading = false;
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

export const netmap = new NetmapStore();
