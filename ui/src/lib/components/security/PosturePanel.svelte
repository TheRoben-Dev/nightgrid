<script lang="ts">
  /**
   * Security → POSTURE (1.1, D-168): the security audit for every host, one
   * score each with its trend; open a host for its checks, worst first, each
   * with the fix and the Library guide whose quest verifies it.
   */
  import { onMount } from "svelte";
  import type { PostureHost } from "../../api";
  import { cost, delta, fixesFor, gradeTone, ordered, sparkline, statusMark, summary, worstFirst } from "../../posture";
  import { ago } from "../../notices";
  import { hosts } from "../../stores/hosts.svelte";
  import { library } from "../../stores/library.svelte";
  import { security } from "../../stores/security.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import Toggle from "../Toggle.svelte";
  import Markdown from "../Markdown.svelte";

  onMount(() => {
    if (!security.loaded) void security.load();
  });

  const byHost = $derived(new Map(security.posture.map((p) => [p.latest.hostId, p])));
  const rows = $derived.by(() => {
    const scored = worstFirst(security.posture).filter((p) => hosts.byId(p.latest.hostId));
    const never = hosts.hosts.filter((h) => !byHost.has(h.id));
    return { scored, never };
  });
  const sum = $derived(summary(rows.scored));
  const openHost = $derived(security.open === null ? null : (byHost.get(security.open) ?? null));
  const openChecks = $derived(openHost ? ordered(openHost.latest.checks) : []);
  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client").map((h) => h.id));

  const name = (id: number) => hosts.byId(id)?.name ?? `host ${id}`;
  function readGuide(article: string) {
    library.open(article);
  }
  function inSystem(id: number) {
    system.select(id);
    void system.openTab("security");
    ui.go("system");
  }
  const setDaily = (v: boolean) => settings.update({ security: { ...settings.value.security, postureDaily: v } });
  const busy = (id: number) => security.checking.includes(id);
  const tone = (p: PostureHost) => gradeTone(p.latest.grade);
</script>

