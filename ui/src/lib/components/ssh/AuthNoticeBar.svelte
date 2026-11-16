<script lang="ts">
  /**
   * Login notices while a host connects (0.10.51): a Tailscale SSH check's
   * approval link, or another server's banner. Shown over every view, since
   * any view may be the one waiting for the connection.
   */
  import { openExternal } from "../../clipboard";
  import { authNotices } from "../../stores/authNotices.svelte";
  import { hosts } from "../../stores/hosts.svelte";
</script>

{#if authNotices.shown.length}
  <div class="notices" role="status" aria-live="polite">
    {#each authNotices.shown as n (authNotices.key(n))}
      {@const name = hosts.byId(n.hostId)?.name ?? n.notice.target}
      <div class="notice">
        <div class="head">
          <span class="tag">{n.notice.tailscale ? "TAILSCALE SSH" : "SERVER MESSAGE"}</span>
          <b>{name}</b>
          <span class="muted">is waiting to log in</span>
        </div>
        {#if n.notice.url}
          <p>{n.notice.tailscale ? "This tailnet asks you to confirm the login in a browser." : "The server sent a link to open."} The connection goes ahead by itself once it's approved.</p>
          <p class="url" title={n.notice.text}>{n.notice.url}</p>
        {:else}
          <pre>{n.notice.text}</pre>
        {/if}
        <div class="acts">
          {#if n.notice.url}
            {@const url = n.notice.url}
            <button type="button" class="btn primary tiny" data-guide="auth-approve" onclick={() => void openExternal(url)}>Approve in browser</button>
          {/if}
          <button type="button" class="btn ghost tiny" onclick={() => authNotices.dismiss(n)}>Hide</button>
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .notices {
    position: fixed;
    top: 64px;
    right: 16px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: min(420px, calc(100vw - 32px));
  }
  .notice {
    padding: 10px 12px;
    border: 1px solid var(--magenta);
    background: color-mix(in srgb, var(--bg-1, #0b0d12) 92%, var(--magenta));
    box-shadow: var(--glow-active);
  }
  .head {
    display: flex;
    gap: 8px;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .tag {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.14em;
    color: var(--magenta);
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
  pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
    font-size: var(--fs-xs);
    max-height: 120px;
    overflow: auto;
  }
  .acts {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
</style>
