<script lang="ts">
  import { api, type MetricPoint } from "../../api";
  import { formatSize } from "../../files/util";
  import { hosts } from "../../stores/hosts.svelte";
  import { formatRate, formatUptime, metrics } from "../../stores/metrics.svelte";
  import { terms } from "../../stores/terms.svelte";
  import Modal from "../Modal.svelte";
  import HostIcon from "../HostIcon.svelte";
  import HistoryChart from "./HistoryChart.svelte";
  import Meter from "./Meter.svelte";
  import Sparkline from "./Sparkline.svelte";

  const id = $derived(metrics.detailId);
  const host = $derived(id == null ? undefined : hosts.byId(id));
  const m = $derived(id == null ? null : metrics.get(id));
  const s = $derived(m?.sample ?? null);

  $effect(() => {
    metrics.setVisible("monitor", id == null ? [] : [id]);
  });

  const RANGES = [
    { id: "1h", label: "1H", secs: 3600 },
    { id: "24h", label: "24H", secs: 86400 },
    { id: "7d", label: "7D", secs: 7 * 86400 },
    { id: "30d", label: "30D", secs: 30 * 86400 },
  ] as const;
  const KINDS = ["cpu", "mem", "net", "io", "temp"] as const;
  type Kind = (typeof KINDS)[number];
  let range = $state<(typeof RANGES)[number]["id"]>("1h");
  let kind = $state<Kind>("cpu");
  let points = $state<MetricPoint[]>([]);
  let span = $state({ from: 0, to: 0 });

  async function loadHistory(hostId: number, secs: number) {
    const to = Math.floor(Date.now() / 1000);
    const from = to - secs;
    try {
      const p = await api.metricsHistory(hostId, from, to);
      if (metrics.detailId === hostId) {
        points = p;
        span = { from, to };
      }
    } catch {
      points = [];
    }
  }
  $effect(() => {
    if (id == null) return;
    const secs = RANGES.find((r) => r.id === range)!.secs;
    void loadHistory(id, secs);
    const t = setInterval(() => void loadHistory(id, secs), 60_000);
    return () => clearInterval(t);
  });

  const pct = (v: number) => `${v.toFixed(v < 10 ? 1 : 0)}%`;
  const chart = $derived.by(() => {
    const ts = points.map((p) => p.ts);
    switch (kind) {
      case "cpu":
        return { ts, max: 100, format: pct, series: [{ label: "CPU", color: "var(--cyan)", values: points.map((p) => p.cpu) }] };
      case "mem":
        return {
          ts,
          max: 100,
          format: pct,
          series: [
            { label: "Memory", color: "var(--cyan)", values: points.map((p) => p.mem) },
            { label: "Swap", color: "var(--magenta)", values: points.map((p) => p.swap) },
          ],
        };
      case "net":
        return {
          ts,
          max: undefined,
          format: formatRate,
          series: [
            { label: "Down", color: "var(--cyan)", values: points.map((p) => p.netRx) },
            { label: "Up", color: "var(--magenta)", values: points.map((p) => p.netTx) },
          ],
        };
      case "io":
        return {
          ts,
          max: undefined,
          format: formatRate,
          series: [
            { label: "Read", color: "var(--cyan)", values: points.map((p) => p.diskRead) },
            { label: "Write", color: "var(--magenta)", values: points.map((p) => p.diskWrite) },
          ],
        };
      case "temp":
        return { ts, max: undefined, format: (v: number) => `${v.toFixed(1)}°C`, series: [{ label: "CPU temp", color: "var(--accent)", values: points.map((p) => p.temp) }] };
    }
  });
  const kindLabel: Record<Kind, string> = { cpu: "CPU", mem: "MEMORY", net: "NETWORK", io: "DISK I/O", temp: "TEMP" };

  const memPct = $derived(s && s.memTotal ? (s.memUsed / s.memTotal) * 100 : null);
  const swapPct = $derived(s && s.swapTotal ? (s.swapUsed / s.swapTotal) * 100 : null);
  const usedPct = (u: number, a: number) => (u + a ? (u / (u + a)) * 100 : 0);
</script>

