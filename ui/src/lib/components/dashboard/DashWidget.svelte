<script module lang="ts">
  /** When each widget last fetched on its own (unix ms); shared by every copy of a widget. */
  const autoAt: Record<"updates" | "disks" | "backups", number | null> = { updates: null, disks: null, backups: null };
</script>

<script lang="ts">
  /**
   * One dashboard widget's content (M10o; more detail in 0.10.52). UPDATES,
   * DISKS and BACKUPS fetch on their own while shown (servers that are
   * online only, never asking for a password or sudo), on a slow schedule.
   */
  import { untrack } from "svelte";
  import type { AppInfo } from "../../api";
  import { diskRows, updateRows } from "../../fleet";
  import { cmdId, widgetInfo } from "../../dashboard";
  import { agoShort, AUTO_AGE, autoHosts, backupLines, diskLines, due, recentIncidents, serviceRows, updateLines } from "../../overview";
  import CmdTile from "./CmdTile.svelte";
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
  import { metrics } from "../../stores/metrics.svelte";
  import { formatSize } from "../../files/util";

  let { kind, info }: { kind: string; info: AppInfo | null } = $props();

  $effect(() => {
    if (kind === "notes" && !notes.loaded) void notes.load();
    // The job list is local; reading the hosts waits for the schedule below.
    if (kind === "backups" && !backups.loaded) void backups.load();
  });

  type AutoKind = "updates" | "disks" | "backups";
  const isAuto = (k: string): k is AutoKind => k === "updates" || k === "disks" || k === "backups";

  /** Fetches when the data is older than the widget's schedule (looked at each minute). */
  function autoFetch(k: AutoKind) {
    if (typeof document !== "undefined" && document.hidden) return;
    const now = Date.now();
    const ids = autoHosts(hosts.hosts, hosts.pings);
    if (!ids.length) return;
    const last = (full: number | null) => Math.max(autoAt[k] ?? 0, full ?? 0) || null;
    if (k === "updates" && !fleet.checking && due(last(fleet.checkedAt), AUTO_AGE.updates, now)) {
      autoAt.updates = now;
      void fleet.checkAll(ids);
    } else if (k === "disks" && !fleet.disksChecking && due(last(fleet.disksAt), AUTO_AGE.disks, now)) {
      autoAt.disks = now;
      void fleet.checkDisks(ids);
    } else if (k === "backups" && !backups.checking && due(last(backups.checkedAt), AUTO_AGE.backups, now)) {
      autoAt.backups = now;
      void (backups.loaded ? Promise.resolve() : backups.load()).then(() => (backups.jobs.length ? backups.checkAll(ids) : undefined));
    }
  }

  $effect(() => {
    const k = kind;
    if (!isAuto(k)) return;
    // A few seconds' grace so the first host pings are in (offline hosts are skipped).
    const first = setTimeout(() => untrack(() => autoFetch(k)), 4000);
    const every = setInterval(() => untrack(() => autoFetch(k)), 60_000);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  });

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const nowS = $derived(Math.floor(now / 1000));

  const hostName = (id: number | null) => (id == null ? "" : (hosts.hosts.find((h) => h.id === id)?.name ?? ""));
  const sevRank = (s: string) => (s === "critical" ? 0 : 1);
  const activeAlerts = $derived([...alerts.roots].sort((a, b) => sevRank(a.severity) - sevRank(b.severity) || b.since - a.since));
  const incidents = $derived(activeAlerts.length ? [] : recentIncidents(alerts.history, nowS));
  const svcRows = $derived(serviceRows(services.services, services.status, healthOf));
  const updRows = $derived(updateRows(hosts.hosts, fleet.updates));
  const updLines = $derived(updateLines(updRows));
  const updPkgs = $derived(updRows.reduce((n, r) => n + r.count, 0));
  const updNewest = $derived(Math.max(0, ...Object.values(fleet.updates).map((u) => u.checkedAt ?? 0)) || null);
  const dLines = $derived(diskLines(diskRows(hosts.hosts, fleet.disks, settings.value.alerts.forecastDays)));
  const diskNewest = $derived(Math.max(0, ...Object.values(fleet.disks).map((d) => d.checkedAt ?? 0)) || null);
  const bkLines = $derived(backupLines(backups.jobs, backups.status, jobState, nowS));
  const bkProblems = $derived(bkLines.filter((l) => l.state === "failed" || l.state === "error" || l.state === "stale").length);
  const bkNewest = $derived(backups.readAt);
  const pinned = $derived(notes.notes.filter((n) => n.pinned));
  const gpuRows = $derived(
    hosts.hosts.flatMap((h) => (metrics.byHost[h.id]?.sample?.gpus ?? []).map((g, i) => ({ key: `${h.id}-${i}`, hostId: h.id, host: h.name, g }))),
  );
  const goFleet = (tab: "updates" | "disks" | "backups") => {
    if (tab === "backups") fleet.openBackups();
    else fleet.tab = tab;
    ui.go("fleet");
  };
  const ageMs = (at: number | null) => (!at ? "" : nowS - at / 1000 < 60 ? "just now" : `${agoShort(Math.floor(at / 1000), nowS)} ago`);
  const toneClass = (t: string) => (t === "bad" ? "bad" : t === "warn" ? "warn" : t === "ok" ? "ok" : "muted");
  const DOT: Record<string, string> = { up: "ok", down: "bad", warn: "warn", pending: "muted", off: "muted" };
  const JOB_CLASS: Record<string, string> = { ok: "ok", failed: "bad", error: "bad", stale: "warn", running: "cyan", unknown: "muted" };
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
            <span class="muted small num">{agoShort(a.since, nowS)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="calm"><span class="ok">●</span> No active alerts.</p>
    {#if incidents.length}
      <p class="hd">RECENT INCIDENTS</p>
      <ul class="list">
        {#each incidents as a (a.id)}
          <li>
            <button type="button" class="row" title="{a.detail ? `${a.detail} · ` : ''}lasted {agoShort(a.since, a.resolved ?? a.since)}" onclick={() => (alerts.open = true)}>
              <span class="muted small num">{agoShort(a.resolved ?? a.since, nowS)}</span>
              <span class="sev {a.severity} dim">{a.severity === "critical" ? "CRIT" : "WARN"}</span>
              <span class="txt">{a.title}</span>
              <span class="muted small">{hostName(a.hostId)}</span>
              <span class="muted small num" title="how long it lasted">{agoShort(a.since, a.resolved ?? a.since)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted small">Nothing resolved in the last 7 days.</p>
    {/if}
  {/if}
{:else if kind === "services"}
  {@const s = services.summary}
  {#if s.total}
    <p class="sum"><span class="ok">{s.up}</span><span class="muted">/{s.total} up</span>{#if s.down}<span class="bad">{` · ${s.down} down`}</span>{/if}{#if s.warn}<span class="warn">{` · ${s.warn} warn`}</span>{/if}</p>
    <ul class="list">
      {#each svcRows as r (r.svc.id)}
        <li>
          <button type="button" class="row" title={r.detail || r.svc.url} onclick={() => ui.go("services")}>
            <span class="dot {DOT[r.health]}" aria-hidden="true">●</span>
            <span class="txt">{r.svc.name}</span>
            <span class="small num {r.health === 'down' ? 'bad' : r.health === 'warn' ? 'warn' : 'muted'}">{r.note}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">No services on the board yet. <button type="button" class="link" onclick={() => ui.go("services")}>Add some</button></p>
  {/if}
{:else if kind === "updates"}
  {#if updLines.length}
    <p class="sum">{`${updPkgs} `}<span class="muted">pending</span>{#if updNewest}<span class="muted small">{` · checked ${ageMs(updNewest)}`}</span>{/if}</p>
    <ul class="list">
      {#each updLines as l (l.host.id)}
        <li>
          <button type="button" class="row" title={l.detail} onclick={() => goFleet("updates")}>
            <span class="dot {toneClass(l.tone)}" aria-hidden="true">●</span>
            <span class="txt">{l.host.name}</span>
            <span class="small num {l.tone === 'ok' ? 'muted' : toneClass(l.tone)}">{l.text}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">{fleet.checking ? "Checking…" : "Not checked yet. Servers are checked every few hours while this is shown."}</p>
  {/if}
  <div class="foot"><button type="button" class="btn ghost tiny" disabled={fleet.checking || !hosts.hosts.length} onclick={() => fleet.checkAll()}>{fleet.checking ? "Checking…" : "Check all"}</button><button type="button" class="link" onclick={() => goFleet("updates")}>Fleet → Updates</button></div>
{:else if kind === "disks"}
  {#if dLines.length}
    <p class="sum muted small">{fleet.disksChecking ? "reading…" : `read ${ageMs(diskNewest)}`}</p>
    <ul class="list">
      {#each dLines as l (l.host.id)}
        <li>
          <button type="button" class="row" title={l.detail} onclick={() => goFleet("disks")}>
            <span class="dot {toneClass(l.tone)}" aria-hidden="true">●</span>
            <span class="host">{l.host.name}</span>
            <span class="txt small {l.tone === 'ok' ? 'muted' : toneClass(l.tone)}">{l.text}</span>
            {#if l.pct != null}
              <span class="meter" title="fullest mount: {l.pct}% used"><span class:warn={l.pct >= 85} class:bad={l.pct >= 95} style:width="{l.pct}%"></span></span>
              <span class="small num muted pct">{l.pct}%</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">{fleet.disksChecking ? "Reading…" : "Not read yet. Servers are read hourly while this is shown."}</p>
  {/if}
  <div class="foot"><button type="button" class="btn ghost tiny" disabled={fleet.disksChecking || !hosts.hosts.length} onclick={() => fleet.checkDisks()}>{fleet.disksChecking ? "Reading…" : "Check all"}</button><button type="button" class="link" onclick={() => goFleet("disks")}>Fleet → Disks</button></div>
{:else if kind === "backups"}
  {#if bkLines.length}
    <p class="sum">
      {#if bkProblems}<span class="bad">{bkProblems} need a look</span>{:else}<span class="muted">{bkLines.length} job{bkLines.length === 1 ? "" : "s"}</span>{/if}{#if backups.checking}<span class="muted small">&nbsp;· reading…</span>{:else if bkNewest}<span class="muted small">{` · read ${ageMs(bkNewest)}`}</span>{/if}
    </p>
    <ul class="list">
      {#each bkLines as l (l.job.id)}
        <li>
          <button type="button" class="row" title={l.detail} onclick={() => goFleet("backups")}>
            <span class="dot {JOB_CLASS[l.state]}" aria-hidden="true">●</span>
            <span class="txt">{l.job.name} <span class="muted small">{hostName(l.job.hostId)} · {l.job.tool}</span></span>
            <span class="small num muted" title="last good run">{l.age}</span>
            <span class="small {JOB_CLASS[l.state]} word">{l.state.toUpperCase()}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">{backups.loaded ? "No backup jobs on the board." : "Reading…"}</p>
  {/if}
  <div class="foot"><button type="button" class="btn ghost tiny" disabled={backups.checking} onclick={() => void (backups.loaded ? backups.checkAll() : backups.load().then(() => backups.checkAll()))}>{backups.checking ? "Reading…" : "Check all"}</button><button type="button" class="link" onclick={() => goFleet("backups")}>Fleet → Backups</button></div>
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
    <p class="muted small">{notes.loaded ? "Pin a note to keep it here." : "Loading…"} <button type="button" class="link" onclick={() => ui.go("notes")}>Knowledge</button></p>
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
{:else if kind === "gpus"}
  {#if gpuRows.length}
    <ul class="list">
      {#each gpuRows as r (r.key)}
        <li>
          <button type="button" class="row" title={r.g.name} onclick={() => (metrics.detailId = r.hostId)}>
            <span class="txt">{r.host} <span class="muted small">{r.g.name}</span></span>
            <span class="small data">
              <span class:warn={(r.g.util ?? 0) >= 90}>{r.g.util == null ? "--" : `${Math.round(r.g.util)}%`}</span>
              {#if r.g.memTotal}<span class="muted"> · {formatSize(r.g.memUsed ?? 0)}/{formatSize(r.g.memTotal)}</span>{/if}
              {#if r.g.temp != null}<span class:warn={r.g.temp >= 80}> · {Math.round(r.g.temp)}°C</span>{/if}
              {#if r.g.power != null}<span class="muted"> · {Math.round(r.g.power)} W</span>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">No GPUs seen. NVIDIA cards show up when <code>nvidia-smi</code> works on the host; AMD cards through the amdgpu driver.</p>
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
{:else if cmdId(kind) != null}
  <CmdTile id={cmdId(kind) ?? 0} />
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
  .sev.dim {
    opacity: 0.7;
  }
  .hd {
    margin: 8px 0 2px;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .sum {
    margin: 0 0 4px;
    color: var(--text-0);
  }
  .dot {
    flex: none;
    font-size: 0.7em;
    align-self: center;
  }
  .num {
    flex: none;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .pct {
    width: 4ch;
    text-align: right;
  }
  .host {
    flex: none;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .word {
    flex: none;
    width: 8ch;
    text-align: right;
    letter-spacing: 0.06em;
  }
  .meter {
    flex: none;
    width: 44px;
    height: 4px;
    align-self: center;
    background: color-mix(in srgb, var(--line) 70%, transparent);
  }
  .meter > span {
    display: block;
    height: 100%;
    background: var(--cyan);
  }
  .meter > span.warn {
    background: var(--warn);
  }
  .meter > span.bad {
    background: var(--danger);
  }
  .cyan {
    color: var(--cyan);
  }
  .muted {
    color: var(--text-1);
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
