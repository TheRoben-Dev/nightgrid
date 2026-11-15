<script lang="ts">
  /**
   * System → TIMELINE (M10l): what happened on the host, newest first.
   * Nightgrid's own records (alerts, power, compose, upgrades, automation)
   * merged with the host's logs (boots, failed units, OOM kills, package
   * transactions) and Docker's container events. DRIFT can summarise it.
   */
  import { SvelteSet } from "svelte/reactivity";
  import type { HostEvent } from "../../api";
  import { system } from "../../stores/system.svelte";
  import { byDay, clock, dayLabel, FILTERS, filterEvents, KIND_LABEL, RANGES, tally, timelineText, type FilterId } from "../../timeline";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import Modal from "../Modal.svelte";

  const t = $derived(system.timeline);
  const on = new SvelteSet<FilterId>(FILTERS.map((f) => f.id));
  let query = $state("");
  let problems = $state(false);
  let confirmClear = $state(false);

  const shown = $derived(t ? filterEvents(t.events, on, query, problems) : []);
  const days = $derived(byDay(shown));
  const counts = $derived(tally(t?.events ?? []));

  function toggle(id: FilterId) {
    if (on.has(id)) on.delete(id);
    else on.add(id);
  }

  const rangeLabel = $derived(RANGES.find((r) => r.days === system.timelineDays)?.label.toLowerCase().replace(" ", "") ?? `${system.timelineDays}d`);
  const host = $derived(system.host?.name ?? "this host");
  const title = (e: HostEvent) => (e.origin === "nightgrid" ? "Recorded by Nightgrid" : "From the host's own logs");
</script>

<div class="timeline">
  <div class="bar">
    <span class="ranges" role="group" aria-label="Time range" data-guide="timeline-range">
      {#each RANGES as r (r.days)}
        <button type="button" class:on={system.timelineDays === r.days} aria-pressed={system.timelineDays === r.days} onclick={() => system.loadTimeline(r.days)}
          >{r.label}</button
        >
      {/each}
    </span>
    {#if t}
      <span class="counts">
        <span class="mint">{t.events.length}</span> EVENTS{#if counts.critical}<span class="bad">&nbsp;· {counts.critical} CRITICAL</span>{/if}{#if counts.warn}<span
            class="warn">&nbsp;· {counts.warn} WARN</span
          >{/if}
      </span>
    {/if}
    <span class="spacer"></span>
    <input class="input filter" type="search" placeholder="Filter" bind:value={query} aria-label="Filter events" />
    {#if t && shown.length}
      <span data-guide="timeline-summary">
        <ExplainButton
          source="incident timeline of {host}, last {rangeLabel}"
          host={system.host?.name ?? null}
          label="Summarize"
          text={() => timelineText(shown)}
          question="Summarize what happened on this host in this period: the incidents, their likely causes and how they connect (for example a reboot after an upgrade, or a container crash before an alert). What still needs attention?"
        />
      </span>
    {/if}
    <button type="button" class="btn ghost tiny" onclick={() => system.loadTimeline()} disabled={system.timelineLoading}>{system.timelineLoading ? "…" : "Refresh"}</button>
  </div>

  <div class="chips" role="group" aria-label="Show" data-guide="timeline-filters">
    {#each FILTERS as f (f.id)}
      <button type="button" class="chip" class:on={on.has(f.id)} aria-pressed={on.has(f.id)} onclick={() => toggle(f.id)}>{f.label}</button>
    {/each}
    <button type="button" class="chip" class:on={problems} aria-pressed={problems} onclick={() => (problems = !problems)}>Problems only</button>
  </div>

  <div class="scroll" data-guide="system-timeline">
    {#if system.errors.timeline}
      <p class="error-text pad">{system.errors.timeline}</p>
    {:else if !t}
      <p class="muted pad">{system.timelineLoading ? "Reading the host's history…" : ""}</p>
    {:else}
      {#each t.notes as n (n)}<p class="muted small pad note">{n}</p>{/each}
      {#if !shown.length}
        <p class="muted pad">{t.events.length ? "Nothing matches." : `Nothing recorded in the last ${rangeLabel}.`}</p>
      {/if}
      {#each days as d (d.day)}
        <section>
          <h3 class="label day">// {dayLabel(d.day)}</h3>
          <ol class="list">
            {#each d.events as e, i (`${e.at}|${e.title}|${i}`)}
              <li class="ev {e.level}" title={title(e)}>
                <time datetime={new Date(e.at * 1000).toISOString()}>{clock(e.at)}</time>
                <span class="kind">{KIND_LABEL[e.kind]}</span>
                <span class="what">
                  <span class="t">{e.title}</span>
                  {#if e.detail}<span class="muted small">{e.detail}</span>{/if}
                </span>
                <span class="src" aria-label={title(e)}>{e.origin === "nightgrid" ? "NG" : "HOST"}</span>
              </li>
            {/each}
          </ol>
        </section>
      {/each}
      <p class="muted small pad foot">
        NG entries are what Nightgrid saw or did (kept 90 days on this computer). HOST entries come from the host's journal, package logs and Docker, read
        when you open this tab; Docker only remembers recent events.
        <button type="button" class="btn ghost tiny" onclick={() => (confirmClear = true)}>Clear recorded history…</button>
      </p>
    {/if}
  </div>
</div>

<Modal open={confirmClear} title="CLEAR RECORDED HISTORY" tone="danger" width={520} onclose={() => (confirmClear = false)}>
  <p>
    Deletes everything Nightgrid recorded for <strong>{host}</strong>: alerts, power actions, compose runs, upgrades and automation runs. The host's own logs aren't
    touched.
  </p>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (confirmClear = false)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      onclick={() => {
        confirmClear = false;
        void system.clearTimeline();
      }}>Clear {host}'s history</button
    >
  {/snippet}
</Modal>

<style>
  .timeline {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .chips {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .chips {
    gap: 4px;
  }
  .ranges {
    display: inline-flex;
  }
  .ranges button,
  .chip {
    border: 1px solid var(--line);
    background: none;
    color: var(--text-1);
    font-size: var(--fs-xs);
    padding: 2px 8px;
    cursor: pointer;
    letter-spacing: 0.08em;
  }
  .ranges button + button {
    border-left: none;
  }
  .ranges button.on,
  .chip.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .counts {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .mint {
    color: var(--mint);
  }
  .counts .bad {
    color: var(--danger);
  }
  .counts .warn {
    color: var(--warn);
  }
  .spacer {
    flex: 1;
  }
  .filter {
    width: 180px;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  .pad {
    margin: 4px 10px 8px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .note {
    border-left: 2px solid var(--warn);
    padding-left: 8px;
  }
  .day {
    margin: 10px 10px 4px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0 10px;
  }
  .ev {
    display: grid;
    grid-template-columns: 44px 92px 1fr auto;
    gap: 10px;
    align-items: baseline;
    padding: 3px 0 3px 8px;
    border-left: 2px solid var(--line);
    font-size: var(--fs-sm);
  }
  .ev.warn {
    border-left-color: var(--warn);
  }
  .ev.critical {
    border-left-color: var(--danger);
  }
  .ev.ok {
    border-left-color: var(--mint);
  }
  time {
    color: var(--text-1);
    font-variant-numeric: tabular-nums;
  }
  .kind {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--cyan);
  }
  .ev.warn .kind {
    color: var(--warn);
  }
  .ev.critical .kind {
    color: var(--danger);
  }
  .ev.ok .kind {
    color: var(--mint);
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .what .t {
    overflow-wrap: anywhere;
  }
  .src {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .foot {
    margin-top: 14px;
  }
</style>
