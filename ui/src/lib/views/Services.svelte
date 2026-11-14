<script lang="ts">
  import { onMount } from "svelte";
  import Panel from "../components/Panel.svelte";
  import ServiceCard from "../components/services/ServiceCard.svelte";
  import { settings } from "../stores/settings.svelte";
  import { services } from "../stores/services.svelte";

  onMount(() => services.start());

  const sum = $derived(services.summary);
  const groups = $derived(services.groups);
  let checkingAll = $state(false);

  async function checkAll() {
    checkingAll = true;
    try {
      await services.checkAll();
    } finally {
      setTimeout(() => (checkingAll = false), 1200);
    }
  }
</script>

<div class="services">
  <div class="toolbar" role="toolbar" aria-label="Services">
    <input class="input filter" placeholder="Filter services…" bind:value={services.filter} aria-label="Filter services" />
    <div class="tabs" role="radiogroup" aria-label="Group by">
      <button type="button" role="radio" aria-checked={services.groupBy === "group"} class:on={services.groupBy === "group"} onclick={() => services.setGroupBy("group")}>GROUP</button>
      <button type="button" role="radio" aria-checked={services.groupBy === "host"} class:on={services.groupBy === "host"} onclick={() => services.setGroupBy("host")}>HOST</button>
    </div>
    <span class="counts" aria-live="polite">
      <span class="ok">{sum.up} UP</span>
      {#if sum.warn}<span class="warn">{sum.warn} WARN</span>{/if}
      {#if sum.down}<span class="bad">{sum.down} DOWN</span>{/if}
      {#if sum.off}<span>{sum.off} UNCHECKED</span>{/if}
    </span>
    <span class="spacer"></span>
    {#if !settings.value.servicesChecks}
      <span class="paused" title="Background checks are off in Settings → Services">CHECKS PAUSED</span>
    {/if}
    <button type="button" class="btn ghost" disabled={checkingAll || !sum.total} onclick={checkAll}>{checkingAll ? "Checking…" : "Check all"}</button>
    <button type="button" class="btn" onclick={() => (services.importOpen = true)} data-guide="services-import">Import Homepage</button>
    <button type="button" class="btn primary" onclick={() => (services.editing = 0)} data-guide="services-add">Add service</button>
  </div>

  {#if services.error}<p class="error-text" role="alert">{services.error}</p>{/if}

  {#if services.loaded && !services.services.length}
    <Panel title="NO SERVICES" prefix=">">
      <div class="empty">
        <p class="label">BOARD IS EMPTY</p>
        <p class="muted">
          Add the web apps you run (anything with an http or https URL) to see them up or down at a glance, with
          latency, 24-hour uptime and certificate expiry. Already use Homepage? Import its <code>services.yaml</code>.
        </p>
        <div class="actions">
          <button type="button" class="btn primary chamfer-sm" onclick={() => (services.editing = 0)}>Add service</button>
          <button type="button" class="btn chamfer-sm" onclick={() => (services.importOpen = true)}>Import Homepage</button>
        </div>
      </div>
    </Panel>
  {:else if services.loaded && !groups.length}
    <p class="muted">No services match “{services.filter}”.</p>
  {:else}
    {#each groups as g (g.key)}
      <Panel title={g.label.toUpperCase()} prefix=">" count={g.services.length}>
        {#snippet actions()}
          {#if g.checked}<span class="gstat" class:bad={g.up < g.checked}>{g.up}/{g.checked} UP</span>{/if}
        {/snippet}
        <div class="cards">
          {#each g.services as s (s.id)}
            <ServiceCard service={s} showHost={services.groupBy !== "host"} />
          {/each}
        </div>
      </Panel>
    {/each}
  {/if}
</div>

<style>
  .services {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .filter {
    width: 220px;
  }
  .tabs {
    display: flex;
    border: 1px solid var(--line);
  }
  .tabs button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 10px;
    cursor: pointer;
  }
  .tabs button + button {
    border-left: 1px solid var(--line);
  }
  .tabs button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .counts {
    display: flex;
    gap: 10px;
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .ok {
    color: var(--ok);
  }
  .warn,
  .paused {
    color: var(--warn);
  }
  .paused {
    letter-spacing: 0.1em;
  }
  .bad {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 8px;
  }
  .gstat {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--ok);
  }
  .gstat.bad {
    color: var(--warn);
  }
  .empty p {
    margin: 0 0 10px;
    line-height: 1.6;
    max-width: 70ch;
  }
  .empty .label {
    color: var(--text-0);
    font-size: var(--fs-lg);
  }
  code {
    color: var(--cyan);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .error-text {
    margin: 0;
  }
</style>
