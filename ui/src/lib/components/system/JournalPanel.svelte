<script lang="ts">
  import { tick } from "svelte";
  import { PRIORITIES, prioLabel, prioTone, system } from "../../stores/system.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { platform } from "../../stores/platform.svelte";

  // Windows hosts show the Event Log: the unit filter is an event source.
  const onWindows = $derived(platform.isWindows(system.hostId === null ? undefined : hosts.byId(system.hostId)));

  /** Newest journal lines handed to DRIFT. */
  const EXPLAIN_LINES = 150;

  const rows = $derived(system.visibleJournal);
  const f = $derived(system.journalForm);
  let box: HTMLDivElement | undefined = $state();
  let pinned = $state(true);

  // Stay at the bottom while following, unless the user scrolled up.
  $effect(() => {
    void rows.length;
    if (!pinned || !box) return;
    void tick().then(() => box && (box.scrollTop = box.scrollHeight));
  });

  function onScroll() {
    if (!box) return;
    pinned = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
  }

  const fmt = new Intl.DateTimeFormat(undefined, { month: "short", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
  const time = (ts: number) => fmt.format(new Date(ts));

  function apply(e: Event) {
    e.preventDefault();
    pinned = true;
    void system.startJournal();
  }

  const SINCE = [
    { v: "", label: "any time" },
    { v: "-15min", label: "15 min" },
    { v: "-1h", label: "1 hour" },
    { v: "-6h", label: "6 hours" },
    { v: "today", label: "today" },
    { v: "yesterday", label: "since yesterday" },
    { v: "-7d", label: "7 days" },
  ];
  const explainText = () =>
    rows
      .slice(-EXPLAIN_LINES)
      .map((e) => `${time(e.ts)} ${prioLabel(e.priority)} ${e.ident ?? e.unit ?? ""}${e.pid ? `[${e.pid}]` : ""}: ${e.message}`)
      .join("\n");
  const explainSource = $derived(`journal${f.unit.trim() ? `: ${f.unit.trim()}` : ""}`);

  let customSince = $state(!SINCE.some((s) => s.v === system.journalForm.since) || !!system.journalForm.until);
</script>

<div class="journal">
  <form class="bar" onsubmit={apply}>
    <label class="fld">
      <span class="label">{onWindows ? "SOURCE" : "UNIT"}</span>
      <input class="input unit" placeholder={onWindows ? "all sources" : "all units"} bind:value={system.journalForm.unit} autocomplete="off" spellcheck="false" list="journal-units" />
      <datalist id="journal-units">
        {#if onWindows}
          {#each [...new Set(system.journal.map((e) => e.ident ?? ""))].filter(Boolean).slice(0, 200) as src (src)}<option value={src}></option>{/each}
        {:else}
          {#each system.units as u (u.name)}<option value={u.name}></option>{/each}
        {/if}
      </datalist>
    </label>
    <label class="fld">
      <span class="label">LEVEL</span>
      <select class="input" bind:value={system.journalForm.priority}>
        {#each PRIORITIES as p, i (p)}<option value={i}>{i === 7 ? "everything" : `${p} and worse`}</option>{/each}
      </select>
    </label>
    <label class="fld">
      <span class="label">SINCE</span>
      <select
        class="input"
        value={customSince ? "custom" : f.since}
        onchange={(e) => {
          const v = e.currentTarget.value;
          customSince = v === "custom";
          if (!customSince) system.journalForm = { ...system.journalForm, since: v, until: "" };
        }}
      >
        {#each SINCE as s (s.v)}<option value={s.v}>{s.label}</option>{/each}
        <option value="custom">custom…</option>
      </select>
    </label>
    {#if customSince}
      <input class="input time" placeholder="from: 2026-11-14 08:00" bind:value={system.journalForm.since} aria-label="Since" />
      <input class="input time" placeholder="until (optional)" bind:value={system.journalForm.until} aria-label="Until" />
    {/if}
    <label class="fld">
      <span class="label">LINES</span>
      <select class="input" bind:value={system.journalForm.lines}>
        {#each [200, 500, 2000, 5000, 10000] as n (n)}<option value={n}>{n}</option>{/each}
      </select>
    </label>
    <label class="chk"><input type="checkbox" bind:checked={system.journalForm.follow} disabled={!!f.until.trim()} /> follow</label>
    <button type="submit" class="btn primary tiny">Apply</button>
    {#if system.journalRun !== null}
      <button type="button" class="btn ghost tiny" onclick={() => system.stopJournal()}>Stop</button>
    {/if}
    {#if onWindows}
      <label class="chk" title="The Security log needs an administrator account">
        <input type="checkbox" checked={system.journalElevated} onchange={(e) => system.startJournal(e.currentTarget.checked)} /> Security log
      </label>
    {/if}
    <span class="spacer"></span>
    <input class="input filter" type="search" placeholder="Find in loaded lines" bind:value={system.journalQuery} aria-label="Find in journal" />
  </form>

  {#if system.journalLimited && !system.journalElevated}
    <p class="hint" role="status">
      Only your own entries are visible: system services log as other users. Join <code>systemd-journal</code> on the host,
      or
      <button type="button" class="btn ghost tiny" onclick={() => system.startJournal(true)}>Read with sudo</button>
    </p>
  {/if}
  {#each system.journalNotices as n, i (i)}<p class="hint warn">{n}</p>{/each}
  {#if system.errors.journal}<p class="error-text">{system.errors.journal}</p>{/if}

  <div class="log" bind:this={box} onscroll={onScroll} role="log" aria-label="Journal" aria-live="off" data-guide="system-journal">
    {#if system.journalLoading && !rows.length}
      <p class="muted pad">Reading the journal…</p>
    {:else if !rows.length}
      <p class="muted pad">{system.journal.length ? "No lines match." : "No entries."}</p>
    {/if}
    {#each rows as e, i (i)}
      <div class="line {prioTone(e.priority)}">
        <span class="ts">{time(e.ts)}</span>
        <span class="prio">{prioLabel(e.priority)}</span>
        <span class="who" title={e.unit ?? ""}>{e.ident ?? e.unit ?? ""}{e.pid ? `[${e.pid}]` : ""}</span>
        <span class="msg">{e.message}</span>
      </div>
    {/each}
  </div>
  <div class="foot">
    <span>{rows.length}{rows.length !== system.journal.length ? ` / ${system.journal.length}` : ""} LINES</span>
    {#if system.journalRun !== null && f.follow}<span class="live">● FOLLOWING</span>{/if}
    {#if system.journalElevated}<span class="mint">SUDO</span>{/if}
    <span class="spacer"></span>
    {#if rows.length}<ExplainButton source={explainSource} text={explainText} within={box} question="Explain these journal entries. Is anything wrong, and how do I fix it?" />{/if}
    {#if !pinned}
      <button
        type="button"
        class="btn ghost tiny"
        onclick={() => {
          pinned = true;
          if (box) box.scrollTop = box.scrollHeight;
        }}>Jump to end</button
      >
    {/if}
  </div>
</div>

<style>
  .journal {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .foot {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .fld {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .unit {
    width: 170px;
  }
  .time {
    width: 140px;
  }
  .filter {
    width: 200px;
  }
  .chk {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .spacer {
    flex: 1;
  }
  .hint {
    margin: 0;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border: 1px dashed var(--line);
    color: var(--text-1);
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .hint.warn {
    color: var(--warn);
  }
  code {
    color: var(--cyan);
  }
  .log {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    padding: 4px 0;
  }
  .pad {
    padding: 8px 10px;
    margin: 0;
    font-family: var(--font-ui, inherit);
  }
  .line {
    display: grid;
    grid-template-columns: 130px 52px minmax(80px, 180px) 1fr;
    gap: 10px;
    padding: 1px 10px;
    line-height: 1.45;
  }
  .line:hover {
    background: var(--bg-1);
  }
  .ts {
    color: var(--text-1);
  }
  .prio {
    color: var(--text-1);
    letter-spacing: 0.06em;
  }
  .who {
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg {
    color: var(--text-0);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .line.bad .prio,
  .line.bad .msg {
    color: var(--danger);
  }
  .line.warn .prio,
  .line.warn .msg {
    color: var(--warn);
  }
  .line.note .prio {
    color: var(--mint);
  }
  .line.dim .msg {
    color: var(--text-1);
  }
  .foot {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .live {
    color: var(--ok);
  }
  .mint {
    color: var(--mint);
  }
</style>
