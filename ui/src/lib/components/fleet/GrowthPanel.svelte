<script lang="ts">
  /**
   * Fleet → DISKS → What's growing (D-200): the biggest folders and files
   * under a mount, what grew since the last scan (kept on the host), VM
   * disk images, and which hosts' disks are stored on this drive.
   */
  import type { GrowthEntry, MountForecast } from "../../api";
  import { childPath, growingEntries, linkSuggestions, parentPath, deltaText } from "../../growth";
  import { agoText, size } from "../../fleet";
  import { fleet } from "../../stores/fleet.svelte";
  import { hosts } from "../../stores/hosts.svelte";

  let { hostId, mount, forecast, onopen }: { hostId: number; mount: string; forecast: MountForecast; onopen: (hostId: number, mount: string) => void } = $props();

  // Starts at the mount; drilling in moves it (a new mount gets a new panel).
  // svelte-ignore state_referenced_locally
  let path = $state(mount);
  let pick = $state("");
  let linkError = $state<string | null>(null);

  const key = $derived(`${hostId}|${path}`);
  const g = $derived(fleet.growth[key]);
  const r = $derived(g?.report ?? null);
  const growing = $derived(r ? growingEntries(r) : []);
  const biggest = $derived(r ? r.entries.slice(0, 12) : []);
  const images = $derived(r ? r.entries.filter((e) => e.vmImage) : []);
  const stored = $derived(fleet.links.filter((l) => l.ownerHostId === hostId && l.ownerMount === mount));
  const others = $derived(hosts.hosts.filter((h) => h.id !== hostId && !stored.some((l) => l.hostId === h.id)));
  const suggest = $derived(linkSuggestions(images, others));
  const nameOf = (id: number) => hosts.hosts.find((h) => h.id === id)?.name ?? `host ${id}`;

  $effect(() => {
    if (!fleet.growth[key]) void fleet.scanGrowth(hostId, path);
  });

  function open(e: GrowthEntry) {
    if (e.dir) path = childPath(path, e.path);
  }

  async function link(id: number) {
    linkError = null;
    try {
      await fleet.addLink(id, hostId, mount);
      pick = "";
    } catch (e) {
      linkError = String((e as { message?: string })?.message ?? e);
    }
  }
</script>

