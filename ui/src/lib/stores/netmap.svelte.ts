/**
 * Network map (M10i). Nothing is scanned until the user clicks: discovery runs
 * from one chosen host, port audits reuse System → Security.
 */
import { api, errorText, systemError, type Discovery } from "../api";
import { buildGraph, type NetGraph } from "../netmap/graph";
import { hosts } from "./hosts.svelte";
import { system } from "./system.svelte";

class NetmapStore {
  /** A host id, "local" (this computer), or null for the default. */
  scanHost = $state<number | "local" | null>(null);
  sweep = $state(false);
  result = $state<Discovery | null>(null);
  /** Where the current result was scanned from. */
  resultHost = $state<number | "local" | null>(null);
  scannedAt = $state<number | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  selected = $state<string | null>(null);
  auditing = $state<number[]>([]);

  readonly graph: NetGraph = $derived(buildGraph(hosts.hosts, system.portMap, this.result, this.resultHost));
  readonly current = $derived(this.graph.nodes.find((n) => n.id === this.selected) ?? null);

  /** Where to scan from: the user's pick, else the first host, else this computer. */
  get from(): number | "local" {
    if (this.scanHost === "local") return "local";
    if (this.scanHost !== null && hosts.byId(this.scanHost)) return this.scanHost;
    return hosts.hosts[0]?.id ?? "local";
  }

  async scan() {
    const id = this.from;
    if (this.loading) return;
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
