<script lang="ts">
  import type { Disk } from "../../api";
  import { formatSize } from "../../files/util";
  import { formatHours, HEALTH_WORD, system } from "../../stores/system.svelte";

  const r = $derived(system.disks);
  let open = $state<string | null>(null);

  const kind = (d: Disk) => (d.protocol === "NVMe" ? "NVMe SSD" : d.rotationRpm ? `HDD ${d.rotationRpm} rpm` : d.rotationRpm === 0 ? "SSD" : d.protocol);
</script>

<div class="disks" data-guide="system-disks">
  <div class="bar">
    {#if r}
      <span class="sum {r.health}">{r.disks.length} DISK{r.disks.length === 1 ? "" : "S"} · {HEALTH_WORD[r.health]}</span>
    {/if}
    <span class="spacer"></span>
    {#if r || system.disksLoading}
      <button type="button" class="btn ghost tiny" onclick={() => system.loadDisks()} disabled={system.disksLoading}>
        {system.disksLoading ? "Reading…" : "Refresh"}
      </button>
    {/if}
  </div>
  {#if system.errors.disks}<p class="error-text">{system.errors.disks}</p>{/if}

  <div class="grid">
    {#if !r && !system.disksLoading && !system.errors.disks}
      <div class="empty">
        <p class="label">SMART NEEDS ROOT</p>
        <p class="muted">Disk health comes from <code>smartctl</code> (smartmontools) run with sudo on the host.</p>
        <button type="button" class="btn primary" onclick={() => system.loadDisks()}>Read SMART</button>
      </div>
    {:else if r && !r.disks.length}
      <p class="muted">smartctl found no disks (virtual machines and some RAID controllers hide them).</p>
    {/if}
    {#each r?.disks ?? [] as d (d.name)}
      <article class="card {d.health}">
        <header>
          <span class="dev">{d.name}</span>
          <span class="model" title={d.model ?? ""}>{d.model ?? "Unknown model"}</span>
          <span class="state"><span class="dot" aria-hidden="true"></span>{HEALTH_WORD[d.health]}</span>
        </header>
        <dl>
          <div><dt>TYPE</dt><dd>{kind(d)}</dd></div>
          <div><dt>SIZE</dt><dd>{d.capacity ? formatSize(d.capacity) : "—"}</dd></div>
          <div><dt>TEMP</dt><dd>{d.tempC != null ? `${d.tempC} °C` : "—"}</dd></div>
          <div><dt>POWERED</dt><dd title={d.powerOnHours != null ? `${d.powerOnHours} hours` : ""}>{formatHours(d.powerOnHours)}</dd></div>
          {#if d.nvme}
            <div><dt>WEAR</dt><dd>{d.nvme.percentageUsed}%</dd></div>
            <div><dt>WRITTEN</dt><dd>{formatSize(d.nvme.bytesWritten)}</dd></div>
          {/if}
        </dl>
        {#if d.issues.length}
          <ul class="issues">
            {#each d.issues as i (i)}<li>{i}</li>{/each}
          </ul>
        {/if}
        <footer>
          <span class="muted">{d.serial ? `S/N ${d.serial}` : ""}{d.firmware ? ` · FW ${d.firmware}` : ""}</span>
          <span class="spacer"></span>
          {#if d.attributes.length || d.nvme}
            <button type="button" class="btn ghost tiny" aria-expanded={open === d.name} onclick={() => (open = open === d.name ? null : d.name)}>
              {open === d.name ? "Hide details" : "Details"}
            </button>
          {/if}
        </footer>
        {#if open === d.name}
          {#if d.attributes.length}
            <table>
              <thead><tr><th>ID</th><th>ATTRIBUTE</th><th>VAL</th><th>WORST</th><th>THR</th><th>RAW</th></tr></thead>
              <tbody>
                {#each d.attributes as a (a.id)}
                  <tr class={a.health}>
                    <td>{a.id}</td>
                    <td>{a.name.replace(/_/g, " ")}</td>
                    <td>{a.value}</td>
                    <td>{a.worst}</td>
                    <td>{a.thresh || "—"}</td>
                    <td class="raw">{a.raw}{a.failed ? ` (failed ${a.failed})` : ""}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
          {#if d.nvme}
            <dl class="nvme">
              <div><dt>SPARE</dt><dd>{d.nvme.availableSpare}% (min {d.nvme.spareThreshold}%)</dd></div>
              <div><dt>MEDIA ERRORS</dt><dd>{d.nvme.mediaErrors}</dd></div>
              <div><dt>ERROR LOG</dt><dd>{d.nvme.errorLogEntries}</dd></div>
              <div><dt>UNSAFE SHUTDOWNS</dt><dd>{d.nvme.unsafeShutdowns}</dd></div>
              <div><dt>CRITICAL</dt><dd>{d.nvme.criticalWarning ? `0x${d.nvme.criticalWarning.toString(16)}` : "none"}</dd></div>
            </dl>
          {/if}
        {/if}
      </article>
    {/each}
  </div>
</div>

<style>
  .disks {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
  }
  .spacer {
    flex: 1;
  }
  .sum.ok {
    color: var(--ok);
  }
  .sum.warn {
    color: var(--warn);
  }
  .sum.bad {
    color: var(--danger);
  }
  .sum.unknown {
    color: var(--text-1);
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 8px;
    align-content: start;
  }
  .empty {
    padding: 16px;
    display: grid;
    gap: 8px;
    justify-items: start;
  }
  .empty p {
    margin: 0;
  }
  code {
    color: var(--cyan);
  }
  .card {
    --tone: var(--text-1);
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 10px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-left: 2px solid var(--tone);
    min-width: 0;
  }
  .card.ok {
    --tone: var(--ok);
  }
  .card.warn {
    --tone: var(--warn);
  }
  .card.bad {
    --tone: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 45%, var(--line));
  }
  header {
    display: flex;
    gap: 8px;
    align-items: baseline;
    min-width: 0;
  }
  .dev {
    font-family: var(--font-mono);
    color: var(--text-0);
  }
  .model {
    flex: 1;
    color: var(--text-1);
    font-size: var(--fs-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    display: flex;
    gap: 5px;
    align-items: center;
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--tone);
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--tone);
  }
  dl {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  dl div {
    display: flex;
    flex-direction: column;
  }
  dt {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  dd {
    margin: 0;
    color: var(--cyan);
  }
  .issues {
    margin: 0;
    padding-left: 16px;
    font-size: var(--fs-sm);
    color: var(--tone);
  }
  footer {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-xs);
  }
  th {
    text-align: left;
    color: var(--text-1);
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    font-weight: 600;
    border-bottom: 1px solid var(--line);
    padding: 2px 4px;
  }
  td {
    padding: 2px 4px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  }
  tr.warn td {
    color: var(--warn);
  }
  tr.bad td {
    color: var(--danger);
  }
  .raw {
    font-family: var(--font-mono);
  }
  .tiny {
    min-height: 22px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
</style>
