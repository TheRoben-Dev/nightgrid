<script lang="ts">
  /** Fleet (M10m): health across every host at once. */
  import UpdateCenter from "../components/fleet/UpdateCenter.svelte";
  import type { FleetTab } from "../fleet";
  import { fleet } from "../stores/fleet.svelte";
  import { hosts } from "../stores/hosts.svelte";

  const TABS: { id: FleetTab; label: string }[] = [{ id: "updates", label: "UPDATES" }];
  const pending = $derived(Object.values(fleet.updates).reduce((n, s) => n + (s.report?.updates.length ?? 0), 0));
</script>

<div class="fleet">
  <div class="toolbar">
    <div class="tabs" role="tablist" aria-label="Fleet view">
      {#each TABS as t (t.id)}
        <button type="button" role="tab" data-guide="fleet-tab-{t.id}" class:on={fleet.tab === t.id} aria-selected={fleet.tab === t.id} onclick={() => (fleet.tab = t.id)}>
          {t.label}{#if t.id === "updates" && pending}<span class="badge">{pending}</span>{/if}
        </button>
      {/each}
    </div>
    <span class="muted small">{hosts.hosts.length} host{hosts.hosts.length === 1 ? "" : "s"}</span>
  </div>
  <div class="pane">
    {#if !hosts.hosts.length}
      <div class="empty">
        <p class="label">NO HOSTS</p>
        <p class="muted">Add a host first; pending updates across all of them show up here.</p>
        <button type="button" class="btn primary" onclick={() => (hosts.editing = 0)}>Add host</button>
      </div>
    {:else if fleet.tab === "updates"}
      <UpdateCenter />
    {/if}
  </div>
</div>

<style>
  .fleet {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .small {
    font-size: var(--fs-xs);
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
  .badge {
    margin-left: 6px;
    padding: 0 4px;
    color: var(--bg-0);
    background: var(--cyan);
  }
  .pane {
    flex: 1;
    min-height: 0;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 20px;
  }
</style>
