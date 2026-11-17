import { describe, expect, it } from "vitest";
import { cliCommand, httpConfig, masked, maskToken, stdioConfig } from "./mcp";

describe("mcp snippets", () => {
  it("builds the HTTP entry with a bearer header", () => {
    const c = JSON.parse(httpConfig("http://127.0.0.1:47821/mcp", "abc"));
    expect(c.mcpServers.nightgrid).toEqual({ type: "http", url: "http://127.0.0.1:47821/mcp", headers: { Authorization: "Bearer abc" } });
  });
  it("builds the stdio entry", () => {
    expect(JSON.parse(stdioConfig("/opt/nightgrid")).mcpServers.nightgrid).toEqual({ command: "/opt/nightgrid", args: ["--mcp"] });
  });
  it("masks the token everywhere", () => {
    const t = "0123456789abcdef";
    expect(maskToken(t)).toBe("••••••••cdef");
    const text = cliCommand("http://127.0.0.1:1/mcp", t);
    expect(masked(text, t)).not.toContain(t);
    expect(masked(text, null)).toBe(text);
  });
});
