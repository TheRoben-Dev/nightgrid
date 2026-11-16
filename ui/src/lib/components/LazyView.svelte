<script lang="ts" module>
  import type { Component } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type Loader = () => Promise<{ default: Component<any> }>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const loaded = new SvelteMap<Loader, Component<any>>();
</script>

<script lang="ts">
  // Renders a view loaded on first use; later visits render at once.
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
</script>

{#if comp}
  {@const View = comp}
  <View {...props} />
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
