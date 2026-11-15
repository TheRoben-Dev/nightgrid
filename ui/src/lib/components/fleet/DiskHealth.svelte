<script lang="ts">
  /**
   * Fleet → DISKS (M10m): storage pools (ZFS, mdraid, btrfs), SMART and a
   * "full in about X days" forecast per mount, for every host.
   */
  import { diskRows, disksText, fillText, fillTone, POOL_WORD, rateText, size, sparkPath, usedPct, agoText } from "../../fleet";
  import { fleet } from "../../stores/fleet.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";

  $effect(() => {
    if (hosts.hosts.length) fleet.ensureDisks();
  });

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const within = $derived(settings.value.alerts.forecastDays);
  const rows = $derived(diskRows(hosts.hosts, fleet.disks, within));
  const counts = $derived({ bad: rows.filter((r) => r.tone === "bad").length, warn: rows.filter((r) => r.tone === "warn").length });
  const W = 120;
  const H = 22;

  async function openSmart(id: number) {
    system.select(id);
    ui.go("system");
    await system.openTab("disks");
  }
</script>

<div class="disks" data-guide="fleet-disks">
  <div class="bar">
    <button type="button" class="btn ghost tiny" data-guide="fleet-disks-check" disabled={fleet.disksChecking} onclick={() => fleet.checkDisks()}
      >{fleet.disksChecking ? "Reading…" : "Check all"}</button
    >
    <span class="muted small">read {agoText(fleet.disksAt, now)}</span>
    <span class="spacer"></span>
    {#if counts.bad}<span class="sum bad">{counts.bad} NEED ATTENTION</span>{/if}
    {#if counts.warn}<span class="sum warn">{counts.warn} TO WATCH</span>{/if}
    {#if !counts.bad && !counts.warn && fleet.disksAt}<span class="sum ok">ALL HEALTHY</span>{/if}
    {#if rows.some((r) => r.state?.status === "ok")}
      <ExplainButton
        source="disk health"
        text={() => disksText(rows)}
        question="Which of these disks, pools or mounts need attention first, and what should I do about each (replace, scrub, clean up, expand)?"
      />
    {/if}
  </div>

  <div class="scroll">
    {#each rows as r (r.host.id)}
      {@const d = r.state}
      <section class="host t-{r.tone}">
        <header>
          <span class="name">{r.host.name}</span>
          {#if d?.status === "checking"}<span class="muted small">reading…</span>{/if}
          {#if d?.status === "error"}<span class="bad small" title={d.error}>{d.error}</span>{/if}
          {#if d?.smart}
            <span class="chip {d.smart.health}" title="SMART"
              >SMART · {d.smart.disks.length} DISK{d.smart.disks.length === 1 ? "" : "S"}{d.smart.health === "ok" ? " OK" : d.smart.health === "unknown" ? " ?" : ` · ${d.smart.health.toUpperCase()}`}</span
            >
          {:else if d?.smartNote}
            <span class="muted small" title={d.smartNote}>SMART: {d.smartNote}</span>
          {/if}
          <span class="spacer"></span>
          {#if d?.smart}<button type="button" class="btn ghost tiny" onclick={() => openSmart(r.host.id)}>SMART details</button>{/if}
          <button type="button" class="btn ghost tiny" disabled={d?.status === "checking"} onclick={() => fleet.checkDisksHost(r.host.id)}>Check</button>
        </header>

        {#if d?.smart}
          {@const sick = d.smart.disks.filter((k) => k.health === "warn" || k.health === "bad")}
          {#each sick as k (k.name)}
            <p class="issue {k.health}">{k.name}{k.model ? ` (${k.model})` : ""}: {k.issues.join("; ") || "SMART reports a problem"}</p>
          {/each}
        {/if}

        {#if d?.pools?.pools.length}
          <div class="pools">
            {#each d.pools.pools as p (`${p.kind}/${p.name}`)}
              <div class="pool">
                <div class="prow">
                  <span class="kind">{POOL_WORD[p.kind]}</span>
                  <strong>{p.name}</strong>
                  {#if p.detail}<span class="muted">{p.detail}</span>{/if}
                  <span class="state {p.health}">{p.state.toUpperCase()}</span>
                  {#if p.size}<span class="muted small">{p.used != null ? `${size(p.used)} of ` : ""}{size(p.size)}</span>{/if}
                  <span class="spacer"></span>
                  <span class="devs">
                    {#each p.devices as dev (dev.name)}
                      <span class="dev" class:badd={!["ONLINE", "active", "present", "spare", "DEGRADED", "AVAIL"].includes(dev.state)} title="{dev.state}{dev.errors ? ` · errors ${dev.errors}` : ''}"
                        >{dev.name}</span
                      >
                    {/each}
                  </span>
                </div>
                {#each p.issues as i (i)}<p class="issue {p.health}">{i}</p>{/each}
                {#if p.activity}
                  <p class="activity muted small">
                    {p.activity}
                    {#if p.progress != null}<span class="prog"><span style="width:{p.progress}%"></span></span> {p.progress.toFixed(1)}%{/if}
                  </p>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if d?.forecasts.length}
          <table class="mounts">
            <tbody>
              {#each d.forecasts as f (f.mount)}
                {@const tone = fillTone(f, within)}
                <tr>
                  <td class="mount">{f.mount}</td>
                  <td class="use">
                    <span class="meter"><span class="fill t-{tone}" style="width:{usedPct(f)}%"></span></span>
                    <span class="small muted">{size(f.used)} / {size(f.size)}</span>
                  </td>
                  <td class="rate small">{rateText(f.perDay)}</td>
                  <td class="spark">
                    {#if f.daily.length > 1}
                      <svg width={W} height={H} viewBox="0 0 {W} {H}" aria-hidden="true"><path d={sparkPath(f.daily, W, H, f.size * 0.02)} class="t-{tone}" /></svg>
                    {/if}
                  </td>
                  <td class="eta small t-{tone}">{fillText(f)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else if d && d.status !== "checking"}
          <p class="muted small pad">No usage history yet: it's recorded while the host is monitored (Overview), one point an hour.</p>
        {/if}
        {#if d?.status === "ok" && !d.pools?.pools.length && d.forecasts.length}
          <p class="muted small pad">No ZFS pools, RAID arrays or btrfs filesystems.</p>
        {/if}
      </section>
    {/each}
  </div>
</div>

<style>
  .disks {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .bar,
  header,
  .prow {
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
  .sum {
    letter-spacing: 0.1em;
  }
  .ok,
  .sum.ok,
  .state.ok {
    color: var(--ok);
  }
  .bad,
  .sum.bad,
  .state.bad,
  .issue.bad {
    color: var(--danger);
  }
  .sum.warn,
  .state.warn,
  .issue.warn {
    color: var(--warn);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .host {
    border: 1px solid var(--line);
    padding: 8px 10px;
  }
  .host.t-bad {
    border-left: 2px solid var(--danger);
  }
  .host.t-warn {
    border-left: 2px solid var(--warn);
  }
  .name {
    font-size: var(--fs-sm);
    color: var(--text-0);
    letter-spacing: 0.04em;
  }
  .chip {
    padding: 0 5px;
    border: 1px solid currentColor;
    letter-spacing: 0.08em;
    color: var(--ok);
  }
  .chip.warn {
    color: var(--warn);
  }
  .chip.bad {
    color: var(--danger);
  }
  .chip.unknown {
    color: var(--text-1);
  }
  .pools {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pool {
    padding-left: 8px;
    border-left: 1px solid var(--line);
  }
  .prow {
    font-size: var(--fs-sm);
  }
  .kind {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--cyan);
    width: 48px;
  }
  .state {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .devs {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .dev {
    font-size: var(--fs-xs);
    padding: 0 4px;
    border: 1px solid var(--line);
    color: var(--text-1);
  }
  .dev.badd {
    color: var(--danger);
    border-color: var(--danger);
  }
  .issue {
    margin: 2px 0 0 58px;
    font-size: var(--fs-xs);
  }
  header + .issue {
    margin-left: 0;
  }
  .activity {
    margin: 2px 0 0 58px;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .prog {
    display: inline-block;
    width: 120px;
    height: 4px;
    background: color-mix(in srgb, var(--line) 70%, transparent);
  }
  .prog span {
    display: block;
    height: 100%;
    background: var(--cyan);
  }
  .mounts {
    width: 100%;
    border-collapse: collapse;
    margin-top: 8px;
  }
  .mounts td {
    padding: 2px 8px 2px 0;
    font-size: var(--fs-sm);
    vertical-align: middle;
  }
  .mount {
    font-family: var(--font-mono);
    width: 22%;
    overflow-wrap: anywhere;
  }
  .use {
    width: 30%;
  }
  .use .meter {
    display: inline-block;
    width: 120px;
    height: 6px;
    background: color-mix(in srgb, var(--line) 70%, transparent);
    margin-right: 8px;
    vertical-align: middle;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--mint);
  }
  .fill.t-warn {
    background: var(--warn);
  }
  .fill.t-bad {
    background: var(--danger);
  }
  .rate {
    width: 12%;
    color: var(--text-1);
    white-space: nowrap;
  }
  .spark {
    width: 130px;
  }
  .spark path {
    fill: none;
    stroke: var(--cyan);
    stroke-width: 1.2;
  }
  .spark path.t-warn {
    stroke: var(--warn);
  }
  .spark path.t-bad {
    stroke: var(--danger);
  }
  .eta {
    color: var(--text-1);
    text-align: right;
    white-space: nowrap;
  }
  .eta.t-warn {
    color: var(--warn);
  }
  .eta.t-bad {
    color: var(--danger);
  }
  .pad {
    margin: 6px 0 0;
  }
</style>
