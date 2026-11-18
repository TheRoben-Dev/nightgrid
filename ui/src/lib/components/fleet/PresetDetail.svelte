<script lang="ts">
  /**
   * One starter preset in the gallery: what it is for, exactly what it would
   * change in the app, and an add button behind a confirmation. Nothing on a
   * host changes; tiles only read once added.
   */
  import { api, errorText, type StarterPreset } from "../../api";
  import { defaultDashboards } from "../../dashboard";
  import { CATALOG } from "../../kb/catalog";
  import { alertChanges, dashboardRoom, needsHost, presetConfirm, resolveDashboard, tileWidget, withAlerts } from "../../presets";
  import { categoryLabel } from "../../starters";
  import { cmdWidgets } from "../../stores/cmdwidgets.svelte";
  import { fleet } from "../../stores/fleet.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { library } from "../../stores/library.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";

  let { preset, onback }: { preset: StarterPreset; onback: () => void } = $props();

  let hostId = $state<number | null>(null);
  let confirming = $state(false);
  let busy = $state(false);
  let done = $state<string | null>(null);
  let err = $state<string | null>(null);

  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client"));
  const nameOf = (id: number | null) => hosts.hosts.find((h) => h.id === id)?.name ?? "";
  const articleTitle = (id: string) => CATALOG.find((e) => e.id === id)?.title ?? id;
  const changes = $derived(alertChanges(preset, settings.value.alerts));
  const boards = $derived(settings.value.dashboards?.length ? settings.value.dashboards : defaultDashboards());
  const blocked = $derived.by(() => {
    if (needsHost(preset) && hostId == null) return "Pick the host the tiles run on.";
    if (preset.kind === "dashboard") return dashboardRoom(boards.length);
    if (preset.kind === "alerts" && changes.every((c) => c.same)) return "Your alert settings already match.";
    return null;
  });

  async function add() {
    busy = true;
    confirming = false;
    err = null;
    done = null;
    try {
      if (preset.kind === "alerts") {
        await settings.update({ alerts: withAlerts(preset, $state.snapshot(settings.value.alerts)) });
        if (settings.error) throw new Error(settings.error);
        done = "Alert settings changed.";
      } else if (preset.kind === "searches") {
        for (const s of preset.searches) await api.logSearchSave({ id: null, name: s.name, query: { ...s.query, hosts: [] }, alert: s.alert });
        done = `Saved in Fleet → LOGS.`;
      } else {
        const ids: number[] = [];
        for (const t of preset.tiles) ids.push((await cmdWidgets.save(tileWidget(t, hostId!))).id);
        if (preset.kind === "dashboard" && preset.dashboard) {
          const d = resolveDashboard(preset.dashboard, ids, boards.map((b) => b.name));
          await settings.update({ dashboards: [...$state.snapshot(boards), d], dashboardActive: boards.length });
          if (settings.error) throw new Error(settings.error);
          done = `“${d.name}” added to the Overview.`;
        } else {
          done = `${ids.length} tiles added. Place them on a dashboard from the Overview's edit mode.`;
        }
      }
    } catch (e) {
      err = errorText(e);
    }
    busy = false;
  }
</script>

