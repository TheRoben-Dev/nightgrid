<script lang="ts">
  import { api, errorText, type LogEvent, type LogLine } from "../../api";

  interface Props {
    hostId: number;
    containerId: string;
    running: boolean;
  }
  let { hostId, containerId, running }: Props = $props();

  /** Lines kept in memory; older ones drop off the top. */
  const MAX_LINES = 20_000;
  const ROW = 18;

  let lines = $state.raw<LogLine[]>([]);
  let follow = $state(true);
  let timestamps = $state(true);
  let tail = $state("500");
  let query = $state("");
  let onlyMatches = $state(true);
  let stickBottom = $state(true);
  let ended = $state<string | null>(null);
  let error = $state<string | null>(null);

  let viewport: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let height = $state(300);

  // (Re)start the stream whenever the container or options change.
  $effect(() => {
    const opts = { follow: follow && running, tail: tail === "all" ? null : Number(tail), since: null };
    const id = containerId;
    const host = hostId;
    lines = [];
    ended = null;
    error = null;
    stickBottom = true;
    let stream: number | null = null;
    let stopped = false;
    let pending: LogLine[] = [];
    let frame = 0;
    // Batches from the backend are merged per animation frame.
    const flush = () => {
      frame = 0;
      if (!pending.length) return;
      const next = lines.length + pending.length > MAX_LINES ? [...lines, ...pending].slice(-MAX_LINES) : [...lines, ...pending];
      pending = [];
      lines = next;
    };
    const on = (e: LogEvent) => {
      if (stopped) return;
      if (e.kind === "lines") {
        pending.push(...e.lines);
        if (!frame) frame = requestAnimationFrame(flush);
      } else {
        flush();
        ended = e.error ?? (opts.follow ? "Stream ended (container stopped)." : "End of log.");
        if (e.error) error = e.error;
      }
    };
    api
      .dockerLogsStart(host, id, opts, on)
      .then((s) => {
        stream = s;
        if (stopped) void api.dockerLogsStop(s);
      })
      .catch((e) => (error = errorText(e)));
    return () => {
      stopped = true;
      if (frame) cancelAnimationFrame(frame);
      if (stream !== null) void api.dockerLogsStop(stream);
    };
  });

  const q = $derived(query.trim().toLowerCase());
  const shown = $derived(q && onlyMatches ? lines.filter((l) => l.text.toLowerCase().includes(q)) : lines);
  const matches = $derived(q ? (onlyMatches ? shown.length : lines.filter((l) => l.text.toLowerCase().includes(q)).length) : 0);

  // Virtual window: only rows near the viewport are in the DOM.
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 20));
  const last = $derived(Math.min(shown.length, Math.ceil((scrollTop + height) / ROW) + 20));
  const windowed = $derived(shown.slice(first, last));

  $effect(() => {
    void shown.length;
    if (stickBottom && viewport) queueMicrotask(() => viewport && (viewport.scrollTop = viewport.scrollHeight));
  });

  function onscroll() {
    if (!viewport) return;
    scrollTop = viewport.scrollTop;
    stickBottom = viewport.scrollTop + viewport.clientHeight >= viewport.scrollHeight - ROW * 2;
  }

  function jumpEnd() {
    stickBottom = true;
    if (viewport) viewport.scrollTop = viewport.scrollHeight;
  }

  const fmtTs = (ts: string | null) => (ts ? ts.slice(0, 10) + " " + ts.slice(11, 23) : "");

  function parts(text: string): { t: string; hit: boolean }[] {
    if (!q) return [{ t: text, hit: false }];
    const out: { t: string; hit: boolean }[] = [];
    const lower = text.toLowerCase();
    let i = 0;
    for (;;) {
      const j = lower.indexOf(q, i);
      if (j < 0) break;
      if (j > i) out.push({ t: text.slice(i, j), hit: false });
      out.push({ t: text.slice(j, j + q.length), hit: true });
      i = j + q.length;
    }
    if (i < text.length) out.push({ t: text.slice(i), hit: false });
    return out;
  }

  function copyVisible() {
    const text = shown.map((l) => (timestamps && l.ts ? `${l.ts} ${l.text}` : l.text)).join("\n");
    void navigator.clipboard?.writeText(text);
  }
