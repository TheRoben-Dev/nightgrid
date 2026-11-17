/** Client config snippets for the MCP server (M12g, D-166). */

/** A JSON `mcpServers` entry for clients that speak HTTP. */
export function httpConfig(url: string, token: string): string {
  return JSON.stringify({ mcpServers: { nightgrid: { type: "http", url, headers: { Authorization: `Bearer ${token}` } } } }, null, 2);
}

/** A JSON `mcpServers` entry for clients that start a command (stdio). */
export function stdioConfig(exe: string): string {
  return JSON.stringify({ mcpServers: { nightgrid: { command: exe, args: ["--mcp"] } } }, null, 2);
}

/** A one-line `claude mcp add` for Claude Code. */
export function cliCommand(url: string, token: string): string {
  return `claude mcp add --transport http nightgrid ${url} --header "Authorization: Bearer ${token}"`;
}

/** Hides all but the last four characters of a token. */
export function maskToken(token: string): string {
  return token.length <= 4 ? "••••" : `${"•".repeat(8)}${token.slice(-4)}`;
}

/** Replaces the token inside a snippet, for showing it masked. */
export function masked(text: string, token: string | null): string {
  return token ? text.split(token).join(maskToken(token)) : text;
}
