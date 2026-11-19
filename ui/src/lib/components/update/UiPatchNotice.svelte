<script lang="ts">
  /** A downloaded interface patch is ready: restart now or on the next start (D-185). */
  import { uiPatch } from "../../stores/uipatch.svelte";
</script>

{#if uiPatch.notice && uiPatch.status?.waiting}
  {@const w = uiPatch.status.waiting}
  <div class="notice" role="status" data-guide="ui-patch-notice">
    <div class="head"><span class="tag">INTERFACE FIX</span><b>Ready to apply</b></div>
    {#if w.notes.length}
      <ul>
        {#each w.notes.slice(0, 4) as n (n)}<li>{n}</li>{/each}
      </ul>
    {:else}
      <p>Interface fixes for this version were downloaded and verified.</p>
    {/if}
    <p class="muted">They apply when Nightgrid restarts. Terminals and tunnels close on restart.</p>
    <div class="acts">
      <button type="button" class="btn primary tiny" onclick={() => uiPatch.restart()}>Restart now</button>
      <button type="button" class="btn ghost tiny" onclick={() => (uiPatch.notice = false)}>Next start</button>
    </div>
  </div>
{/if}

<style>
  .notice {
    position: fixed;
    bottom: 16px;
    right: 16px;
    z-index: 59;
    width: min(380px, calc(100vw - 32px));
    padding: 10px 12px;
    border: 1px solid var(--cyan);
    background: color-mix(in srgb, var(--bg-1, #0b0d12) 92%, var(--cyan));
  }
  .head {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .tag {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.14em;
    color: var(--cyan);
  }
  p,
  ul {
    margin: 6px 0 0;
    font-size: var(--fs-sm);
  }
  ul {
    padding-left: 18px;
  }
  .muted {
    color: var(--muted);
    font-size: var(--fs-xs);
  }
  .acts {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 8px;
  }
</style>
