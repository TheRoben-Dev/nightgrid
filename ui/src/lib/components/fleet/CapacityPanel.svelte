<script lang="ts">
  /**
   * Fleet → CAPACITY (1.1, D-172): how much room each host has at its busiest
   * (p95 of the hourly history), what would still fit, and what a new server
   * for a set of workloads needs.
   */
  import { onMount } from "svelte";
  import { WORKLOADS, fit, sizing, type Workload } from "../../fleet11";
  import { capacity } from "../../stores/fleet11.svelte";
  import { hosts } from "../../stores/hosts.svelte";

  onMount(() => {
    if (!capacity.loaded) void capacity.load();
  });

  const GB = 1e9;
  let counts = $state<Record<string, number>>({});
  const picked = $derived(WORKLOADS.filter((w) => (counts[w.id] ?? 0) > 0).map((w) => ({ w, n: counts[w.id] })));
  const combined = $derived<Workload | null>(
    picked.length
      ? {
          id: "set",
          label: picked.map((p) => (p.n > 1 ? `${p.n}× ${p.w.label}` : p.w.label)).join(", "),
          cores: picked.reduce((a, p) => a + p.w.cores * p.n, 0),
          memGb: picked.reduce((a, p) => a + p.w.memGb * p.n, 0),
          diskGb: picked.reduce((a, p) => a + p.w.diskGb * p.n, 0),
        }
      : null,
  );
  const size = $derived(picked.length ? sizing(picked) : null);
  const rows = $derived(capacity.list.filter((c) => hosts.byId(c.hostId)).map((c) => ({ c, f: combined ? fit(c, combined) : null })));
  const name = (id: number) => hosts.byId(id)?.name ?? `host ${id}`;
  const tone = (p: number | null, warn: number, bad: number) => (p == null ? "" : p >= bad ? "bad" : p >= warn ? "warn" : "ok");
  const gb = (b: number) => (b >= 1e12 ? `${(b / 1e12).toFixed(1)} TB` : `${Math.round(b / GB)} GB`);
  const fullest = (c: (typeof rows)[number]["c"]) => [...c.disks].sort((a, b) => b.used / b.size - a.used / a.size)[0] ?? null;
</script>

