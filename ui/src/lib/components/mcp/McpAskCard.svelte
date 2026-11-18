<script lang="ts">
  /** A change an AI client asked for over MCP, waiting for a yes or no (D-166). */
  import { mcp } from "../../stores/mcp.svelte";
</script>

{#if mcp.asks.length}
  {@const a = mcp.asks[0]}
  <div class="ask" role="alertdialog" aria-labelledby="mcp-ask-title" data-guide="mcp-ask">
    <div class="head">
      <span class="tag">MCP · APPROVAL</span>
      {#if mcp.asks.length > 1}<span class="more">+{mcp.asks.length - 1} waiting</span>{/if}
    </div>
    <p id="mcp-ask-title"><b>{a.client || "An AI client"}</b> wants to run <code>{a.tool}</code>:</p>
    <p class="what">{a.summary}</p>
    <p class="small">Nothing happens unless you allow it. The request expires after two minutes.</p>
    <div class="acts">
      <button type="button" class="btn ghost tiny" onclick={() => void mcp.answer(a.id, false)}>Deny</button>
      <button type="button" class="btn primary tiny" onclick={() => void mcp.answer(a.id, true)}>Allow once</button>
    </div>
  </div>
{/if}

<style>
  .ask {
    position: fixed;
    top: 56px;
    right: 16px;
    z-index: 70;
    width: min(420px, calc(100vw - 32px));
    padding: 10px 12px;
    border: 1px solid var(--magenta, var(--warn));
    background: color-mix(in srgb, var(--bg-1, #0b0d12) 92%, var(--magenta, var(--warn)));
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .tag {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.14em;
    color: var(--magenta, var(--warn));
  }
  .more {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  p {
    margin: 6px 0 0;
    font-size: var(--fs-sm);
  }
  .what {
    color: var(--cyan);
    overflow-wrap: anywhere;
  }
  .small {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
</style>
