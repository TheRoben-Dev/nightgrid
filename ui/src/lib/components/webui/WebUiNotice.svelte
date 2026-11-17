<script lang="ts">
  /** Why a web UI didn't open in its Nightgrid window, over every view (D-147). */
  import { webui } from "../../stores/webui.svelte";
</script>

{#if webui.notice}
  {@const n = webui.notice}
  <div class="notice" role="alert" data-guide="webui-notice">
    <div class="head"><span class="tag">WEB UI</span><b>{n.title}</b></div>
    <p>{n.text}</p>
    <p class="url" title={n.url}>{n.url}</p>
    <div class="acts">
      {#if n.browser}
        <button type="button" class="btn primary tiny" onclick={() => void webui.inBrowser(n)}>Open in browser</button>
      {/if}
      <button type="button" class="btn ghost tiny" onclick={() => (webui.notice = null)}>Hide</button>
    </div>
  </div>
{/if}

<style>
  .notice {
    position: fixed;
    bottom: 16px;
    right: 16px;
    z-index: 60;
    width: min(420px, calc(100vw - 32px));
    padding: 10px 12px;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--bg-1, #0b0d12) 92%, var(--warn));
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
    color: var(--warn);
  }
  p {
    margin: 6px 0 0;
    font-size: var(--fs-sm);
  }
  .url {
    color: var(--cyan);
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .acts {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 8px;
  }
</style>
