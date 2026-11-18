<script lang="ts">
  /** Fleet (M10m): health across every host at once. */
  import DiskHealth from "../components/fleet/DiskHealth.svelte";
  import BackupsPanel from "../components/system/BackupsPanel.svelte";
  import SudoPrompt from "../components/system/SudoPrompt.svelte";
  import { backups, jobState } from "../stores/backups.svelte";
  import UpdateCenter from "../components/fleet/UpdateCenter.svelte";
  import UpsPanel from "../components/fleet/UpsPanel.svelte";
  import UptimePanel from "../components/fleet/UptimePanel.svelte";
  import LogSearchPanel from "../components/fleet/LogSearchPanel.svelte";
  import AccessPanel from "../components/fleet/AccessPanel.svelte";
  import SpeedPanel from "../components/fleet/SpeedPanel.svelte";
  import JournalPanel from "../components/fleet/JournalPanel.svelte";
  import BlueprintPanel from "../components/fleet/BlueprintPanel.svelte";
  import PowerPanel from "../components/fleet/PowerPanel.svelte";
  import NodePanel from "../components/fleet/NodePanel.svelte";
  import CatalogPanel from "../components/fleet/CatalogPanel.svelte";
  import DrillsPanel from "../components/fleet/DrillsPanel.svelte";
  import CapacityPanel from "../components/fleet/CapacityPanel.svelte";
  import ReplayPanel from "../components/fleet/ReplayPanel.svelte";
  import { notices } from "../stores/notices.svelte";
  import { alerts } from "../stores/alerts.svelte";
  import { diskTone, type FleetTab } from "../fleet";
  import { settings } from "../stores/settings.svelte";
  import { fleet } from "../stores/fleet.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { ui } from "../stores/ui.svelte";

  const TABS: { id: FleetTab; label: string }[] = [
    { id: "updates", label: "UPDATES" },
    { id: "disks", label: "DISKS" },
    { id: "backups", label: "BACKUPS" },
    { id: "ups", label: "UPS" },
    { id: "uptime", label: "UPTIME" },
    { id: "logs", label: "LOGS" },
    { id: "access", label: "ACCESS" },
    { id: "speed", label: "SPEED" },
    { id: "journal", label: "JOURNAL" },
    { id: "blueprints", label: "BLUEPRINTS" },
    { id: "power", label: "POWER" },
    { id: "node", label: "NODE" },
    { id: "catalog", label: "CATALOG" },
    { id: "drills", label: "DRILLS" },
    { id: "capacity", label: "CAPACITY" },
    { id: "replay", label: "REPLAY" },
  ];
  // Tabs turned off in Settings → Features are hidden.
  const tabs = $derived(TABS.filter((t) => settings.on(`fleet.${t.id}`)));
  $effect(() => {
    if (tabs.length && !tabs.some((t) => t.id === fleet.tab)) open(tabs[0].id);
  });
  const upsBad = $derived(alerts.active.filter((a) => a.kind === "upsOnBattery").length + (alerts.snap?.upsCountdown ? 1 : 0));
  const backupBad = $derived(backups.jobs.filter((j) => ["failed", "error", "stale"].includes(jobState(backups.status[j.id]))).length);
  // A notice opened this view on a tab.
  $effect(() => {
    const t = notices.wantTab && notices.takeTab("fleet");
    if (t && TABS.some((x) => x.id === t)) open(t as FleetTab);
  });
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
      {#each tabs as t (t.id)}
        <button type="button" role="tab" data-guide="fleet-tab-{t.id}" class:on={fleet.tab === t.id} aria-selected={fleet.tab === t.id} onclick={() => open(t.id)}>
          {t.label}{#if t.id === "updates" && pending}<span class="badge">{pending}</span>{/if}{#if t.id === "disks" && diskBad !== "ok"}<span class="badge {diskBad}">!</span>{/if}{#if t.id === "backups" && backupBad}<span class="badge bad">{backupBad}</span>{/if}{#if t.id === "ups" && upsBad}<span class="badge bad">!</span>{/if}
        </button>
      {/each}
    </div>
    <span class="muted small">{hosts.hosts.length} host{hosts.hosts.length === 1 ? "" : "s"}</span>
  </div>
  <div class="pane">
    {#if !hosts.hosts.length && fleet.tab !== "journal"}
      <div class="empty">
        <p class="label">NO HOSTS</p>
        <p class="muted">Add a host first; pending updates, disk health, backups, UPSes and uptime across all of them show up here.</p>
        <button type="button" class="btn primary" onclick={() => (hosts.editing = 0)}>Add host</button>
      </div>
    {:else if !tabs.length}
      <div class="empty">
        <p class="label">EVERY FLEET CHECK IS OFF</p>
        <p class="muted">Turn some on in Settings → Features.</p>
        <button type="button" class="btn" onclick={() => ((ui.settingsTab = "features"), ui.go("settings"))}>Open Features</button>
      </div>
    {:else if fleet.tab === "updates"}
      <UpdateCenter />
    {:else if fleet.tab === "disks"}
      <DiskHealth />
    {:else if fleet.tab === "backups"}
      <BackupsPanel />
    {:else if fleet.tab === "ups"}
      <UpsPanel />
    {:else if fleet.tab === "uptime"}
      <UptimePanel />
    {:else if fleet.tab === "logs"}
      <LogSearchPanel />
    {:else if fleet.tab === "access"}
      <AccessPanel />
    {:else if fleet.tab === "speed"}
      <SpeedPanel />
    {:else if fleet.tab === "journal"}
      <JournalPanel />
    {:else if fleet.tab === "blueprints"}
      <BlueprintPanel />
    {:else if fleet.tab === "power"}
      <PowerPanel />
    {:else if fleet.tab === "node"}
      <NodePanel />
    {:else if fleet.tab === "catalog"}
      <CatalogPanel />
    {:else if fleet.tab === "drills"}
      <DrillsPanel />
    {:else if fleet.tab === "capacity"}
      <CapacityPanel />
    {:else if fleet.tab === "replay"}
      <ReplayPanel />
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
    flex-wrap: wrap;
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