<div class="posture" data-guide="security-posture">
  <div class="row">
    <button type="button" class="btn primary" disabled={!servers.length || security.checking.length > 0} onclick={() => security.check(servers)} data-guide="posture-check-all"
      >{security.checking.length ? `Checking ${security.checking.length}…` : "Check all"}</button
    >
    <span class="muted small">Read only, no sudo. Client hosts are left out of <strong>Check all</strong>.</span>
    <span class="sp"></span>
    <Toggle label="Check connected hosts once a day" checked={settings.value.security.postureDaily} onchange={setDaily} />
  </div>
  {#if security.error}<p class="error-text small">{security.error}</p>{/if}

  <div class="cards">
    <div class="card"><span class="label">FLEET AVERAGE</span><span class="big data {sum.average == null ? '' : sum.average >= 80 ? 'ok' : sum.average >= 60 ? 'warn' : 'bad'}">{sum.average ?? "--"}</span><span class="muted small">{sum.hosts} host{sum.hosts === 1 ? "" : "s"} scored</span></div>
    <div class="card"><span class="label">BELOW 75</span><span class="big data {sum.below ? 'warn' : ''}">{sum.below}</span><span class="muted small">hosts worth a look</span></div>
    <div class="card"><span class="label">FAILED CHECKS</span><span class="big data {sum.failing ? 'bad' : ''}">{sum.failing}</span><span class="muted small">across all hosts</span></div>
    <div class="card">
      <span class="label">FIX THEM</span>
      <button type="button" class="link" onclick={() => readGuide("guides/security-posture")}>Raise your fleet's security score ▤</button>
      <span class="muted small">a Library quest that verifies each fix</span>
    </div>
  </div>

  <div class="split">
    <div class="list">
      {#each rows.scored as p (p.latest.hostId)}
        <button type="button" class="hrow" class:on={security.open === p.latest.hostId} onclick={() => security.select(p.latest.hostId)}>
          <span class="grade {tone(p)}">{p.latest.grade ?? "?"}</span>
          <span class="hn">
            <span class="name">{name(p.latest.hostId)}</span>
            <span class="muted small">{p.latest.error ? p.latest.error : `${fixesFor(p.latest).length} to fix · ${p.latest.openPorts} port${p.latest.openPorts === 1 ? "" : "s"} on every address`} · {ago(p.latest.at)}</span>
          </span>
          {#if p.trend.length > 1}<svg class="spark" viewBox="0 0 80 18" aria-hidden="true"><polyline points={sparkline(p.trend)} /></svg>{/if}
          <span class="score data">{p.latest.score ?? "--"}<span class="d {delta(p).startsWith('−') ? 'bad' : 'ok'}">{delta(p)}</span></span>
        </button>
      {/each}
      {#each rows.never as h (h.id)}
        <div class="hrow plain">
          <span class="grade muted">·</span>
          <span class="hn"><span class="name">{h.name}</span><span class="muted small">not checked yet</span></span>
          <button type="button" class="btn tiny ghost" disabled={busy(h.id)} onclick={() => security.check([h.id])}>{busy(h.id) ? "Checking…" : "Check"}</button>
        </div>
      {/each}
      {#if !hosts.hosts.length}<p class="muted small pad">Add a host first.</p>{/if}
    </div>

    <div class="detail">
      {#if openHost}
        {@const id = openHost.latest.hostId}
        <div class="row">
          <span class="grade big {tone(openHost)}">{openHost.latest.grade ?? "?"}</span>
          <span class="hn"><span class="name">{name(id)}</span><span class="muted small">score {openHost.latest.score ?? "--"} of 100{openHost.previous != null ? `, ${openHost.previous} before` : ""}</span></span>
          <span class="sp"></span>
          <button type="button" class="btn tiny" disabled={busy(id)} onclick={() => security.check([id])}>{busy(id) ? "Checking…" : "Check again"}</button>
          <button type="button" class="btn tiny ghost" onclick={() => inSystem(id)} title="System → security: ports and the same checks for this host">Ports…</button>
        </div>
        {#if openHost.latest.error}<p class="error-text small">{openHost.latest.error}</p>{/if}
        {#each openChecks as c (c.id)}
          <div class="check {c.status}">
            <span class="mark" aria-label={c.status}>{statusMark[c.status]}</span>
            <div class="ct">
              <div class="row tight">
                <strong>{c.title}</strong>
                {#if cost(c)}<span class="muted small">−{cost(c)}</span>{/if}
                <span class="sp"></span>
                <button type="button" class="link small" onclick={() => readGuide(c.article)}>Guide ▤</button>
              </div>
              <span class="muted small">{c.detail}</span>
              {#if c.fix}<div class="fix small"><Markdown src={c.fix} /></div>{/if}
            </div>
          </div>
        {/each}
        {#if security.history.length > 1}
          <h3 class="hd">HISTORY</h3>
          {#each security.history.slice(0, 10) as r (r.at)}
            <div class="hist muted small"><span>{new Date(r.at * 1000).toLocaleString()}</span><span class="data">{r.error ? "not read" : `${r.score} ${r.grade}`}</span></div>
          {/each}
        {/if}
      {:else}
        <p class="muted pad">Pick a host to see its checks. Each check that fails costs its weight, a warning half of it.</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .posture {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .row.tight {
    gap: 6px;
  }
  .sp {
    flex: 1;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 8px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 1px solid var(--line);
    padding: 8px 10px;
    align-items: flex-start;
  }
  .big {
    font-size: var(--fs-lg, 20px);
    color: var(--text-0);
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn);
  }
  .bad {
    color: var(--danger);
  }
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(260px, 2fr) 3fr;
    gap: 10px;
  }
  .list,
  .detail {
    overflow: auto;
    min-height: 0;
    border: 1px solid var(--line);
  }
  .detail {
    padding: 8px 10px;
  }
  .hrow {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 7px 10px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .hrow.plain {
    cursor: default;
  }
  .hrow:not(.plain):hover,
  .hrow:focus-visible,
  .hrow.on {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .hrow.on {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .grade {
    font-family: var(--font-display);
    font-size: 18px;
    width: 26px;
    text-align: center;
    border: 1px solid currentColor;
    flex: none;
  }
  .grade.big {
    font-size: 26px;
    width: 38px;
  }
  .grade.muted {
    color: var(--text-2);
  }
  .hn {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spark {
    width: 80px;
    height: 18px;
    flex: none;
  }
  .spark polyline {
    fill: none;
    stroke: var(--cyan);
    stroke-width: 1.2;
  }
  .score {
    width: 56px;
    text-align: right;
    color: var(--text-0);
  }
  .d {
    font-size: var(--fs-xs);
    margin-left: 3px;
  }
  .check {
    display: flex;
    gap: 10px;
    padding: 7px 0;
    border-bottom: 1px solid var(--line);
  }
  .mark {
    width: 18px;
    flex: none;
    text-align: center;
    font-weight: 700;
  }
  .check.pass .mark {
    color: var(--ok);
  }
  .check.warn .mark {
    color: var(--warn);
  }
  .check.fail .mark {
    color: var(--danger);
  }
  .check.unknown .mark {
    color: var(--text-2);
  }
  .ct {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }
  .fix {
    border-left: 2px solid var(--line);
    padding-left: 8px;
    color: var(--text-1);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 14px 0 4px;
  }
  .hist {
    display: flex;
    justify-content: space-between;
    padding: 2px 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  @media (max-width: 900px) {
    .split {
      grid-template-columns: 1fr;
    }
  }
</style>