<div class="detail">
  <section class="col">
    <div class="row">
      <button type="button" class="btn ghost tiny" onclick={onback}>← All starters</button>
      <span class="cat">{categoryLabel(preset.category).toUpperCase()}</span>
    </div>
    <h3 class="big">{preset.title}</h3>
    {#each preset.purpose.trim().split(/\n\s*\n/) as para (para)}<p class="small para">{para.replace(/\s*\n\s*/g, " ")}</p>{/each}

    {#if preset.library.length}
      <h4 class="label">LIBRARY</h4>
      <div class="chips">
        {#each preset.library as a (a)}<button type="button" class="chip" onclick={() => library.open(a)}>▤ {articleTitle(a)}</button>{/each}
      </div>
    {/if}
  </section>

  <section class="col">
    <h4 class="label">WHAT IT ADDS</h4>
    {#if changes.length}
      <table class="small tbl">
        <thead><tr><th>Setting</th><th>Now</th><th>After</th></tr></thead>
        <tbody>
          {#each changes as c (c.key)}<tr class:same={c.same}><td>{c.label}</td><td class="muted">{c.from}</td><td>{c.to}</td></tr>{/each}
        </tbody>
      </table>
    {/if}
    {#if preset.dashboard}
      <p class="small"><span class="k">DASHBOARD</span> {preset.dashboard.name} <span class="muted">({preset.dashboard.widgets.map((w) => (w.kind.startsWith("tile:") ? preset.tiles[Number(w.kind.slice(5))]?.name : w.kind)).join(", ")})</span></p>
    {/if}
    {#if preset.tiles.length}
      <ul class="small list">
        {#each preset.tiles as t (t.name)}
          <li><span class="k">TILE</span> {t.name} <span class="muted">{t.what}</span><code class="cmd">{t.command}</code></li>
        {/each}
      </ul>
    {/if}
    {#if preset.searches.length}
      <ul class="small list">
        {#each preset.searches as s (s.name)}
          <li><span class="k">SEARCH</span> {s.name}{#if s.alert}<span class="warn"> · alerts on new matches</span>{/if} <span class="muted">{s.what}</span><code class="cmd">{s.query.text}</code></li>
        {/each}
      </ul>
    {/if}

    {#if needsHost(preset)}
      <label class="field">Host the tiles run on
        <select class="input" value={hostId} onchange={(e) => ((hostId = (e.currentTarget as HTMLSelectElement).value === "" ? null : Number((e.currentTarget as HTMLSelectElement).value)), (done = null))}>
          <option value="">pick a host…</option>
          {#each servers as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
        </select>
      </label>
    {/if}
    {#if blocked}<p class="muted small">{blocked}</p>{/if}

    {#if confirming}
      <div class="confirm" role="alertdialog" aria-label="Confirm">
        <p class="small">{presetConfirm(preset, nameOf(hostId))}</p>
        <div class="row">
          <button type="button" class="btn primary" disabled={busy} onclick={() => void add()}>Yes, add</button>
          <button type="button" class="btn ghost" onclick={() => (confirming = false)}>Cancel</button>
        </div>
      </div>
    {:else}
      <div class="row">
        <button type="button" class="btn primary" disabled={!!blocked || busy || !!done} onclick={() => (confirming = true)}>{busy ? "Adding…" : preset.kind === "alerts" ? "Apply these settings…" : "Add…"}</button>
        {#if done && preset.kind === "dashboard"}<button type="button" class="btn ghost" onclick={() => ui.go("overview")}>Open the Overview</button>{/if}
        {#if done && preset.kind === "searches"}<button type="button" class="btn ghost" onclick={() => ((fleet.tab = "logs"), ui.go("fleet"))}>Open Fleet → LOGS</button>{/if}
      </div>
    {/if}
    {#if done}<p class="ok small">{done}</p>{/if}
    {#if err}<p class="bad small" role="alert">{err}</p>{/if}
  </section>
</div>

<style>
  .detail {
    display: grid;
    grid-template-columns: minmax(280px, 1.1fr) minmax(300px, 1fr);
    gap: 18px;
  }
  @media (max-width: 900px) {
    .detail {
      grid-template-columns: 1fr;
    }
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .cat {
    color: var(--magenta);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .big {
    margin: 0;
    font-size: var(--fs-lg, 18px);
    color: var(--text-0);
  }
  .label {
    margin: 6px 0 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .para {
    color: var(--text-0);
    line-height: 1.5;
  }
  .muted {
    color: var(--text-1);
  }
  .ok {
    color: var(--ok, #4ade80);
  }
  .bad {
    color: var(--danger);
  }
  .warn {
    color: var(--warn);
  }
  .k {
    display: inline-block;
    min-width: 76px;
    color: var(--cyan);
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 6px;
  }
  .cmd {
    display: block;
    margin-top: 2px;
    font-family: var(--font-mono);
    color: var(--text-1);
    white-space: pre-wrap;
    word-break: break-all;
  }
  .tbl {
    border-collapse: collapse;
  }
  .tbl th {
    text-align: left;
    color: var(--text-1);
    font-weight: normal;
    padding: 2px 10px 2px 0;
  }
  .tbl td {
    padding: 2px 10px 2px 0;
    color: var(--text-0);
  }
  .tbl tr.same td {
    opacity: 0.5;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 1px 8px;
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text-0);
    border-color: var(--cyan);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .confirm {
    border: 1px solid var(--warn);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