<div class="cap" data-guide="fleet-capacity">
  <div class="row">
    <span class="label">BUSIEST OVER</span>
    {#each [7, 30, 90] as d (d)}<button type="button" class="btn tiny" class:primary={capacity.days === d} onclick={() => capacity.load(d)}>{d} days</button>{/each}
    <span class="muted small">p95 of the hourly averages: what a host reaches on its busiest hours, ignoring the very top 5%.</span>
  </div>
  {#if capacity.error}<p class="error-text small">{capacity.error}</p>{/if}

  <div class="table">
    <div class="th">
      <span>HOST</span><span>SIZE</span><span>CPU avg / p95</span><span>MEMORY avg / p95</span><span>FULLEST DISK</span>{#if combined}<span>FITS?</span>{/if}
    </div>
    {#each rows as { c, f } (c.hostId)}
      {@const d = fullest(c)}
      <div class="tr">
        <span class="name">{name(c.hostId)}</span>
        <span class="small">{c.spec ? `${c.spec.cores} cores · ${gb(c.spec.memTotal)}` : "not seen yet"}</span>
        <span class="bar"
          ><span class="fill {tone(c.cpuP95, 70, 85)}" style:width="{c.cpuP95 ?? 0}%"></span><span class="txt data">{c.cpuAvg ?? "--"}% / {c.cpuP95 ?? "--"}%</span></span
        >
        <span class="bar"
          ><span class="fill {tone(c.memP95, 80, 90)}" style:width="{c.memP95 ?? 0}%"></span><span class="txt data">{c.memAvg ?? "--"}% / {c.memP95 ?? "--"}%</span></span
        >
        <span class="small">{d ? `${d.mount} ${Math.round((d.used / d.size) * 100)}% of ${gb(d.size)}${d.daysLeft != null ? ` · full in ${Math.round(d.daysLeft)} d` : ""}` : "--"}</span>
        {#if f}<span class="fit {f.verdict}" title={f.why}>{f.verdict === "fits" ? "FITS" : f.verdict === "tight" ? "TIGHT" : f.verdict === "no" ? "NO" : "?"} <span class="muted small">{f.why}</span></span>{/if}
      </div>
    {:else}
      <p class="muted small pad">No hosts yet.</p>
    {/each}
  </div>
  {#if capacity.loaded && rows.some((r) => r.c.hours < 24)}<p class="muted small">Hosts with less than a day of history show rough numbers; Nightgrid keeps an hourly average while a host is monitored.</p>{/if}

  <div class="plan" data-guide="capacity-whatif">
    <div class="wl">
      <h3 class="hd">WHAT IF I ADD…</h3>
      {#each WORKLOADS as w (w.id)}
        <label class="wrow small">
          <input class="input n" type="number" min="0" max="20" value={counts[w.id] ?? 0} oninput={(e) => (counts = { ...counts, [w.id]: Math.max(0, Math.min(20, Number(e.currentTarget.value) || 0)) })} />
          <span>{w.label}</span>
          <span class="muted">{w.cores} core{w.cores === 1 ? "" : "s"} · {w.memGb} GB · {w.diskGb} GB disk</span>
        </label>
      {/each}
      {#if picked.length}<button type="button" class="btn tiny ghost" onclick={() => (counts = {})}>Clear</button>{/if}
    </div>
    <div class="size">
      <h3 class="hd">A NEW SERVER FOR THESE</h3>
      {#if size && combined}
        <p class="big data">{size.cores} cores · {size.memGb} GB · {size.diskGb} GB</p>
        <p class="muted small">{combined.cores.toFixed(1)} busy cores, {combined.memGb} GB and {combined.diskGb} GB of disk at their busiest, plus 30% headroom and room for the host itself (Proxmox or the OS). The column FITS? above says which existing host could take them instead.</p>
      {:else}
        <p class="muted small">Pick workloads on the left to see which host can take them and what a new server (for example a Proxmox host) would need. The numbers are typical, not measured; your own may differ.</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .cap {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    overflow: auto;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .table {
    border: 1px solid var(--line);
  }
  .th,
  .tr {
    display: grid;
    grid-template-columns: minmax(110px, 1fr) minmax(120px, 1fr) minmax(130px, 1.2fr) minmax(130px, 1.2fr) minmax(140px, 1.4fr) auto;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .th {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    color: var(--text-2);
  }
  .name {
    color: var(--text-0);
  }
  .bar {
    position: relative;
    height: 18px;
    border: 1px solid var(--line);
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    opacity: 0.35;
    background: var(--cyan);
  }
  .fill.warn {
    background: var(--warn);
  }
  .fill.bad {
    background: var(--danger);
  }
  .txt {
    position: relative;
    font-size: var(--fs-xs);
    padding-left: 4px;
    color: var(--text-0);
  }
  .fit {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fit.fits {
    color: var(--ok);
  }
  .fit.tight {
    color: var(--warn);
  }
  .fit.no {
    color: var(--danger);
  }
  .plan {
    display: grid;
    grid-template-columns: 3fr 2fr;
    gap: 10px;
  }
  .wl,
  .size {
    border: 1px solid var(--line);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .wrow {
    display: grid;
    grid-template-columns: 56px 1fr auto;
    gap: 8px;
    align-items: center;
  }
  .n {
    width: 52px;
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 0 0 4px;
  }
  .big {
    font-size: var(--fs-lg, 20px);
    color: var(--text-0);
  }
  .muted {
    color: var(--text-2);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px;
  }
  p {
    margin: 0;
  }
  @media (max-width: 900px) {
    .plan {
      grid-template-columns: 1fr;
    }
  }
</style>
