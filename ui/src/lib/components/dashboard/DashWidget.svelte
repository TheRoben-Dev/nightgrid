<script lang="ts">
  /**
   * One dashboard widget's content (M10o). Widgets only show what the app
   * already knows; ones that need a check over SSH (updates, disks, backups)
   * offer a button instead of starting work when the Overview opens.
   */
  import type { AppInfo } from "../../api";
  import { diskRows, totals, updateRows } from "../../fleet";
  import { widgetInfo } from "../../dashboard";
  import { DEFAULT_KEYMAP, formatChord } from "../../keys";
  import { alerts } from "../../stores/alerts.svelte";
  import { backups, jobState } from "../../stores/backups.svelte";
  import { desktopLabel, desktops } from "../../stores/desktops.svelte";
  import { fleet } from "../../stores/fleet.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { notes } from "../../stores/notes.svelte";
  import { healthOf, services } from "../../stores/services.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { tunnels } from "../../stores/tunnels.svelte";
  import { ui } from "../../stores/ui.svelte";
  import HostTile from "../metrics/HostTile.svelte";

  let { kind, info }: { kind: string; info: AppInfo | null } = $props();

  $effect(() => {
    if (kind === "notes" && !notes.loaded) void notes.load();
  });

  const hostName = (id: number | null) => (id == null ? "" : (hosts.hosts.find((h) => h.id === id)?.name ?? ""));
  const sevRank = (s: string) => (s === "critical" ? 0 : 1);
  const activeAlerts = $derived([...alerts.active].sort((a, b) => sevRank(a.severity) - sevRank(b.severity) || b.since - a.since));
  const svcDown = $derived(services.services.filter((s) => ["down", "warn"].includes(healthOf(s, services.status[s.id]))));
  const upd = $derived(totals(updateRows(hosts.hosts, fleet.updates)));
  const updChecked = $derived(Object.keys(fleet.updates).length > 0);
  const disk = $derived(diskRows(hosts.hosts, fleet.disks, settings.value.alerts.forecastDays));
  const diskBad = $derived(disk.filter((r) => r.tone === "bad" || r.tone === "warn"));
  const bkBad = $derived(backups.jobs.filter((j) => ["failed", "error", "stale"].includes(jobState(backups.status[j.id]))));
  const pinned = $derived(notes.notes.filter((n) => n.pinned));
  const goFleet = (tab: "updates" | "disks" | "backups") => {
    if (tab === "backups") fleet.openBackups();
    else fleet.tab = tab;
    ui.go("fleet");
  };
</script>

