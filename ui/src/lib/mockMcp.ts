/** Browser-only stand-in for the MCP server (M12g). */
import type { McpAsk, McpCall, McpSettings_Serialize as McpSettings, McpStatus_Serialize as McpStatus } from "./bindings";

let settings: McpSettings = { enabled: false, port: 47821, allowWrites: false };
let token = "3f9c0d4e8b2a71c6e5d4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2";
const calls: McpCall[] = [
  { at: Date.now() / 1000 - 240, client: "claude-ai", tool: "metric_history", outcome: "ok", detail: "host=server01 hours=24" },
  { at: Date.now() / 1000 - 300, client: "claude-ai", tool: "list_hosts", outcome: "ok", detail: "" },
];
let asks: McpAsk[] = [];

function status(): McpStatus {
  const running = settings.enabled;
  return {
    settings: { ...settings },
    running,
    url: running ? `http://127.0.0.1:${settings.port}/mcp` : null,
    error: null,
    token: running ? token : null,
    exe: "/Applications/Nightgrid.app/Contents/MacOS/nightgrid",
    calls: running ? [...calls] : [],
    asks: [...asks],
  };
}

export const mockMcp = {
  status: async (): Promise<McpStatus> => status(),
  set: async (s: McpSettings): Promise<McpStatus> => {
    if (s.port < 1024 || s.port > 65535) throw new Error("invalid value for mcp.port: must be between 1024 and 65535");
    settings = { ...s };
    return status();
  },
  newToken: async (): Promise<McpStatus> => {
    token = [...crypto.getRandomValues(new Uint8Array(32))].map((b) => b.toString(16).padStart(2, "0")).join("");
    return status();
  },
  answer: async (id: number, allow: boolean): Promise<null> => {
    const a = asks.find((x) => x.id === id);
    asks = asks.filter((x) => x.id !== id);
    if (a) calls.unshift({ at: Date.now() / 1000, client: a.client, tool: a.tool, outcome: allow ? "ok" : "denied", detail: a.summary });
    return null;
  },
  /** Preview helper: queue a change waiting for approval. */
  fakeAsk: (a: Omit<McpAsk, "id">): McpAsk => {
    const ask = { id: asks.length + 1, ...a };
    asks = [...asks, ask];
    return ask;
  },
};
