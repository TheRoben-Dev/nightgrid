<script lang="ts">
  /**
   * Fleet → REPLAY (1.1, D-173): step through an outage or a burst of alerts.
   * Metrics, alerts, restarts, OOM kills, deploys, journal changes and
   * notices on one scrubber; logs for the window on request.
   */
  import { onMount } from "svelte";
  import { api, errorText, type Incident, type LogHit } from "../../api";
  import { clock, duration, eventsUntil, pointAt, pos } from "../../fleet11";
  import { replay } from "../../stores/fleet11.svelte";

  onMount(() => {
    if (!replay.loaded) void replay.load();
  });

  const r = $derived(replay.replay);
  const p = $derived(r ? pointAt(r.metrics, replay.at) : null);
  const seen = $derived(r ? eventsUntil(r, replay.at) : []);
  let logs = $state<LogHit[] | null>(null);
  let logsBusy = $state(false);
  let logsNote = $state<string | null>(null);

  // Play: one minute of the window per tick.
  $effect(() => {
    if (!replay.playing || !r) return;
    const step = Math.max(60, Math.round((r.to - r.from) / 240));
    const t = setInterval(() => {
      if (replay.at >= r.to) replay.playing = false;
      else replay.at = Math.min(r.to, replay.at + step);
    }, 120);
    return () => clearInterval(t);
  });

  function pick(i: Incident) {
    logs = null;
    logsNote = null;
    void replay.open(i);
  }

  function line(key: "cpu" | "mem"): string {
    if (!r || !r.metrics.length) return "";
    return r.metrics
      .filter((m) => m[key] != null)
      .map((m) => `${(pos(r, m.ts) * 1000).toFixed(1)},${(100 - (m[key] ?? 0)).toFixed(1)}`)
      .join(" ");
  }

  async function readLogs() {
    if (!r || r.incident.hostId == null) return;
    const mins = Math.ceil((Date.now() / 1000 - r.from) / 60);
    if (mins > 7 * 24 * 60) {
      logsNote = "Log search reaches back 7 days; this incident is older.";
      return;
    }
    logsBusy = true;
    logsNote = null;
    try {
      const res = await api.logSearch({ text: "", regex: false, caseSensitive: false, hosts: [r.incident.hostId], journal: true, containers: true, unit: null, container: null, priority: 4, sinceMins: mins });
      logs = res.hits.filter((h) => (h.ts ?? 0) >= r.from && (h.ts ?? 0) <= r.to).sort((a, b) => (a.ts ?? 0) - (b.ts ?? 0));
      if (res.errors.length) logsNote = res.errors.map((e) => e.message).join("; ");
    } catch (e) {
      logsNote = errorText(e);
    } finally {
      logsBusy = false;
    }
  }
  const tone = (l: string) => (l === "critical" ? "bad" : l === "warn" ? "warn" : l === "ok" ? "ok" : "muted");
  const srcWord: Record<string, string> = { event: "EVENT", change: "CHANGE", outage: "UPTIME", notice: "NOTICE" };
</script>