</script>

<div class="logs">
  <div class="bar">
    <input class="input search" type="search" placeholder="Search logs" bind:value={query} aria-label="Search logs" />
    {#if q}<span class="count">{matches} match{matches === 1 ? "" : "es"}</span>{/if}
    <label><input type="checkbox" bind:checked={onlyMatches} /> only matches</label>
    <span class="spacer"></span>
    <label title={running ? "" : "Container is not running"}>
      <input type="checkbox" bind:checked={follow} disabled={!running} /> follow
    </label>
    <label><input type="checkbox" bind:checked={timestamps} /> timestamps</label>
    <label
      >tail
      <select class="input" bind:value={tail} aria-label="Lines from the end">
        <option value="100">100</option>
        <option value="500">500</option>
        <option value="2000">2000</option>
        <option value="10000">10000</option>
        <option value="all">all</option>
      </select>
    </label>
    <button type="button" class="btn ghost tiny" onclick={copyVisible} title="Copy the lines shown">Copy</button>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (focusable so the log scrolls with the keyboard) -->
  <div
    class="viewport"
    bind:this={viewport}
    bind:clientHeight={height}
    {onscroll}
    role="log"
    aria-live="off"
    aria-label="Container log"
    tabindex="0"
  >
    <div class="spacer-rows" style:height="{shown.length * ROW}px">
      <div class="rows" style:transform="translateY({first * ROW}px)">
        {#each windowed as l, i (first + i)}
          <div class="row" class:err={l.stream === "err"}>
            {#if timestamps}<span class="ts">{fmtTs(l.ts)}</span>{/if}<span class="txt"
              >{#each parts(l.text) as p, k (k)}{#if p.hit}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</span
            >
          </div>
        {/each}
      </div>
    </div>
    {#if !lines.length && !error}
      <p class="muted empty">{ended ? "No log output." : "Waiting for output…"}</p>
    {/if}
  </div>
  <div class="foot">
    <span class="muted">{lines.length.toLocaleString()} lines{lines.length >= MAX_LINES ? " (oldest dropped)" : ""}</span>
    {#if follow && running && !ended}<span class="live">● LIVE</span>{/if}
    {#if error}<span class="error-text">{error}</span>{:else if ended}<span class="muted">{ended}</span>{/if}
    <span class="spacer"></span>
    {#if !stickBottom}<button type="button" class="btn ghost tiny" onclick={jumpEnd}>Jump to end ↓</button>{/if}
  </div>
</div>

<style>
  .logs {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .bar,
  .foot {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    font-size: var(--fs-xs);
    color: var(--text-1);
    flex-wrap: wrap;
  }
  .bar {
    border-bottom: 1px solid var(--line);
  }
  .foot {
    border-top: 1px solid var(--line);
  }
  .search {
    width: 200px;
  }
  label {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .count {
    color: var(--cyan);
  }
  .spacer {
    flex: 1;
  }
  .viewport {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--bg-0);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .viewport:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
  .spacer-rows {
    position: relative;
    min-width: max-content;
  }
  .rows {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
  }
  .row {
    height: 18px;
    line-height: 18px;
    white-space: pre;
    padding: 0 8px;
    color: var(--text-0);
  }
  .row.err .txt {
    color: var(--danger);
  }
  .ts {
    color: var(--text-1);
    margin-right: 10px;
    opacity: 0.8;
  }
  mark {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
    color: var(--text-0);
  }
  .empty {
    position: absolute;
    top: 8px;
    left: 10px;
    margin: 0;
  }
  .live {
    color: var(--mint);
    letter-spacing: 0.1em;
  }
  .tiny {
    padding: 2px 10px;
    font-size: var(--fs-xs);
  }
</style>
