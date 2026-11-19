<script lang="ts">
  /** Settings → Connections → MCP server (M12g, D-166). */
  import { onMount } from "svelte";
  import { writeClipboard } from "../../clipboard";
  import { cliCommand, httpConfig, masked, stdioConfig } from "../../mcp";
  import { mcp } from "../../stores/mcp.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";

  type Kind = "http" | "stdio" | "cli";
  let kind = $state<Kind>("http");
  let revealed = $state(false);
  let copied = $state(false);
  let port = $state("47821");

  onMount(() => {
    void mcp.refresh().then(() => {
      if (mcp.status) port = String(mcp.status.settings.port);
    });
  });

  const s = $derived(mcp.status);
  const snippet = $derived.by(() => {
    if (!s?.url || !s.token) return "";
    if (kind === "stdio") return s.exe ? stdioConfig(s.exe) : "";
    return kind === "cli" ? cliCommand(s.url, s.token) : httpConfig(s.url, s.token);
  });

  async function copy() {
    await writeClipboard(snippet);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function when(at: number | null): string {
    return at ? new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "";
  }
</script>

<Panel title="MCP SERVER · AI CLIENTS" guide="settings-mcp">
  <p class="muted lead">
    Lets an AI client on this computer (Claude Desktop, Claude Code, or any MCP client) read your homelab through Nightgrid: hosts, metrics, alerts,
    timelines, service checks, the journal, disk forecasts and containers. Only on 127.0.0.1, behind a token. Read-only unless you allow changes below.
  </p>
  {#if s}
    <Toggle
      label="Run the MCP server"
      hint="Off by default. Nightgrid must be running for a client to reach it."
      checked={s.settings.enabled}
      disabled={mcp.busy}
      onchange={(v) => {
        if (!v) revealed = false;
        void mcp.set({ enabled: v });
      }}
    />
    <Toggle
      label="Allow changes (each one asks here first)"
      hint="Restart or stop a container, add a journal note. Every change waits for Allow in Nightgrid; nothing runs without you."
      checked={s.settings.allowWrites}
      disabled={mcp.busy}
      onchange={(v) => void mcp.set({ allowWrites: v })}
    />
    <label class="field port">
      <span class="sub">Port</span>
      <input
        class="input"
        inputmode="numeric"
        bind:value={port}
        disabled={mcp.busy}
        onchange={() => {
          const n = Number(port);
          if (Number.isInteger(n) && n !== s.settings.port) void mcp.set({ port: n });
        }}
      />
    </label>
    {#if s.error}<p class="error-text">{s.error}</p>{/if}
    {#if mcp.error}<p class="error-text">{mcp.error}</p>{/if}

    {#if s.running && s.url && s.token}
      <div class="block" data-guide="mcp-config">
        <span class="sub">Connect a client</span>
        <div class="seg tabstrip sm" role="tablist">
          <button type="button" role="tab" aria-selected={kind === "http"} class:on={kind === "http"} onclick={() => (kind = "http")}>HTTP (JSON)</button>
          <button type="button" role="tab" aria-selected={kind === "stdio"} class:on={kind === "stdio"} onclick={() => (kind = "stdio")}>Command (stdio)</button>
          <button type="button" role="tab" aria-selected={kind === "cli"} class:on={kind === "cli"} onclick={() => (kind = "cli")}>Claude Code</button>
        </div>
        {#if snippet}
          <pre class="snippet">{revealed ? snippet : masked(snippet, s.token)}</pre>
        {:else}
          <p class="muted small">This app's path is unknown; use HTTP.</p>
        {/if}
        <p class="muted small">
          {#if kind === "stdio"}
            For clients that only start a command: <code>--mcp</code> forwards to the running app and reads the token itself, so the config holds no secret.
          {:else}
            Paste it into the client's MCP config. The token stays in Nightgrid's data folder too; a new token makes old configs stop working.
          {/if}
        </p>
        <div class="actions">
          <button type="button" class="btn primary tiny" disabled={!snippet} onclick={copy}>{copied ? "Copied" : "Copy"}</button>
          {#if kind !== "stdio"}
            <button type="button" class="btn ghost tiny" onclick={() => (revealed = !revealed)}>{revealed ? "Hide token" : "Reveal token"}</button>
          {/if}
          <button type="button" class="btn ghost tiny" disabled={mcp.busy} title="Clients using the old token stop working" onclick={() => void mcp.newToken()}
            >New token</button
          >
        </div>
      </div>

      <div class="block">
        <span class="sub">Recent calls</span>
        {#if s.calls.length}
          <ul class="calls">
            {#each s.calls.slice(0, 12) as c, i (i)}
              <li class:bad={c.outcome !== "ok"}>
                <span class="t">{when(c.at)}</span>
                <span class="tool">{c.tool}</span>
                <span class="who">{c.client}</span>
                <span class="d">{c.outcome === "ok" ? c.detail : `${c.outcome}: ${c.detail}`}</span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted small">No calls yet.</p>
        {/if}
      </div>
    {:else if s.settings.enabled && !s.error}
      <p class="muted small">Starting…</p>
    {/if}
  {:else if mcp.error}
    <p class="error-text">{mcp.error}</p>
  {/if}
</Panel>

<style>
  .lead {
    margin: 0 0 10px;
    font-size: var(--fs-sm);
  }
  .sub {
    display: block;
    margin-bottom: 6px;
    color: var(--text-1);
  }
  .port .input {
    width: 100%;
  }
  .port {
    max-width: 180px;
    margin-top: 10px;
  }
  .block {
    border-top: 1px solid var(--line);
    padding-top: 10px;
    margin-top: 10px;
  }
  .seg {
    margin-bottom: 8px;
  }
  .snippet {
    margin: 0;
    padding: 8px;
    border: 1px solid var(--line);
    background: var(--bg-0, transparent);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 220px;
    overflow: auto;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
  .small {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
  }
  .calls {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 3px;
    font-size: var(--fs-xs);
  }
  .calls li {
    display: grid;
    grid-template-columns: auto minmax(90px, auto) minmax(60px, auto) 1fr;
    gap: 8px;
    min-width: 0;
  }
  .calls .t {
    white-space: nowrap;
    color: var(--text-1);
  }
  .calls .tool {
    color: var(--cyan);
  }
  .calls .who {
    color: var(--text-1);
  }
  .calls .d {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .calls li.bad .d {
    color: var(--warn);
  }
</style>
