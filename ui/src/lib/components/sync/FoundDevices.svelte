<script lang="ts">
  /**
   * Nightgrid apps on this LAN (D-208): searched every few seconds while
   * shown. `compact` is the first-run card in the empty host list.
   */
  import { onMount } from "svelte";
  import type { FoundDevice } from "../../api";
  import { sync } from "../../stores/sync.svelte";
  import JoinFoundDialog from "./JoinFoundDialog.svelte";

  let { compact = false, every = 5000, stopAfter = 0 }: { compact?: boolean; every?: number; stopAfter?: number } = $props();

  let picked = $state<FoundDevice | null>(null);
  let searched = $state(false);

  onMount(() => {
    sync.start();
    const started = Date.now();
    const run = async () => {
      if (picked) return;
      if (stopAfter && Date.now() - started > stopAfter && searched) return;
      await sync.discover();
      searched = true;
    };
    void run();
    const t = setInterval(() => void run(), every);
    return () => clearInterval(t);
  });

  const list = $derived(compact ? sync.found.filter((d) => !d.paired) : sync.found);
</script>

{#if compact}
  {#each list as d (d.id)}
    <div class="first" role="status">
      <span class="tag">FOUND ON YOUR NETWORK</span>
      <span class="name">{d.name}</span>
      <span class="muted">{d.hosts} host{d.hosts === 1 ? "" : "s"} · {d.addr}</span>
      <button type="button" class="btn primary sm" onclick={() => (picked = d)}>Import and pair…</button>
    </div>
  {/each}
{:else}
  <ul class="found" aria-busy={sync.searching}>
    {#each list as d (d.id)}
      <li>
        <span class="dot" aria-hidden="true"></span>
        <span class="name">{d.name}</span>
        <span class="muted small mono">{d.addr}</span>
        <span class="muted small">Nightgrid {d.appVersion} · {d.hosts} host{d.hosts === 1 ? "" : "s"}{d.sameVersion ? "" : " · other version"}</span>
        <span class="spacer"></span>
        {#if d.paired}
          <span class="chip">PAIRED</span>
        {:else}
          <button type="button" class="btn sm" onclick={() => (picked = d)}>Import and pair…</button>
        {/if}
      </li>
    {:else}
      <li class="muted small">
        {searched ? "No other Nightgrid found yet. On the other computer, turn on \"Let my other devices find me\" in Settings → Sync. Both need to be on the same network." : "Looking…"}
      </li>
    {/each}
  </ul>
{/if}

<JoinFoundDialog device={picked} onclose={() => (picked = null)} />

<style>
  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .found li {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--mint);
    box-shadow: 0 0 6px var(--mint);
  }
  .name {
    color: var(--text-0);
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .chip {
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--mint);
    border: 1px solid color-mix(in srgb, var(--mint) 50%, var(--line));
    padding: 1px 6px;
  }
  .muted {
    color: var(--text-1);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .first {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    margin-bottom: 8px;
    border: 1px solid var(--cyan);
    background: color-mix(in srgb, var(--cyan) 8%, transparent);
    font-size: var(--fs-xs);
  }
  .first .tag {
    letter-spacing: 0.12em;
    color: var(--cyan);
  }
  .first .btn {
    align-self: flex-start;
    margin-top: 4px;
  }
</style>
