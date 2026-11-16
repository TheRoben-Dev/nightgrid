<script lang="ts">
  /**
   * Monthly uptime report (M10t): availability per host and service for a
   * calendar month against a target, its outages, and an HTML export.
   */
  import { api, errorText, type UptimeReport } from "../../api";
  import { durText, lastMonths, pctText, TARGETS } from "../../uptime";
  import Modal from "../Modal.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();

  const months = lastMonths(13);
  let pick = $state(`${months[0].year}-${months[0].month}`);
  let target = $state<number>(99.9);
  let report = $state<UptimeReport | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let saved = $state<string | null>(null);

  const ym = $derived(pick.split("-").map(Number) as [number, number]);

  $effect(() => {
    if (!open) return;
    const [y, m] = ym;
    const t = target;
    void (async () => {
      error = null;
      try {
        report = await api.uptimeReport(y, m, t);
      } catch (e) {
        error = errorText(e);
      }
    })();
  });

  const met = $derived(report ? report.rows.filter((r) => r.met === true).length : 0);
  const withData = $derived(report ? report.rows.filter((r) => r.availability != null).length : 0);
  const overall = $derived.by(() => {
    if (!report) return null;
    const up = report.rows.reduce((a, r) => a + r.up, 0);
    const down = report.rows.reduce((a, r) => a + r.down, 0);
    return up + down ? (up * 100) / (up + down) : null;
  });
  const tone = (p: number | null) => (p == null ? "muted" : p >= target - 1e-9 ? "ok" : p >= 99 ? "warn" : "bad");

  async function exportHtml() {
    busy = true;
    saved = null;
    try {
      const path = await api.uptimeReportExport(ym[0], ym[1], target);
      if (path) saved = path;
    } catch (e) {
      error = errorText(e);
    }
    busy = false;
  }
</script>

<Modal {open} title="UPTIME REPORT" width={760} {onclose}>
  <div class="opts">
    <label class="field">
      <span>Month</span>
      <select class="input" bind:value={pick}>
        {#each months as m (`${m.year}-${m.month}`)}<option value="{m.year}-{m.month}">{m.label}</option>{/each}
      </select>
    </label>
    <label class="field">
      <span>Target</span>
      <select class="input" bind:value={target}>
        {#each TARGETS as t (t)}<option value={t}>{t}%</option>{/each}
      </select>
    </label>
  </div>
  {#if error}<p class="bad small" role="alert">{error}</p>{/if}
  {#if report}
    <div class="sum">
      <div><span class="k">AVAILABILITY</span><b class={tone(overall)}>{pctText(overall)}</b></div>
      <div><span class="k">MET TARGET</span><b class={met === withData ? "ok" : "bad"}>{met} of {withData}</b></div>
      <div><span class="k">OUTAGES</span><b>{report.incidents.length}</b></div>
      <div><span class="k">DOWNTIME</span><b>{durText(report.rows.reduce((a, r) => a + r.down, 0))}</b></div>
    </div>
    <div class="table">
      <table>
        <thead><tr><th>NAME</th><th class="n">AVAILABILITY</th><th class="n">DOWNTIME</th><th class="n">OUTAGES</th><th class="n">LONGEST</th><th>TARGET</th></tr></thead>
        <tbody>
          {#each report.rows as r (`${r.node.kind}:${r.node.id}`)}
            <tr>
              <td><span class="kind">{r.node.kind === "host" ? "HOST" : "SVC"}</span> {r.name}</td>
              <td class="n {tone(r.availability)}">{pctText(r.availability)}</td>
              <td class="n">{r.down ? durText(r.down) : "—"}</td>
              <td class="n">{r.outages}</td>
              <td class="n">{r.longest ? durText(r.longest) : "—"}</td>
              <td class={r.met === true ? "ok" : r.met === false ? "bad" : "muted"}>{r.met === true ? "met" : r.met === false ? "missed" : "no data"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if saved}<p class="ok small" role="status">Saved {saved}.</p>{/if}
    <p class="muted small">Counts only the time Nightgrid was watching; planned sleep isn't downtime. The export adds the list of outages and prints cleanly.</p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={onclose}>Close</button>
    <button type="button" class="btn primary" data-guide="uptime-report-export" disabled={!report || busy} onclick={exportHtml}>Export HTML…</button>
  {/snippet}
</Modal>

<style>
  .opts {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .opts .field {
    flex: 1 1 200px;
  }
  .sum {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    margin: 12px 0;
  }
  .sum div {
    border: 1px solid var(--line);
    padding: 6px 10px;
  }
  .sum b {
    display: block;
    font-size: var(--fs-lg);
    font-weight: 500;
  }
  .k,
  .kind {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--cyan);
  }
  .table {
    max-height: 320px;
    overflow: auto;
    border: 1px solid var(--line);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-xs);
  }
  th,
  td {
    text-align: left;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
  }
  th {
    color: var(--text-1);
    letter-spacing: 0.08em;
    font-weight: 500;
    position: sticky;
    top: 0;
    background: var(--bg-1);
  }
  .n {
    text-align: right;
    font-variant-numeric: tabular-nums;
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
    color: var(--text-1);
  }
  .small {
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .sum {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