<Modal open={id != null} title={host ? `MONITOR :: ${host.name}` : "MONITOR"} width={1040} onclose={() => (metrics.detailId = null)}>
  {#if host && m}
    <div class="top">
      <HostIcon art={host.icon} color={host.color} size="md" />
      <div class="sys">
        <div class="hn">{s?.hostname || host.hostname}</div>
        {#if s}
          <div class="muted">{s.kernel} · up {formatUptime(s.uptimeSecs)} · load {s.load.map((l) => l.toFixed(2)).join(" ")}</div>
        {:else if m.state === "needsLogin"}
          <div class="warn">Needs a login first. Open a terminal on {host.name} (password or host key), then metrics start.</div>
        {:else if m.message}
          <div class="warn">{m.message}</div>
        {:else}
          <div class="muted">Waiting for the first sample…</div>
        {/if}
      </div>
      <span class="spacer"></span>
      {#if m.state === "offline" || m.state === "error"}
        <button type="button" class="btn ghost tiny" onclick={() => metrics.retry(host.id)}>Retry</button>
      {/if}
      <button type="button" class="btn ghost tiny" onclick={() => terms.openHost(host.id)}>Shell</button>
    </div>

    {#if s}
      <div class="grid">
        <section>
          <h3 class="label">CPU <span class="data big">{s.cpu == null ? "--" : pct(s.cpu)}</span></h3>
          <Sparkline values={m.cpu} max={100} height={56} label="CPU trend" />
          <div class="cores" style:--n={Math.min(s.cores.length, 16)}>
            {#each s.cores as c, i (i)}
              <span class="core" title="core {i}: {pct(c)}">
                <span class="cfill" class:warn={c >= 75} class:crit={c >= 90} style:height="{Math.max(2, c)}%"></span>
              </span>
            {/each}
          </div>
        </section>

        <section>
          <h3 class="label">MEMORY <span class="data big">{memPct == null ? "--" : pct(memPct)}</span></h3>
          <Sparkline values={m.mem} max={100} height={56} label="Memory trend" />
          <div class="kv">
            <span>RAM</span><Meter value={memPct} label="Memory" /><span class="data">{formatSize(s.memUsed)} / {formatSize(s.memTotal)}</span>
            <span>SWAP</span><Meter value={swapPct} warn={50} crit={80} label="Swap" /><span class="data"
              >{s.swapTotal ? `${formatSize(s.swapUsed)} / ${formatSize(s.swapTotal)}` : "none"}</span
            >
          </div>
        </section>

        <section>
          <h3 class="label">DISKS <span class="muted small">↓ {formatRate(s.diskRead)} read · ↑ {formatRate(s.diskWrite)} write</span></h3>
          <div class="kv mounts">
            {#each s.mounts as d (d.mount)}
              {@const p = usedPct(d.used, d.avail)}
              <span class="mono" title="{d.device} ({d.fstype})">{d.mount}</span>
              <Meter value={p} warn={80} crit={92} label={d.mount} />
              <span class="data">{pct(p)} · {formatSize(d.avail)} free</span>
            {:else}
              <span class="muted">No mounted filesystems reported.</span>
            {/each}
          </div>
          {#if s.disks.length}
            <div class="ifaces">
              {#each s.disks as d (d.name)}
                <span class="mono">{d.name}</span><span class="data">R {formatRate(d.rx)}</span><span class="data">W {formatRate(d.tx)}</span>
              {/each}
            </div>
          {/if}
        </section>

        <section>
          <h3 class="label">NETWORK <span class="muted small">↓ {formatRate(s.netRx)} · ↑ {formatRate(s.netTx)}</span></h3>
          <Sparkline values={m.net} height={56} label="Network trend" color="var(--magenta)" />
          <div class="ifaces">
            {#each s.net as n (n.name)}
              <span class="mono">{n.name}</span><span class="data">↓ {formatRate(n.rx)}</span><span class="data">↑ {formatRate(n.tx)}</span>
            {/each}
          </div>
        </section>

        <section>
          <h3 class="label">TEMPERATURES</h3>
          {#if s.temps.length}
            <div class="temps">
              {#each s.temps as t, i (i)}
                <span class="mono">{t.chip}{t.label ? ` · ${t.label}` : ""}</span>
                <span class="data" class:hot={t.celsius >= 80}>{t.celsius.toFixed(1)}°C</span>
              {/each}
            </div>
          {:else}
            <p class="muted small">n/a: this host exposes no sensors (common in VMs and containers).</p>
          {/if}
        </section>
      </div>
    {/if}

    <section class="history">
      <div class="hbar">
        <h3 class="label">HISTORY</h3>
        <div class="seg" role="tablist" aria-label="Metric">
          {#each KINDS as k (k)}
            <button type="button" role="tab" aria-selected={kind === k} class:on={kind === k} onclick={() => (kind = k)}>{kindLabel[k]}</button>
          {/each}
        </div>
        <span class="spacer"></span>
        <div class="seg" role="tablist" aria-label="Range">
          {#each RANGES as r (r.id)}
            <button type="button" role="tab" aria-selected={range === r.id} class:on={range === r.id} onclick={() => (range = r.id)}>{r.label}</button>
          {/each}
        </div>
      </div>
      <HistoryChart ts={chart.ts} series={chart.series} max={chart.max} format={chart.format} from={span.from} to={span.to} />
    </section>
  {/if}
</Modal>

<style>
  .top {
    display: flex;
    gap: 14px;
    align-items: center;
    margin-bottom: 12px;
  }
  .sys {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .hn {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-size: var(--fs-lg);
  }
  .muted {
    color: var(--text-1);
    font-size: var(--fs-xs);
  }
  .small {
    font-size: var(--fs-xs);
    letter-spacing: 0;
    text-transform: none;
    font-family: var(--font-mono);
  }
  .warn {
    color: var(--accent);
    font-size: var(--fs-sm);
  }
  .spacer {
    flex: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 10px;
  }
  section {
    border: 1px solid var(--line);
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  h3 {
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .big {
    font-family: var(--font-mono);
    font-size: var(--fs-lg);
    letter-spacing: 0;
  }
  .data {
    color: var(--cyan);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .data.hot {
    color: var(--danger);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cores {
    display: grid;
    grid-template-columns: repeat(var(--n), 1fr);
    gap: 3px;
    height: 44px;
  }
  .core {
    position: relative;
    background: color-mix(in srgb, var(--line) 50%, transparent);
  }
  .cfill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    background: var(--cyan);
    transition: height var(--dur) ease-out;
  }
  .cfill.warn {
    background: var(--accent);
  }
  .cfill.crit {
    background: var(--danger);
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 6px 10px;
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .mounts {
    grid-template-columns: minmax(60px, 140px) 1fr auto;
  }
  .ifaces,
  .temps {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 4px 12px;
    align-items: center;
  }
  .temps {
    grid-template-columns: 1fr auto;
  }
  .history {
    margin-top: 10px;
  }
  .hbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 9px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
</style>