<div class="rep" data-guide="fleet-replay">
  <div class="split">
    <div class="list">
      <div class="row pad">
        <span class="label">LAST</span>
        {#each [7, 14, 30, 90] as d (d)}<button type="button" class="btn tiny" class:primary={replay.days === d} onclick={() => replay.load(d)}>{d} d</button>{/each}
      </div>
      {#each replay.incidents as i (`${i.kind}:${i.start}:${i.title}`)}
        <button type="button" class="irow" class:on={r?.incident.start === i.start && r?.incident.title === i.title} onclick={() => pick(i)}>
          <span class="lv {tone(i.level)}">{i.kind === "outage" ? "DOWN" : "ALERTS"}</span>
          <span class="in"><span class="name">{i.title}</span><span class="muted small">{new Date(i.start * 1000).toLocaleString()} · {duration(i.end - i.start)}</span></span>
        </button>
      {:else}
        <p class="muted small pad">{replay.loaded ? "No outages or alerts in this period." : "Reading…"}</p>
      {/each}
    </div>

    <div class="detail">
      {#if replay.error}<p class="error-text small">{replay.error}</p>{/if}
      {#if r}
        <div class="row">
          <span class="in"><span class="name">{r.incident.title}</span><span class="muted small">{r.incident.detail}</span></span>
          <span class="sp"></span>
          <button type="button" class="btn tiny" disabled={logsBusy || r.incident.hostId == null} onclick={readLogs} data-guide="replay-logs">{logsBusy ? "Reading logs…" : "Logs for this window"}</button>
          <button type="button" class="btn tiny ghost" onclick={() => replay.export()}>Export</button>
        </div>

        <div class="scrub">
          <svg viewBox="0 0 1000 100" preserveAspectRatio="none" class="chart" aria-hidden="true">
            <rect class="span" x={pos(r, r.incident.start) * 1000} width={Math.max(2, (pos(r, r.incident.end) - pos(r, r.incident.start)) * 1000)} y="0" height="100" />
            <polyline class="cpu" points={line("cpu")} />
            <polyline class="mem" points={line("mem")} />
            {#each r.events as e (e.at + e.title)}<line class="tick {tone(e.level)}" x1={pos(r, e.at) * 1000} x2={pos(r, e.at) * 1000} y1="88" y2="100" />{/each}
            <line class="cursor" x1={pos(r, replay.at) * 1000} x2={pos(r, replay.at) * 1000} y1="0" y2="100" />
          </svg>
          <input type="range" min={r.from} max={r.to} step="30" bind:value={replay.at} aria-label="Time" data-guide="replay-scrub" />
          <div class="row tight small">
            <button type="button" class="btn tiny" onclick={() => (replay.at >= r.to && (replay.at = r.from), (replay.playing = !replay.playing))}>{replay.playing ? "❚❚ Pause" : "▶ Play"}</button>
            <span class="data">{clock(replay.at)}</span>
            <span class="muted">{replay.at < r.incident.start ? `${duration(r.incident.start - replay.at)} before` : replay.at <= r.incident.end ? "during" : `${duration(replay.at - r.incident.end)} after`}</span>
            <span class="sp"></span>
            <span class="leg cpu">CPU {p?.cpu != null ? `${Math.round(p.cpu)}%` : "--"}</span>
            <span class="leg mem">MEM {p?.mem != null ? `${Math.round(p.mem)}%` : "--"}</span>
            <span class="muted">load {p?.load1 != null ? p.load1.toFixed(2) : "--"}</span>
          </div>
          {#if !r.metrics.length}<p class="muted small">No metrics were recorded for this window (the host wasn't monitored then).</p>{/if}
        </div>

        <div class="events">
          {#each r.events as e (e.at + e.title)}
            <button type="button" class="ev" class:past={seen.includes(e)} onclick={() => (replay.at = e.at)}>
              <span class="data small">{clock(e.at)}</span>
              <span class="src small {tone(e.level)}">{srcWord[e.source]}</span>
              <span class="et"><span>{e.title}</span>{#if e.detail}<span class="muted small">{e.detail}</span>{/if}</span>
            </button>
          {:else}
            <p class="muted small">Nothing else was recorded in this window.</p>
          {/each}
        </div>

        {#if logsNote}<p class="muted small">{logsNote}</p>{/if}
        {#if logs}
          <h3 class="hd">WARNINGS AND ERRORS IN THE LOGS ({logs.length})</h3>
          <div class="logs">
            {#each logs as l, i (i)}
              <div class="log small" class:past={(l.ts ?? 0) <= replay.at}><span class="data">{clock(l.ts ?? 0)}</span><span class="muted">{l.source}</span><span class="data">{l.text}</span></div>
            {:else}
              <p class="muted small">No warnings or errors in the journal or container logs in this window.</p>
            {/each}
          </div>
        {/if}
      {:else}
        <p class="muted pad">Pick an outage or an alert burst to replay it: the host's CPU and memory, alerts, container restarts, OOM kills, deploys and other changes, minute by minute from 30 minutes before to 30 minutes after.</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .rep {
    height: 100%;
    min-height: 0;
  }
  .split {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(240px, 1fr) 2.4fr;
    gap: 10px;
  }
  .list,
  .detail {
    overflow: auto;
    min-height: 0;
    border: 1px solid var(--line);
  }
  .detail {
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .row.tight {
    gap: 8px;
  }
  .sp {
    flex: 1;
  }
  .irow {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 7px 10px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .irow:hover,
  .irow:focus-visible,
  .irow.on {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .irow.on {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .lv,
  .src {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    width: 54px;
    flex: none;
  }
  .in {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    color: var(--text-0);
  }
  .scrub {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .chart {
    width: 100%;
    height: 110px;
    border: 1px solid var(--line);
  }
  .span {
    fill: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  polyline {
    fill: none;
    stroke-width: 1.4;
    vector-effect: non-scaling-stroke;
  }
  .cpu {
    stroke: var(--cyan);
    color: var(--cyan);
  }
  .mem {
    stroke: var(--magenta);
    color: var(--magenta);
  }
  .tick {
    stroke: var(--text-2);
    vector-effect: non-scaling-stroke;
  }
  .tick.bad {
    stroke: var(--danger);
  }
  .tick.warn {
    stroke: var(--warn);
  }
  .cursor {
    stroke: var(--text-0);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  input[type="range"] {
    width: 100%;
  }
  .leg {
    font-family: var(--font-display);
  }
  .events {
    display: flex;
    flex-direction: column;
  }
  .ev {
    all: unset;
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 48px 62px 1fr;
    gap: 8px;
    padding: 4px 0;
    border-bottom: 1px solid var(--line);
    opacity: 0.45;
    cursor: pointer;
  }
  .ev.past {
    opacity: 1;
  }
  .et {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 6px 0 0;
  }
  .logs {
    max-height: 260px;
    overflow: auto;
    border: 1px solid var(--line);
  }
  .log {
    display: grid;
    grid-template-columns: 48px minmax(80px, 160px) 1fr;
    gap: 8px;
    padding: 2px 6px;
    opacity: 0.5;
  }
  .log.past {
    opacity: 1;
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn);
  }
  .bad {
    color: var(--danger);
  }
  .muted {
    color: var(--text-2);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 8px 10px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  @media (max-width: 900px) {
    .split {
      grid-template-columns: 1fr;
    }
  }
</style>
