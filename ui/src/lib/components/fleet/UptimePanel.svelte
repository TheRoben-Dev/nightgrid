<script lang="ts">
  /**
   * Fleet → UPTIME (M10t): one bar per day for every host and checked
   * service, availability over the range, and each one's outages.
   */
  import { api, errorText, type DepNode, type UptimeOutage, type UptimeSeries } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { services } from "../../stores/services.svelte";
  import UptimeReport from "./UptimeReport.svelte";
  import { barTitle, barTone, dayBars, DAY, durText, nodeKey, outageLen, outageStats, outageWhen, pctText, totals, utcOffsetMin } from "../../uptime";

  const RANGES = [
    { days: 30, label: "30 DAYS" },
    { days: 90, label: "90 DAYS" },
  ] as const;
  let days = $state<number>(90);
  let series = $state<UptimeSeries[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let open = $state<string | null>(null);
  let outages = $state<UptimeOutage[]>([]);
  let outagesLoading = $state(false);
  let reportOpen = $state(false);

  async function load() {
    try {
      series = await api.uptimeDays(days, utcOffsetMin());
      error = null;
    } catch (e) {
      error = errorText(e);
    }
    loaded = true;
  }
  $effect(() => {
    void days;
    void load();
  });
  $effect(() => {
    if (!services.loaded) void services.refresh();
    const t = setInterval(() => void load(), 60_000);
    return () => clearInterval(t);
  });

  interface Row {
    node: DepNode;
    name: string;
    sub: string;
    client: boolean;
  }
  const rows = $derived.by(() => {
    const h: Row[] = hosts.hosts.map((x) => ({ node: { kind: "host", id: x.id }, name: x.name, sub: x.hostname, client: x.role === "client" }));
    const s: Row[] = services.services.filter((x) => x.checkEnabled).map((x) => ({ node: { kind: "service", id: x.id }, name: x.name, sub: x.url, client: false }));
    return { hosts: h, services: s };
  });
  const byKey = $derived(Object.fromEntries(series.map((s) => [nodeKey(s.node), s])));
  const overall = $derived(totals(series.flatMap((s) => dayBars(s, days))));

  async function toggle(r: Row) {
    const k = nodeKey(r.node);
    if (open === k) {
      open = null;
      return;
    }
    open = k;
    outages = [];
    outagesLoading = true;
    const now = Math.floor(Date.now() / 1000);
    try {
      outages = await api.uptimeOutages(r.node, now - days * DAY, now);
    } catch (e) {
      error = errorText(e);
    }
    outagesLoading = false;
  }
</script>

<div class="uptime" data-guide="fleet-uptime">
  <div class="bar">
    <div class="seg" role="group" aria-label="Range">
      {#each RANGES as r (r.days)}
        <button type="button" class:on={days === r.days} onclick={() => (days = r.days)}>{r.label}</button>
      {/each}
    </div>
    <span class="muted small" title="A target must stay down for 2½ minutes to count. The first 3 minutes after Nightgrid starts or this computer wakes, and moments when every host fails at once (this computer's own network), are unknown time. Client devices going away is never downtime.">
      Up out of the time Nightgrid was watching. Grey is unknown, red is confirmed down; sleep, short blips and this computer's own network drops don't count.
    </span>
    <span class="spacer"></span>
    {#if overall.pct != null}<span class="small">ALL <strong class="t-{barTone(overall.pct)}">{pctText(overall.pct)}</strong></span>{/if}
    <button type="button" class="btn ghost tiny" data-guide="uptime-report" onclick={() => (reportOpen = true)}>Monthly report…</button>
  </div>
  {#if error}<p class="bad small" role="alert">{error}</p>{/if}
  {#if loaded && !series.length}
    <p class="muted small">
      Nothing recorded yet. Uptime is recorded while Nightgrid runs (in the tray too): hosts from monitoring (Overview), services from their checks. Bars fill in from today.
    </p>
  {/if}

  <div class="scroll">
    {#each [{ title: "HOSTS", list: rows.hosts }, { title: "SERVICES", list: rows.services }] as g (g.title)}
      {#if g.list.length}
        <h3 class="label">{g.title}</h3>
        {#each g.list as r (nodeKey(r.node))}
          {@const bars = dayBars(byKey[nodeKey(r.node)], days)}
          {@const t = totals(bars)}
          {@const k = nodeKey(r.node)}
          <div class="row" class:open={open === k}>
            <button type="button" class="head" aria-expanded={open === k} onclick={() => void toggle(r)}>
              <span class="name">{r.name}</span>
              <span class="days" aria-hidden="true">
                {#each bars as b (b.day)}<i class={barTone(b.pct)} title={barTitle(b)}></i>{/each}
              </span>
              <span class="pct t-{barTone(t.pct)}">{pctText(t.pct)}</span>
              <span class="down muted small">{t.down ? `${durText(t.down)} down` : r.client ? "client, not counted" : t.seenDays ? "no downtime" : ""}</span>
            </button>
            {#if open === k}
              <div class="detail">
                <p class="muted small">
                  {r.sub} · watched {t.seenDays} of {days} days{t.asleep ? ` · ${durText(t.asleep)} asleep` : ""}
                </p>
                {#if outagesLoading}
                  <p class="muted small">Reading outages…</p>
                {:else if outages.length}
                  {@const st = outageStats(outages)}
                  <p class="small">{st.count} outage{st.count === 1 ? "" : "s"} · {durText(st.total)} in all · longest {durText(st.longest)}</p>
                  <ul class="outages">
                    {#each outages as o (o.id)}
                      <li><span class="when">{outageWhen(o)}</span><span class="len">{durText(outageLen(o))}</span><span class="muted">{o.detail}</span></li>
                    {/each}
                  </ul>
                {:else}
                  <p class="muted small">No outages in this range.</p>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    {/each}
  </div>
</div>

<UptimeReport open={reportOpen} onclose={() => (reportOpen = false)} />

<style>
  .uptime {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
  }
  .seg button {
    background: none;
    border: 0;
    color: var(--text-1);
    padding: 3px 10px;
    font: inherit;
    letter-spacing: 0.08em;
    cursor: pointer;
  }
  .seg button.on {
    color: var(--text-0);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .bad {
    color: var(--danger);
  }
  .scroll {
    overflow: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  h3 {
    margin: 10px 0 4px;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .row {
    border: 1px solid transparent;
  }
  .row.open {
    border-color: var(--line);
  }
  .head {
    display: grid;
    grid-template-columns: minmax(100px, 180px) 1fr 72px 150px;
    gap: 12px;
    align-items: center;
    width: 100%;
    background: none;
    border: 0;
    padding: 4px 6px;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .head:hover {
    background: var(--bg-1);
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .days {
    display: flex;
    gap: 1px;
    height: 16px;
  }
  .days i {
    flex: 1;
    min-width: 1px;
    background: var(--ok);
  }
  .days i.warn {
    background: var(--warn);
  }
  .days i.bad {
    background: var(--danger);
  }
  .days i.none {
    background: var(--line);
  }
  .pct {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .t-ok {
    color: var(--ok);
  }
  .t-warn {
    color: var(--warn);
  }
  .t-bad {
    color: var(--danger);
  }
  .t-none {
    color: var(--text-1);
  }
  .detail {
    padding: 4px 8px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .detail p {
    margin: 0;
  }
  .outages {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-xs);
  }
  .outages li {
    display: grid;
    grid-template-columns: 140px 90px 1fr;
    gap: 10px;
    padding: 2px 0;
    border-bottom: 1px solid var(--line);
  }
  .len {
    color: var(--danger);
  }
</style>