<div class="growth" data-guide="fleet-disks-growth">
  <div class="bar">
    <span class="title">WHAT'S GROWING</span>
    <span class="path">{path}</span>
    {#if path !== mount}<button type="button" class="btn ghost tiny" onclick={() => (path = parentPath(path, mount))}>Up</button>{/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost tiny" disabled={g?.busy} onclick={() => fleet.scanGrowth(hostId, path)}>{g?.busy ? "Counting…" : "Scan again"}</button>
  </div>

  {#if forecast.owner}
    <p class="muted small">
      This is {forecast.owner.mount} on {forecast.owner.host}: scanning it there is faster and sees every file.
      <button type="button" class="btn ghost tiny" onclick={() => forecast.owner && onopen(forecast.owner.hostId, forecast.owner.mount)}>Open on {forecast.owner.host}</button>
    </p>
  {/if}
  {#if g?.error}<p class="bad small">{g.error}</p>{/if}
  {#if g?.busy && !r}<p class="muted small">Counting every folder on this filesystem. A big drive can take a few minutes.</p>{/if}

  {#if r?.missing}
    <p class="muted small">This folder isn't there or can't be opened.</p>
  {:else if r}
    <p class="small summary">
      {size(r.total)} here.
      {#if r.prevAt}
        <span class:grew={(r.totalDelta ?? 0) > 0}>{deltaText(r.totalDelta)} since the last scan {agoText(r.prevAt * 1000, r.at * 1000)}.</span>
      {:else}
        <span class="muted">First scan of this folder: scan again later (tomorrow is good) to see what grew.</span>
      {/if}
      {#if r.denied}
        <span class="muted">{r.denied} folder{r.denied === 1 ? "" : "s"} couldn't be read.</span>
        <button type="button" class="btn ghost tiny" disabled={g?.busy} onclick={() => fleet.scanGrowth(hostId, path, true)}>Read as root</button>
      {/if}
    </p>

    <div class="cols">
      {#if growing.length}
        <div>
          <div class="head">GREW SINCE LAST SCAN</div>
          {#each growing as e (e.path)}{@render row(e, true)}{/each}
        </div>
      {/if}
      <div>
        <div class="head">BIGGEST</div>
        {#each biggest as e (e.path)}{@render row(e, false)}{/each}
        {#if !biggest.length}<p class="muted small">Nothing here.</p>{/if}
      </div>
    </div>
  {/if}

  {#if forecast.kind === "local" || forecast.kind === "pool" || forecast.kind === "passthrough"}
    <div class="links">
      <span class="head">DISKS STORED HERE</span>
      {#each stored as l (l.id)}
        <span class="chip">{nameOf(l.hostId)} <button type="button" class="x" aria-label="Unlink {nameOf(l.hostId)}" onclick={() => fleet.removeLink(l.id)}>×</button></span>
      {/each}
      {#each suggest as s (s.host.id)}
        <button type="button" class="btn ghost tiny" title="Found {s.file}" onclick={() => link(s.host.id)}>{s.host.name}'s disk is here</button>
      {/each}
      <select class="input tiny" aria-label="A host whose disks are stored here" bind:value={pick} onchange={() => pick && link(Number(pick))}>
        <option value="">+ a VM stored here…</option>
        {#each others as h (h.id)}<option value={String(h.id)}>{h.name}</option>{/each}
      </select>
      {#if linkError}<span class="bad small">{linkError}</span>{/if}
    </div>
    <p class="muted small">A VM whose disk image is on this drive stops when it fills up: linked, its alerts fold under this drive's DISK FULL. Shares mounted from here aren't links: they're found on their own.</p>
  {/if}
</div>

{#snippet row(e: GrowthEntry, delta: boolean)}
  <div class="entry">
    <button type="button" class="name" class:dir={e.dir} disabled={!e.dir} onclick={() => open(e)} title={e.dir ? "Look inside" : e.path}>{e.path}{e.dir ? "/" : ""}</button>
    {#if e.vmImage}<span class="tag vm">VM DISK</span>{/if}
    {#if e.recent}<span class="tag">CHANGED TODAY</span>{/if}
    <span class="spacer"></span>
    {#if delta}<span class="grew small">{deltaText(e.delta)}</span>{/if}
    <span class="small muted num">{size(e.size)}</span>
  </div>
  {#if e.hint}<p class="hint small muted">{e.hint}</p>{/if}
{/snippet}

<style>
  .growth {
    border-left: 1px solid var(--cyan);
    padding: 6px 0 6px 10px;
    margin: 4px 0 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .bar,
  .links,
  .entry {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .title,
  .head {
    letter-spacing: 0.1em;
    color: var(--cyan);
    font-size: var(--fs-xs);
  }
  .path {
    font-family: var(--font-mono);
    color: var(--text-0);
    overflow-wrap: anywhere;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  p {
    margin: 0;
  }
  .bad {
    color: var(--danger);
  }
  .grew {
    color: var(--warn);
  }
  .summary {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
  }
  .entry {
    flex-wrap: nowrap;
    min-width: 0;
  }
  .name {
    all: unset;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .name.dir {
    color: var(--text-0);
    cursor: pointer;
  }
  .name.dir:hover,
  .name.dir:focus-visible {
    color: var(--cyan);
    text-decoration: underline;
  }
  .num {
    white-space: nowrap;
  }
  .tag {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    padding: 0 4px;
    border: 1px solid var(--line);
    color: var(--text-1);
    white-space: nowrap;
  }
  .tag.vm {
    color: var(--magenta, var(--warn));
    border-color: currentColor;
  }
  .hint {
    margin: 0 0 4px 12px;
  }
  .chip {
    padding: 0 6px;
    border: 1px solid var(--line);
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .x {
    all: unset;
    cursor: pointer;
    color: var(--text-1);
  }
  .x:hover {
    color: var(--danger);
  }
</style>
