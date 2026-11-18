<script lang="ts">
  /** The quest log (1.4): beside INBOX, every active Library quest with its current step and progress per host. */
  import { onMount } from "svelte";
  import { questLog } from "../../stores/questlog.svelte";
  import { quests as checks } from "../../stores/quests.svelte";
  import { library } from "../../stores/library.svelte";
  import { scramble } from "../../scramble";
  import { ui } from "../../stores/ui.svelte";
  import type { Quest } from "../../api";

  let root = $state<HTMLElement | null>(null);
  let showDone = $state(false);
  const active = $derived(questLog.active);
  const done = $derived(questLog.list.filter((q) => q.completedAt != null));

  onMount(() => questLog.start());

  $effect(() => {
    if (questLog.open) void questLog.loadLib();
  });

  function onDoc(e: MouseEvent) {
    if (questLog.open && root && !root.contains(e.target as Node)) questLog.open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (questLog.open && e.key === "Escape") questLog.open = false;
  }
  function browse() {
    questLog.open = false;
    library.questsOnly = true;
    library.shelf = "all";
    library.cat = "all";
    library.read(null);
    ui.go("library");
  }
  const checking = (q: Quest) => q.hostId != null && checks.busy === checks.key(q.articleId, q.hostId);
</script>

<svelte:document onmousedown={onDoc} onkeydown={onKey} />

<div class="qwrap" bind:this={root}>
  <button
    type="button"
    class="ind qbtn"
    class:on={questLog.open}
    class:live={active.length > 0}
    data-guide="quests"
    title="Quest log: Library guides you started"
    aria-haspopup="dialog"
    aria-expanded={questLog.open}
    onclick={() => (questLog.open = !questLog.open)}
    ><span class="glyph" aria-hidden="true">◆</span> QUESTS{#if active.length}<b class="count">{active.length}</b>{/if}</button
  >
  {#if questLog.open}
    <div class="pop" role="dialog" aria-label="Quest log">
      <div class="head">
        <span class="label"><span use:scramble={"QUEST LOG"}></span></span>
        <span class="sp"></span>
        <button type="button" class="link small" onclick={browse}>Find a quest…</button>
      </div>
      <div class="list">
        {#each active as q (q.id)}
          {@const p = questLog.progress(q)}
          {@const host = questLog.hostName(q)}
          <div class="quest">
            <button type="button" class="open" onclick={() => questLog.show(q.articleId, q.hostId)} title="Open the guide">
              <span class="qt">{q.title}</span>
              <span class="where">{host ? `◉ ${host}` : "no host"} · objective {Math.min(p.objectives.done + 1, p.objectives.total || 1)}/{p.objectives.total || "?"} · {p.done}/{p.total} steps</span>
              <span class="bar" aria-hidden="true"><i style:width={`${p.total ? (p.done / p.total) * 100 : 0}%`}></i></span>
              {#if p.current}
                <span class="obj">{p.current.partTitle}</span>
                <span class="now"><span class="diamond" aria-hidden="true">◇</span> {p.current.text}</span>
              {:else if !questLog.lib}
                <span class="now muted">Loading…</span>
              {:else if !p.total}
                <span class="now muted">This guide changed and has no steps now.</span>
              {/if}
            </button>
            <div class="acts">
              {#if q.hostId != null && questLog.pendingChecks(q).length}
                <button type="button" class="link small" disabled={checks.busy !== null} title="Runs the guide's read-only checks on {host}; steps that pass complete" onclick={() => void questLog.verify(q)}>{checking(q) ? "Checking…" : "Check now"}</button>
              {/if}
              <button type="button" class="link small dim" onclick={() => void questLog.abandon(q)}>Abandon</button>
            </div>
          </div>
        {:else}
          <div class="empty">
            <p>No active quests.</p>
            <p class="muted small">Guides with steps (marked <b class="tag">◇ QUEST</b> in the Library) can be started as a quest: open one and press <b>▶ START QUEST</b>. The log then shows the step you're on, per host, and a quest update appears when a step is done.</p>
            <button type="button" class="btn tiny" onclick={browse}>BROWSE QUEST GUIDES</button>
          </div>
        {/each}
        {#if done.length}
          <button type="button" class="link small donehead" onclick={() => (showDone = !showDone)}>{showDone ? "▾" : "▸"} Completed ({done.length})</button>
          {#if showDone}
            {#each done as q (q.id)}
              {@const host = questLog.hostName(q)}
              <div class="quest done">
                <button type="button" class="open" onclick={() => questLog.show(q.articleId, q.hostId)}>
                  <span class="qt">◆ {q.title}</span>
                  <span class="where">{host ? `◉ ${host}` : "no host"} · done {new Date((q.completedAt ?? 0) * 1000).toLocaleDateString()}</span>
                </button>
                <div class="acts"><button type="button" class="link small dim" onclick={() => void questLog.abandon(q)}>Remove</button></div>
              </div>
            {/each}
          {/if}
        {/if}
      </div>
      {#if questLog.error}<p class="error-text small pad">{questLog.error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .qwrap {
    position: relative;
  }
  .qbtn {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .qbtn:focus-visible {
    outline: 1px solid var(--accent);
  }
  .qbtn.on {
    color: var(--magenta);
  }
  .glyph {
    color: var(--text-2);
  }
  .qbtn.live .glyph {
    color: var(--warn);
  }
  .count {
    padding: 0 4px;
    background: var(--warn);
    color: var(--bg-0);
    font-weight: 600;
  }
  .pop {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    width: min(440px, 90vw);
    max-height: min(600px, 75vh);
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid color-mix(in srgb, var(--warn) 50%, var(--line));
    border-top: 2px solid var(--warn);
    box-shadow: 0 10px 30px rgb(0 0 0 / 45%);
    z-index: 60;
    letter-spacing: normal;
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - 14px), calc(100% - 14px) 100%, 0 100%);
  }
  .head {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line);
  }
  .label {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--warn);
  }
  .sp {
    flex: 1;
  }
  .list {
    overflow: auto;
    min-height: 0;
  }
  .quest {
    display: flex;
    gap: 8px;
    padding: 9px 10px;
    border-bottom: 1px solid var(--line);
    border-left: 3px solid var(--warn);
  }
  .quest.done {
    border-left-color: var(--mint);
    opacity: 0.75;
  }
  .open {
    all: unset;
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    cursor: pointer;
  }
  .open:focus-visible {
    outline: 1px solid var(--accent);
  }
  .quest:hover {
    background: color-mix(in srgb, var(--warn) 6%, transparent);
  }
  .qt {
    color: var(--text-0);
    font-weight: 600;
  }
  .where,
  .obj {
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: var(--tracking-label);
  }
  .obj {
    color: var(--warn);
    margin-top: 3px;
    text-transform: uppercase;
  }
  .now {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .diamond {
    color: var(--warn);
  }
  .bar {
    height: 3px;
    background: color-mix(in srgb, var(--line) 70%, transparent);
    position: relative;
  }
  .bar i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--warn);
  }
  .acts {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    flex: none;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
  }
  .link:disabled {
    color: var(--text-2);
    cursor: default;
  }
  .link.dim {
    color: var(--text-2);
  }
  .link.dim:hover {
    color: var(--danger);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .donehead {
    display: block;
    padding: 8px 10px;
  }
  .empty {
    padding: 12px 10px;
  }
  .empty p {
    margin: 0 0 8px;
  }
  .tag {
    color: var(--warn);
    font-weight: 400;
  }
  .pad {
    padding: 6px 10px;
    margin: 0;
  }
</style>
