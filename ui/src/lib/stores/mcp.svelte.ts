/**
 * The MCP server (M12g, D-166): its status for Settings, and the changes an
 * AI client asked for that wait for a yes or no here.
 */
import { listen } from "@tauri-apps/api/event";
import { api, errorText, inTauri, type McpAsk, type McpSettings, type McpStatus } from "../api";

class McpStore {
  status = $state<McpStatus | null>(null);
  asks = $state<McpAsk[]>([]);
  error = $state<string | null>(null);
  busy = $state(false);
  private started = false;

  /** Listens for approval requests; called once at startup. */
  start() {
    if (this.started) return;
    this.started = true;
    if (inTauri()) {
      void listen<McpAsk>("mcp://ask", (e) => {
        if (!this.asks.some((a) => a.id === e.payload.id)) this.asks = [...this.asks, e.payload];
      });
      // A request made while the window was closed is still waiting.
      window.addEventListener("focus", () => void this.refresh());
    }
    void this.refresh();
  }

  async refresh() {
    try {
      const s = await api.mcpStatus();
      this.status = s;
      this.asks = s.asks;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  private async run(f: () => Promise<McpStatus>) {
    this.busy = true;
    this.error = null;
    try {
      this.status = await f();
      this.asks = this.status.asks;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.busy = false;
    }
  }

  set = (patch: Partial<McpSettings>) => {
    const cur = this.status?.settings;
    if (!cur) return Promise.resolve();
    return this.run(() => api.mcpSet({ ...cur, ...patch }));
  };
  newToken = () => this.run(() => api.mcpNewToken());

  async answer(id: number, allow: boolean) {
    this.asks = this.asks.filter((a) => a.id !== id);
    try {
      await api.mcpAnswer(id, allow);
    } catch (e) {
      this.error = errorText(e);
    }
    void this.refresh();
  }
}

export const mcp = new McpStore();
