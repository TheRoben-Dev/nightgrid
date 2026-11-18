<script lang="ts">
  /** Security (1.1): a posture score per host, and every secret's age and rotation. */
  import PosturePanel from "../components/security/PosturePanel.svelte";
  import SecretsPanel from "../components/security/SecretsPanel.svelte";
  import { notices } from "../stores/notices.svelte";
  import { security, type SecurityTab } from "../stores/security.svelte";
  import { settings } from "../stores/settings.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { ui } from "../stores/ui.svelte";

  const TABS: { id: SecurityTab; label: string }[] = [
    { id: "posture", label: "POSTURE" },
    { id: "secrets", label: "SECRETS" },
  ];
  const tabs = $derived(TABS.filter((t) => settings.on(`security.${t.id}`)));
  $effect(() => {
    if (tabs.length && !tabs.some((t) => t.id === security.tab)) security.tab = tabs[0].id;
  });
  // A notice opened this view on a tab.
  $effect(() => {
    const t = notices.wantTab && notices.takeTab("security");
    if (t === "posture" || t === "secrets") security.tab = t;
  });
  const below = $derived(security.posture.filter((p) => (p.latest.score ?? 100) < 75).length);
</script>

<div class="security">
  <div class="toolbar">
    <div class="tabs" role="tablist" aria-label="Security view">
      {#each tabs as t (t.id)}
        <button type="button" role="tab" data-guide="security-tab-{t.id}" class:on={security.tab === t.id} aria-selected={security.tab === t.id} onclick={() => (security.tab = t.id)}>
          {t.label}{#if t.id === "posture" && below}<span class="badge warn">{below}</span>{/if}
        </button>
      {/each}
    </div>
    <span class="muted small">{hosts.hosts.length} host{hosts.hosts.length === 1 ? "" : "s"}</span>
  </div>
  <div class="pane">
    {#if !tabs.length}
      <div class="empty">
        <p class="label">BOTH SECURITY TABS ARE OFF</p>
        <button type="button" class="btn" onclick={() => ((ui.settingsTab = "features"), ui.go("settings"))}>Open Features</button>
      </div>
    {:else if security.tab === "posture"}
      <PosturePanel />
    {:else}
      <SecretsPanel />
    {/if}
  </div>
</div>

<style>
  .security {
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
    background: var(--warn);
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
