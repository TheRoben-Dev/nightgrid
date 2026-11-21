<script lang="ts" module>
  import type { Component } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type Loader = () => Promise<{ default: Component<any> }>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const loaded = new SvelteMap<Loader, Component<any>>();
</script>

<script lang="ts">
  import { componentPacks, packedInfo } from "../componentUi";
  // Renders a view loaded on first use; later visits render at once. A view
  // from a component pack that throws while rendering is swapped for the
  // copy built into the app (componentUi.ts).
  interface Props {
    load: Loader;
    props?: Record<string, unknown>;
  }
  let { load, props = {} }: Props = $props();
  const comp = $derived(loaded.get(load) ?? null);
  let error = $state<string | null>(null);
  $effect(() => {
    const l = load;
    if (loaded.has(l)) return;
    error = null;
    l()
      .then((m) => {
        loaded.set(l, m.default);
      })
      .catch((e) => (error = String((e as { message?: string })?.message ?? e)));
  });

  function renderFailed(e: unknown, reset: () => void) {
    const info = packedInfo(load);
    if (!info || !componentPacks.failed(info.componentId, e)) return;
    const l = load;
    void info.bundled().then((m) => {
      loaded.set(l, m.default);
      reset();
    });
  }
</script>

{#if comp}
  {@const View = comp}
  <svelte:boundary onerror={renderFailed}>
    <View {...props} />
    {#snippet failed(e)}
      <p class="error" role="alert"><span aria-hidden="true">▲</span> This view stopped: {String((e as { message?: string })?.message ?? e)}</p>
    {/snippet}
  </svelte:boundary>
{:else if error}
  <p class="error" role="alert"><span aria-hidden="true">▲</span> Couldn't load this view: {error}</p>
{:else}
  <p class="loading label" aria-live="polite">LOADING…</p>
{/if}

<style>
  .loading {
    color: var(--text-1);
    padding: 12px 2px;
  }
  .error {
    color: var(--danger);
  }
</style>
