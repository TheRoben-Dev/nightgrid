<script lang="ts">
  /**
   * Fleet → SPEED (M10x): iperf3 throughput both ways and ping latency between
   * two of your hosts. Nightgrid starts a short-lived iperf3 server on the
   * target and the client on the source; only your own hosts are involved.
   */
  import { hosts } from "../../stores/hosts.svelte";
  import { netperf } from "../../stores/netperf.svelte";
  import { best, formatBps, formatMs, pairs, tone } from "../../netperf";
  import Sparkline from "../metrics/Sparkline.svelte";

  let from = $state<number | null>(null);
  let to = $state<number | null>(null);
  let target = $state("");
  let targetTouched = false;
  let secs = $state(5);
  let confirmClear = $state<string | null>(null);

  $effect(() => {
    if (!netperf.loaded) void netperf.load();
  });
  $effect(() => {
    if (from == null && hosts.hosts.length) from = hosts.hosts[0].id;
    if (to == null && hosts.hosts.length > 1) to = hosts.hosts[1].id;
  });
  const hostOf = (id: number) => hosts.hosts.find((h) => h.id === id);
  const nameOf = (id: number) => hostOf(id)?.name ?? `#${id}`;
  // The address suggestion follows the target host until edited.
  $effect(() => {
    const h = to == null ? undefined : hostOf(to);
    if (!targetTouched) target = h ? (h.hostname.split("@").pop() ?? h.hostname) : "";
  });

  const list = $derived(pairs(netperf.tests));
  const when = (ts: number | null) => (ts == null ? "" : new Date(ts * 1000).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }));

  function run() {
    if (from == null || to == null || from === to) return;
    void netperf.run(from, to, target.trim() || null, secs);
  }
</script>

<div class="speed" data-guide="fleet-speed">
  <div class="bar">
    <label>From <select class="input" bind:value={from}>{#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}</select></label>
    <span aria-hidden="true">→</span>
    <label>To <select class="input" bind:value={to}>{#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}</select></label>
    <label
      >at <input class="input mono addr" bind:value={target} oninput={() => (targetTouched = true)} placeholder="address the source uses" spellcheck="false" title="The address the source host connects to, e.g. the target's LAN IP" /></label
    >
    <label>for <select class="input" bind:value={secs}><option value={5}>5 s</option><option value={10}>10 s</option><option value={20}>20 s</option></select> each way</label>
    <button type="button" class="btn primary tiny" data-guide="speed-run" disabled={!!netperf.running || from == null || to == null || from === to} onclick={run}
      >{netperf.running ? "Testing…" : "Run test"}</button
    >
  </div>
  <p class="muted small hint">
    Needs <code>iperf3</code> on both hosts. The target runs an iperf3 server on port 5201 only for the test (it stops itself after two minutes at most), so
    its firewall must let the source in on that port. Pick the address the source can reach directly, usually the target's LAN IP.
  </p>
  {#if from != null && from === to}<p class="warn-text small">Pick two different hosts.</p>{/if}
  {#if netperf.error}<p class="error-text small pre" role="alert">{netperf.error}</p>{/if}

  <div class="scroll">
    <div class="row headrow" aria-hidden="true">
      <span>PAIR</span><span class="n">UP</span><span class="n">DOWN</span><span class="n">LATENCY</span><span class="n">RETRANS</span><span>HISTORY</span><span>LAST RUN</span><span></span>
    </div>
    {#each list as p (p.key)}
      {@const t = p.latest}
      {@const tn = tone(t, p.runs)}
      <div class="row {tn}">
        <span class="pair"><strong>{nameOf(p.fromHost)}</strong> → <strong>{nameOf(p.toHost)}</strong><span class="muted small tgt">{t.target}</span></span>
        <span class="n data">{formatBps(t.upBps)}</span>
        <span class="n data">{formatBps(t.downBps)}</span>
        <span class="n data" title={t.jitterMs != null ? `± ${formatMs(t.jitterMs)}${t.lossPct ? `, ${t.lossPct}% lost` : ""}` : ""}
          >{formatMs(t.rttMs)}{#if t.lossPct}<span class="warn-text"> · {t.lossPct}% lost</span>{/if}</span
        >
        <span class="n data">{t.retransmits ?? "--"}</span>
        <span class="spark"><Sparkline values={p.runs.map((r) => (best(r) ?? 0) / 1e6)} capacity={30} height={22} label="Throughput history" /></span>
        <span class="muted small nowrap">{when(t.at)}</span>
        <span class="acts">
          <button
            type="button"
            class="btn ghost tiny"
            disabled={!!netperf.running}
            onclick={() => void netperf.run(p.fromHost, p.toHost, t.target, secs)}>{netperf.running === p.key ? "Testing…" : "Again"}</button
          >
          <button type="button" class="btn ghost tiny" onclick={() => (confirmClear = p.key)}>Clear…</button>
        </span>
        {#if netperf.findings[p.key] || (tn !== "ok" && t.error)}
          <p class="note small {tn}">{netperf.findings[p.key] ?? ""}{netperf.findings[p.key] && t.error ? " " : ""}{t.error ?? ""}</p>
        {:else if tn === "warn" && !t.error}
          <p class="note small warn">
            {(best(t) ?? 0) >= 85e6 && (best(t) ?? 0) <= 100e6
              ? "About 100 Mbit/s: a link on the way may have negotiated Fast Ethernet. Check the cable and the port speed (ethtool)."
              : (t.lossPct ?? 0) > 0
                ? "Ping lost packets on this path."
                : "Slower than usual for this pair."}
          </p>
        {/if}
        {#if confirmClear === p.key}
          <div class="confirm" role="alertdialog" aria-label="Confirm">
            <span>Forget all {p.runs.length} results for {nameOf(p.fromHost)} → {nameOf(p.toHost)}?</span>
            <button type="button" class="btn ghost tiny" onclick={() => (confirmClear = null)}>Keep</button>
            <button
              type="button"
              class="btn danger tiny"
              onclick={() => {
                confirmClear = null;
                void netperf.clear(p.fromHost, p.toHost);
              }}>Forget results</button
            >
          </div>
        {/if}
      </div>
    {:else}
      <p class="muted small empty">{netperf.loaded ? "No tests yet. Pick two hosts and run one." : "Loading…"}</p>
    {/each}
  </div>
</div>

<style>
  .speed {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    padding: 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .bar label {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    color: var(--text-1);
  }
  .addr {
    width: 170px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .hint {
    margin: 6px 8px;
  }
  .pre {
    white-space: pre-wrap;
    margin: 0 8px 6px;
  }
  .warn-text {
    color: var(--warn);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(160px, 2fr) 90px 90px 100px 60px 90px 115px auto;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .headrow {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    position: sticky;
    top: 0;
    background: var(--bg-1);
    z-index: 1;
  }
  .row.warn {
    border-left: 2px solid var(--warn);
  }
  .row.bad {
    border-left: 2px solid var(--danger);
  }
  .n {
    text-align: right;
  }
  .pair {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nowrap {
    white-space: nowrap;
  }
  .tgt {
    margin-left: 6px;
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }
  .note {
    grid-column: 1 / -1;
    margin: 0;
    white-space: pre-wrap;
  }
  .note.warn {
    color: var(--warn);
  }
  .note.bad {
    color: var(--danger);
  }
  .confirm {
    grid-column: 1 / -1;
    display: flex;
    gap: 6px;
    align-items: center;
    border: 1px solid var(--danger);
    padding: 4px 8px;
  }
  .confirm span {
    flex: 1;
  }
  .empty {
    padding: 12px 8px;
  }
</style>