{#if kind === "nodes"}
  {#if hosts.hosts.length}
    <div class="hostgrid">
      {#each hosts.hosts as h (h.id)}<HostTile host={h} ping={hosts.pings[h.id]} />{/each}
    </div>
  {:else}
    <div class="empty">
      <p class="label">NO NODES REGISTERED</p>
      <p class="muted">Add a host or import your <code>~/.ssh/config</code>. Open the palette with <kbd>{formatChord(DEFAULT_KEYMAP["palette.open"])}</kbd> to look around.</p>
      <div class="actions">
        <button class="btn primary chamfer-sm" type="button" onclick={() => (hosts.editing = 0)}>Add host</button>
        <button class="btn chamfer-sm" type="button" onclick={() => (hosts.importOpen = true)}>Import ssh config</button>
      </div>
    </div>
  {/if}
{:else if kind === "alerts"}
  {#if activeAlerts.length}
    <ul class="list">
      {#each activeAlerts as a (a.id)}
        <li>
          <button type="button" class="row" onclick={() => alerts.go(a)}>
            <span class="sev {a.severity}">{a.severity === "critical" ? "CRIT" : "WARN"}</span>
            <span class="txt" title={a.detail}>{a.title}</span>
            <span class="muted small">{hostName(a.hostId)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="calm"><span class="ok">●</span> No active alerts.</p>
  {/if}
{:else if kind === "services"}
  {@const s = services.summary}
  <p class="big"><span class="ok">{s.up}</span><span class="muted">/{s.total} up</span>{#if s.down}<span class="bad"> · {s.down} down</span>{/if}{#if s.warn}<span class="warn"> · {s.warn} warn</span>{/if}</p>
  {#if svcDown.length}
    <ul class="list">
      {#each svcDown as v (v.id)}
        <li><button type="button" class="row" onclick={() => ui.go("services")}><span class="txt">{v.name}</span><span class="small {healthOf(v, services.status[v.id]) === 'down' ? 'bad' : 'warn'}">{healthOf(v, services.status[v.id]).toUpperCase()}</span></button></li>
      {/each}
    </ul>
  {:else if !s.total}
    <p class="muted small">No services on the board yet. <button type="button" class="link" onclick={() => ui.go("services")}>Add some</button></p>
  {/if}
{:else if kind === "updates"}
  {#if updChecked}
    <p class="big">{upd.packages}<span class="muted small"> pending on {upd.hosts} host{upd.hosts === 1 ? "" : "s"}</span></p>
    <p class="small">
      {#if upd.security}<span class="bad">{upd.security} security</span> ·{/if}
      {#if upd.reboot}<span class="warn">{upd.reboot} need a reboot</span> ·{/if}
      {#if upd.failed}<span class="muted">{upd.failed} couldn't check</span>{/if}
    </p>
  {:else}
    <p class="muted small">Not checked yet.</p>
  {/if}
  <div class="foot"><button type="button" class="btn ghost tiny" disabled={fleet.checking} onclick={() => fleet.checkAll()}>{fleet.checking ? "Checking…" : "Check all"}</button><button type="button" class="link" onclick={() => goFleet("updates")}>Fleet → Updates</button></div>
{:else if kind === "disks"}
  {#if fleet.disksAt}
    {#if diskBad.length}
      <ul class="list">
        {#each diskBad as r (r.host.id)}<li><button type="button" class="row" onclick={() => goFleet("disks")}><span class="txt">{r.host.name}</span><span class="small {r.tone}">{r.tone === "bad" ? "NEEDS ATTENTION" : "WATCH"}</span></button></li>{/each}
      </ul>
    {:else}
      <p class="calm"><span class="ok">●</span> All pools and mounts healthy.</p>
    {/if}
  {:else}
    <p class="muted small">Not read yet.</p>
  {/if}
  <div class="foot"><button type="button" class="btn ghost tiny" disabled={fleet.disksChecking || !hosts.hosts.length} onclick={() => fleet.checkDisks()}>{fleet.disksChecking ? "Reading…" : "Check all"}</button><button type="button" class="link" onclick={() => goFleet("disks")}>Fleet → Disks</button></div>
{:else if kind === "backups"}
  {#if backups.loaded}
    {#if bkBad.length}
      <ul class="list">
        {#each bkBad as j (j.id)}<li><button type="button" class="row" onclick={() => goFleet("backups")}><span class="txt">{j.name}</span><span class="small bad">{jobState(backups.status[j.id]).toUpperCase()}</span></button></li>{/each}
      </ul>
    {:else}
      <p class="calm"><span class="ok">●</span> {backups.jobs.length ? `${backups.jobs.length} job${backups.jobs.length === 1 ? "" : "s"}, none failed or stale.` : "No backup jobs on the board."}</p>
    {/if}
  {:else}
    <p class="muted small">Not read yet.</p>
  {/if}
  <div class="foot"><button type="button" class="btn ghost tiny" disabled={backups.checking} onclick={() => backups.load()}>{backups.checking ? "Reading…" : "Check"}</button><button type="button" class="link" onclick={() => goFleet("backups")}>Fleet → Backups</button></div>
{:else if kind === "tunnels"}
  {#if tunnels.tunnels.length}
    <ul class="list">
      {#each tunnels.tunnels as t (t.id)}
        {@const st = tunnels.states[t.id] ?? t.state}
        <li>
          <button type="button" class="row" onclick={() => ui.go("tunnels")}>
            <span class="txt">{t.name || t.spec}</span>
            <span class="small {st.kind === 'running' ? 'ok' : st.kind === 'failed' || st.kind === 'retrying' ? 'warn' : 'muted'}">{st.kind.toUpperCase()}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">No tunnels. <button type="button" class="link" onclick={() => ui.go("tunnels")}>Add one</button></p>
  {/if}
{:else if kind === "notes"}
  {#if pinned.length}
    <ul class="list">
      {#each pinned as n (n.id)}
        <li><button type="button" class="row" onclick={() => { notes.selected = n.id; ui.go("notes"); }}><span class="txt">{n.title}</span></button></li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">{notes.loaded ? "Pin a note to keep it here." : "Loading…"} <button type="button" class="link" onclick={() => ui.go("notes")}>Notes</button></p>
  {/if}
{:else if kind === "desktops"}
  {#if desktops.tabs.length || desktops.rdp.length}
    <ul class="list">
      {#each desktops.tabs as t (t.key)}
        <li><button type="button" class="row" onclick={() => { desktops.active = t.key; ui.go("desktops"); }}><span class="txt">{desktopLabel(t)}</span><span class="small {t.status === 'connected' ? 'ok' : 'muted'}">{t.status.toUpperCase()}</span></button></li>
      {/each}
      {#each desktops.rdp as r (r.id)}
        <li><button type="button" class="row" onclick={() => ui.go("desktops")}><span class="txt">{r.guest}</span><span class="small muted">RDP :{r.localPort}</span></button></li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">No VM screens open. Open one from System → VMS.</p>
  {/if}
{:else if kind === "status"}
  <dl class="kv">
    <dt>BUILD</dt>
    <dd>{info ? `v${info.version}` : "…"}</dd>
    <dt>PLATFORM</dt>
    <dd>{info ? `${info.os} / ${info.arch}` : "…"}</dd>
    <dt>DATABASE</dt>
    <dd>{info ? `schema v${info.schemaVersion}` : "…"}</dd>
    <dt>KEYCHAIN</dt>
    <dd class:ok={info?.keychain.available} class:warn={info && !info.keychain.available}>
      {#if info}{info.keychain.available ? "available" : (info.keychain.detail ?? "unavailable")}{:else}…{/if}
    </dd>
    <dt>DATA DIR</dt>
    <dd class="path">{info?.dataDir ?? "…"}</dd>
  </dl>
{:else}
  <p class="muted small">This widget ({widgetInfo(kind)?.title ?? kind}) isn't available in this version.</p>
{/if}

<style>
  .hostgrid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 8px;
  }
  .empty p {
    margin: 0 0 8px;
    line-height: 1.6;
  }
  .empty .label {
    color: var(--text-0);
    font-size: var(--fs-lg);
  }
  .actions,
  .foot {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .foot {
    margin-top: auto;
    padding-top: 6px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .row {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: baseline;
    background: none;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
    padding: 4px 0;
    color: var(--text-0);
    text-align: left;
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-sm);
  }
  .row:hover .txt {
    color: var(--cyan);
  }
  .txt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sev {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    width: 5ch;
    flex: none;
  }
  .sev.critical,
  .bad {
    color: var(--danger);
  }
  .sev.warn,
  .warn {
    color: var(--warn);
  }
  .sev.info {
    color: var(--cyan);
  }
  .ok {
    color: var(--ok);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .big {
    font-size: var(--fs-lg);
    margin: 0 0 4px;
    color: var(--text-0);
  }
  .calm {
    margin: 0;
    color: var(--text-1);
  }
  p {
    margin: 0 0 4px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-xs);
  }
  .kv {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 12px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .kv dt {
    color: var(--text-1);
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
  }
  .kv dd {
    margin: 0;
    color: var(--text-0);
  }
  .path {
    overflow-wrap: anywhere;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  code {
    color: var(--cyan);
  }
</style>
