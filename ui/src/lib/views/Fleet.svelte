<script lang="ts">
  /** Fleet (M10m): health across every host at once. */
  import DiskHealth from "../components/fleet/DiskHealth.svelte";
  import BackupsPanel from "../components/system/BackupsPanel.svelte";
  import SudoPrompt from "../components/system/SudoPrompt.svelte";
  import { backups, jobState } from "../stores/backups.svelte";
  import UpdateCenter from "../components/fleet/UpdateCenter.svelte";
  import { diskTone, type FleetTab } from "../fleet";
  import { settings } from "../stores/settings.svelte";
  import { fleet } from "../stores/fleet.svelte";
  import { hosts } from "../stores/hosts.svelte";

  const TABS: { id: FleetTab; label: string }[] = [
    { id: "updates", label: "UPDATES" },
    { id: "disks", label: "DISKS" },
    { id: "backups", label: "BACKUPS" },
  ];
  const backupBad = $derived(backups.jobs.filter((j) => ["failed", "error", "stale"].includes(jobState(backups.status[j.id]))).length);
  function open(t: FleetTab) {
    if (t === "backups") fleet.openBackups();
    else fleet.tab = t;
  }
  // Coming back to the view on the BACKUPS tab still reads the board once.
  $effect(() => {
    if (fleet.tab === "backups" && !backups.loaded) fleet.openBackups();
  });
  const diskBad = $derived(
    Object.values(fleet.disks).reduce((worst, d) => {
      const t = diskTone(d, settings.value.alerts.forecastDays);
      return t === "bad" || worst === "bad" ? "bad" : t === "warn" || worst === "warn" ? "warn" : worst;
    }, "ok"),
  );
  const pending = $derived(Object.values(fleet.updates).reduce((n, s) => n + (s.report?.updates.length ?? 0), 0));
</script>

<div class="fleet">
  <div class="toolbar">
    <div class="tabs" role="tablist" aria-label="Fleet view">
      {#each TABS as t (t.id)}
        <button type="button" role="tab" data-guide="fleet-tab-{t.id}" class:on={fleet.tab === t.id} aria-selected={fleet.tab === t.id} onclick={() => open(t.id)}>
          {t.label}{#if t.id === "updates" && pending}<span class="badge">{pending}</span>{/if}{#if t.id === "disks" && diskBad !== "ok"}<span class="badge {diskBad}">!</span>{/if}{#if t.id === "backups" && backupBad}<span class="badge bad">{backupBad}</span>{/if}
        </button>
      {/each}
    </div>
    <span class="muted small">{hosts.hosts.length} host{hosts.hosts.length === 1 ? "" : "s"}</span>
  </div>
  <div class="pane">
    {#if !hosts.hosts.length}
      <div class="empty">
        <p class="label">NO HOSTS</p>
        <p class="muted">Add a host first; pending updates, disk health and backups across all of them show up here.</p>
        <button type="button" class="btn primary" onclick={() => (hosts.editing = 0)}>Add host</button>
      </div>
    {:else if fleet.tab === "updates"}
      <UpdateCenter />
    {:else if fleet.tab === "disks"}
      <DiskHealth />
    {:else if fleet.tab === "backups"}
      <BackupsPanel />
    {/if}
  </div>
</div>

<SudoPrompt />

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
  .badge.warn {
    background: var(--warn);
  }
  .badge.bad {
    background: var(--danger);
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
