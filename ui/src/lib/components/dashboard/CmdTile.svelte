<script lang="ts">
  /** A custom command widget's tile (M10x): the parsed value, coloured by its thresholds. */
  import { ago } from "../../gitstacks";
  import { display, intervalLabel } from "../../cmdwidgets";
  import { cmdWidgets } from "../../stores/cmdwidgets.svelte";
  import { hosts } from "../../stores/hosts.svelte";

  let { id }: { id: number } = $props();

  $effect(() => cmdWidgets.watch(id));
  $effect(() => {
    if (!cmdWidgets.loaded) void cmdWidgets.load();
  });

  const w = $derived(cmdWidgets.byId(id));
  const r = $derived(cmdWidgets.readings[id]);
  const err = $derived(cmdWidgets.errors[id]);
  const host = $derived(w ? (hosts.hosts.find((h) => h.id === w.hostId)?.name ?? `#${w.hostId}`) : "");
  let showOut = $state(false);
</script>

{#if !cmdWidgets.loaded}
  <p class="muted small">Loading…</p>
{:else if !w}
  <p class="muted small">This command widget was deleted. Remove the tile in Edit mode.</p>
{:else}
  <div class="tile" data-guide="cmd-tile">
    <p class="val {err ? 'error' : (r?.level ?? 'info')}" title={r?.output || w.command}>{err ? "ERROR" : display(r, w.unit)}</p>
    <p class="meta muted small">
      <span class="host">{host}</span>
      <span>· {cmdWidgets.running[id] ? "running…" : r ? ago(r.at) : "not run yet"}</span>
      <span>· every {intervalLabel(w.intervalSecs)}</span>
    </p>
    {#if err}<p class="small errtext" title={err}>{err}</p>{/if}
    {#if showOut && r}<pre class="out">{r.output || "(no output)"}</pre>{/if}
    <div class="foot">
      <button type="button" class="link" disabled={cmdWidgets.running[id]} onclick={() => cmdWidgets.run(id)}>Run now</button>
      {#if r}<button type="button" class="link" aria-expanded={showOut} onclick={() => (showOut = !showOut)}>{showOut ? "Hide output" : "Output"}</button>{/if}
      <button type="button" class="link" data-guide="cmd-edit" onclick={() => (cmdWidgets.editing = { ...w })}>Edit…</button>
    </div>
  </div>
{/if}

<style>
  .tile {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .val {
    margin: 0;
    font-size: calc(var(--fs-lg) * 1.5);
    font-family: var(--font-mono);
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .val.ok {
    color: var(--ok);
  }
  .val.warn {
    color: var(--warn);
  }
  .val.crit,
  .val.error,
  .errtext {
    color: var(--danger);
  }
  .val.info {
    color: var(--cyan);
  }
  .meta {
    margin: 2px 0 0;
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .host {
    color: var(--text-0);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .errtext {
    margin: 2px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .out {
    margin: 4px 0 0;
    flex: 1;
    min-height: 0;
    overflow: auto;
    font-size: var(--fs-xs);
    background: var(--bg-0);
    border: 1px solid var(--line);
    padding: 4px 6px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .foot {
    margin-top: auto;
    padding-top: 4px;
    display: flex;
    gap: 10px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-xs);
  }
  .link:disabled {
    color: var(--text-1);
    cursor: default;
  }
</style>
