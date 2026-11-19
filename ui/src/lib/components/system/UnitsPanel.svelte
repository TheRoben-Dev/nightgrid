<script lang="ts">
  import type { Unit } from "../../api";
  import { actionVerb, system, UNIT_ACTIONS, unitCounts, unitTone, type UnitFilter } from "../../stores/system.svelte";

  const counts = $derived(unitCounts(system.units));
  const list = $derived(system.filteredUnits);
  const selected = $derived(system.selectedUnit ? system.units.find((u) => u.name === system.selectedUnit) : undefined);
  const FILTERS: { id: UnitFilter; label: string }[] = [
    { id: "all", label: "ALL" },
    { id: "running", label: "ACTIVE" },
    { id: "failed", label: "FAILED" },
    { id: "inactive", label: "INACTIVE" },
  ];

  /** Actions that make sense for the unit's current state. */
  function actionsFor(u: Unit) {
    const up = u.active === "active" || u.active === "activating" || u.active === "reloading";
    const enabled = u.fileState === "enabled" || u.fileState === "enabled-runtime";
    const canToggle = u.fileState === "enabled" || u.fileState === "disabled";
    return UNIT_ACTIONS.filter((a) => {
      if (a === "start") return !up && u.load !== "masked";
      if (a === "stop" || a === "restart" || a === "reload") return up;
      if (a === "enable") return canToggle && !enabled;
      if (a === "disable") return canToggle && enabled;
      return false;
    });
  }

  function onKey(e: KeyboardEvent, u: Unit) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      void system.selectUnit(system.selectedUnit === u.name ? null : u.name);
    }
  }
</script>

<div class="units" class:with-side={!!selected}>
  <div class="bar">
    <div class="seg tabstrip sm" role="radiogroup" aria-label="Unit filter">
      {#each FILTERS as f (f.id)}
        <button type="button" role="radio" aria-checked={system.unitFilter === f.id} class:on={system.unitFilter === f.id} onclick={() => (system.unitFilter = f.id)}>
          {f.label}{#if f.id === "failed" && counts.failed}<span class="bad">&nbsp;{counts.failed}</span>{/if}
        </button>
      {/each}
    </div>
    <span class="counts"><span class="mint">{counts.running}</span> RUNNING · {counts.total} SERVICES</span>
    <span class="spacer"></span>
    <input class="input filter" type="search" placeholder="Filter units" bind:value={system.unitQuery} aria-label="Filter units" />
    <button type="button" class="btn ghost tiny" onclick={() => system.loadUnits()} disabled={system.unitsLoading}>{system.unitsLoading ? "…" : "Refresh"}</button>
  </div>

  <div class="split">
    <div class="list" role="listbox" aria-label="Service units" data-guide="system-units">
      {#if system.errors.units}
        <p class="error-text pad">{system.errors.units}</p>
      {:else if system.unitsLoading && !system.unitsLoaded}
        <p class="muted pad">Loading units…</p>
      {:else if !list.length}
        <p class="muted pad">{system.units.length ? "No units match." : "No service units."}</p>
      {/if}
      {#each list as u (u.name)}
        {@const tone = unitTone(u)}
        <div
          class="row {tone}"
          class:sel={system.selectedUnit === u.name}
          role="option"
          tabindex="0"
          aria-selected={system.selectedUnit === u.name}
          onclick={() => system.selectUnit(system.selectedUnit === u.name ? null : u.name)}
          onkeydown={(e) => onKey(e, u)}
        >
          <span class="dot" aria-hidden="true"></span>
          <span class="name" title={u.name}>{u.name.replace(/\.service$/, "")}</span>
          <span class="state">{u.active}<span class="sub">/{u.sub}</span></span>
          <span class="file" class:enabled={u.fileState === "enabled"}>{u.fileState ?? "—"}</span>
          <span class="desc" title={u.description}>{u.description}</span>
        </div>
      {/each}
    </div>

    {#if selected}
      <aside class="side" aria-label="Unit {selected.name}">
        <header>
          <h3 title={selected.name}>{selected.name}</h3>
          <button type="button" class="x" aria-label="Close" onclick={() => system.selectUnit(null)}>×</button>
        </header>
        <div class="actions">
          {#each actionsFor(selected) as a (a)}
            <button
              type="button"
              class="btn tiny"
              class:danger={a === "stop" || a === "disable"}
              class:ghost={a !== "stop" && a !== "disable"}
              disabled={system.unitBusy.includes(selected.name)}
              onclick={() => system.request(selected.name, a)}>{actionVerb(a)}</button
            >
          {/each}
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" onclick={() => system.openLogs(selected.name)}>Logs</button>
        </div>
        <pre class="status">{system.unitStatus ?? "…"}</pre>
      </aside>
    {/if}
  </div>
</div>

<style>
  .units {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .actions {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .counts {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .mint {
    color: var(--mint);
  }
  .bad {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .filter {
    width: 200px;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 6px;
  }
  .list {
    flex: 1;
    min-width: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .pad {
    padding: 12px;
    margin: 0;
  }
  .row {
    --tone: var(--text-1);
    display: grid;
    grid-template-columns: 10px minmax(140px, 1.2fr) 140px 80px minmax(0, 2fr);
    gap: 10px;
    align-items: center;
    padding: 4px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .with-side .row {
    grid-template-columns: 10px minmax(120px, 1fr) 130px 70px;
  }
  .with-side .desc {
    display: none;
  }
  .row:hover {
    background: var(--bg-1);
  }
  .row.sel {
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .row:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
  .row.ok {
    --tone: var(--ok);
  }
  .row.bad {
    --tone: var(--danger);
  }
  .row.warn {
    --tone: var(--warn);
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--tone);
  }
  .row.off .dot {
    background: transparent;
    border: 1px solid var(--text-1);
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    color: var(--tone);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
  }
  .row.off .state {
    color: var(--text-1);
  }
  .sub {
    color: var(--text-1);
  }
  .file {
    color: var(--text-1);
    font-size: var(--fs-xs);
  }
  .file.enabled {
    color: var(--mint);
  }
  .desc {
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .side {
    flex: 0 0 46%;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h3 {
    margin: 0;
    flex: 1;
    font-family: var(--font-display);
    font-size: var(--fs-md);
    letter-spacing: 0.06em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: var(--fs-lg);
  }
  .status {
    flex: 1;
    margin: 0;
    padding: 8px;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
    color: var(--cyan);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
