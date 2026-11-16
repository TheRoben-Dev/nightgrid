/**
 * Network tools (D-142). Each tool runs on the machine picked under RUN FROM
 * (a host over SSH, or this computer) with that machine's own commands.
 * The map's device panel opens a tool here with the address filled in.
 */
import { api, errorText, type NetToolId, type ToolResult } from "../api";
import { hosts } from "./hosts.svelte";
import { ui } from "./ui.svelte";

export type ToolId = NetToolId | "wol" | "cidr" | "mac";

export interface RunRecord {
  id: number;
  tool: ToolId;
  from: string;
  at: number;
  result: ToolResult | null;
  error: string | null;
}

class NetToolsStore {
  tool = $state<ToolId>("ping");
  /** Where tools run: a host id or "local". null until picked (defaults below). */
  from = $state<number | "local" | null>(null);
  target = $state("");
  count = $state("4");
  dnsType = $state("A");
  dnsServer = $state("");
  ports = $state("22,80,443");
  url = $state("");
  mac = $state("");
  cidr = $state("");
  running = $state(false);
  /** Newest first, at most 12. */
  history = $state<RunRecord[]>([]);
  macResult = $state<string | null>(null);
  private seq = 0;

  /** The machine to run from: the pick, else the remembered host, else this computer. */
  get runFrom(): number | "local" {
    if (this.from !== null && (this.from === "local" || hosts.byId(this.from))) return this.from;
    return hosts.preferred("nettools", (h) => h.role !== "client")?.id ?? "local";
  }

  get fromName(): string {
    const f = this.runFrom;
    return f === "local" ? "this computer" : (hosts.byId(f)?.name ?? "?");
  }

  setFrom(f: number | "local") {
    this.from = f;
    if (f !== "local") hosts.remember("nettools", f);
  }

  /** Opens a tool with a target filled in (from the map), run from `from` when given. */
  open(tool: ToolId, target: string, from?: number | "local" | null) {
    this.tool = tool;
    if (tool === "http") this.url = target.startsWith("http") ? target : `http://${target}`;
    else if (tool === "wol" || tool === "mac") this.mac = target;
    else if (tool === "cidr") this.cidr = target;
    else this.target = target;
    if (from !== undefined && from !== null) this.setFrom(from);
    ui.go("nettools");
  }

  async run() {
    const t = this.tool;
    if (this.running || t === "cidr" || t === "mac" || t === "wol") return;
    this.running = true;
    const from = this.runFrom;
    const rec: RunRecord = { id: ++this.seq, tool: t, from: this.fromName, at: Date.now(), result: null, error: null };
    try {
      rec.result = await api.netTool(from === "local" ? null : from, {
        tool: t,
        target: t === "http" ? this.url.trim() : this.target.trim(),
        option: t === "ping" ? this.count : t === "dns" ? this.dnsType : t === "ports" ? this.ports : "",
        server: t === "dns" ? this.dnsServer.trim() : "",
      });
    } catch (e) {
      rec.error = errorText(e);
    } finally {
      this.history = [rec, ...this.history].slice(0, 12);
      this.running = false;
    }
  }

  async wake() {
    if (this.running) return;
    this.running = true;
    const from = this.runFrom;
    const rec: RunRecord = { id: ++this.seq, tool: "wol", from: this.fromName, at: Date.now(), result: null, error: null };
    try {
      const mac = await api.netWake(from === "local" ? null : from, this.mac.trim());
      rec.result = { ran: `Wake-on-LAN ${mac}`, output: "", facts: [{ label: "Magic packet", value: `sent to ${mac}`, tone: "ok" }], table: null, note: null };
    } catch (e) {
      rec.error = errorText(e);
    } finally {
      this.history = [rec, ...this.history].slice(0, 12);
      this.running = false;
    }
  }

  async lookupMac() {
    this.macResult = (await api.netMacVendor(this.mac.trim()).catch(() => null)) ?? "Not in the maker list";
  }
}

export const nettools = new NetToolsStore();
