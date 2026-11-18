<script lang="ts">
  /**
   * Fleet → CATALOG (1.1, D-170): what runs where. Containers on every server
   * host with version, ports, URLs, data folders and whether a backup job
   * names them. Read when asked; nothing is stored.
   */
  import { catalogMatch, catalogSummary, coverageLabel, coverageTone } from "../../fleet11";
  import { hosts } from "../../stores/hosts.svelte";
  import { catalog } from "../../stores/fleet11.svelte";

  let q = $state("");
  let onlyLoose = $state(false);
  let open = $state<string | null>(null);
  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client").map((h) => h.id));
  const sum = $derived(catalogSummary(catalog.hosts));
  const shown = $derived(
    catalog.hosts.map((h) => ({ ...h, entries: h.entries.filter((e) => catalogMatch(e, q) && (!onlyLoose || e.coverage === "none" || e.coverage === "partial")) })),
  );
</script>

<div class="cat" data-guide="fleet-catalog">
  <div class="row">
    <button type="button" class="btn primary" disabled={catalog.building || !servers.length} onclick={() => catalog.build(servers)} data-guide="catalog-build"
      >{catalog.building ? "Reading hosts…" : catalog.builtAt ? "Read again" : "Build catalog"}</button
    >
    <button type="button" class="btn ghost" disabled={!catalog.hosts.length} onclick={() => catalog.export()}>Export Markdown</button>
    <input class="input" placeholder="Filter: name, image, URL, folder" bind:value={q} />
    <label class="muted small"><input type="checkbox" bind:checked={onlyLoose} /> only without a full backup</label>
    <span class="sp"></span>
    {#if catalog.builtAt}<span class="muted small">read {new Date(catalog.builtAt).toLocaleTimeString()}</span>{/if}
  </div>
  {#if catalog.error}<p class="error-text small">{catalog.error}</p>{/if}
  {#if catalog.saved}<p class="muted small">Saved to {catalog.saved}</p>{/if}

  {#if !catalog.hosts.length}
    <p class="muted pad">
      <strong>Build catalog</strong> reads every server host's containers (image, version, ports, data folders), matches them with your service checks and reverse-proxy routes, and checks which data folders a backup job's command line names. Client hosts are left out. Nothing on the hosts changes.
    </p>
  {:else}
    <div class="cards">
      <div class="card"><span class="label">CONTAINERS</span><span class="big data">{sum.containers}</span><span class="muted small">{sum.running} running</span></div>
      <div class="card"><span class="label">WITHOUT A FULL BACKUP</span><span class="big data {sum.unprotected ? 'warn' : ''}">{sum.unprotected}</span><span class="muted small">data folders no backup job names</span></div>
      <div class="card"><span class="label">URLS</span><span class="big data">{sum.urls}</span><span class="muted small">from service checks and proxy routes</span></div>
    </div>
    <div class="list">
      {#each shown as h (h.hostId)}
        <h3 class="hd">{h.name.toUpperCase()} <span class="muted">{h.error ? "" : `${h.entries.length} container${h.entries.length === 1 ? "" : "s"}`}</span></h3>
        {#if h.error}<p class="error-text small pad">{h.error}</p>{/if}
        {#each h.entries as e (e.name)}
          {@const key = `${h.hostId}/${e.name}`}
          <button type="button" class="crow" class:on={open === key} onclick={() => (open = open === key ? null : key)}>
            <span class="dot {e.state === 'running' ? 'ok' : 'muted'}">●</span>
            <span class="cn"><span class="name">{e.name}</span><span class="muted small">{e.image}{e.project ? ` · ${e.project}` : ""}</span></span>
            <span class="small data">{e.version ?? ""}</span>
            <span class="small ports">{e.ports.join(", ")}</span>
            <span class="cov {coverageTone[e.coverage]}">{coverageLabel[e.coverage]}</span>
          </button>
          {#if open === key}
            <div class="detail small">
              {#if e.urls.length}<div><span class="label">URLS</span> {#each e.urls as u (u)}<span class="url data">{u}</span>{/each}</div>{/if}
              {#if e.health}<div><span class="label">HEALTH</span> {e.health}</div>{/if}
              {#if e.mounts.length}
                <span class="label">DATA FOLDERS</span>
                {#each e.mounts as m (m.source + m.destination)}
                  <div class="mount"><span class="data">{m.source}</span><span class="muted">→ {m.destination}</span><span class={m.backedUpBy ? "ok" : "bad"}>{m.backedUpBy ? `in ${m.backedUpBy}` : "no backup job names it"}</span></div>
                {/each}
              {:else}
                <p class="muted">No data folders: nothing to back up beyond its compose file.</p>
              {/if}
            </div>
          {/if}
        {/each}
        {#if !h.error && h.jobsRead === 0 && h.entries.some((e) => e.mounts.length)}<p class="muted small pad">No backup job on this host has a systemd unit Nightgrid could read; add one in Fleet → BACKUPS.</p>{/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .cat {
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
  .row .input {
    min-width: 220px;
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
  }
  .big {
    font-size: var(--fs-lg, 20px);
    color: var(--text-0);
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
    margin: 0;
    padding: 8px 10px 4px;
    border-bottom: 1px solid var(--line);
    position: sticky;
    top: 0;
    background: var(--bg-1, var(--bg-0));
  }
  .crow {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: grid;
    grid-template-columns: 14px minmax(160px, 2fr) minmax(60px, 0.6fr) minmax(90px, 1fr) 96px;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .crow:hover,
  .crow:focus-visible,
  .crow.on {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .cn {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    color: var(--text-0);
  }
  .cn .small,
  .ports {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cov {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    text-align: right;
  }
  .detail {
    padding: 6px 10px 10px 34px;
    border-bottom: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .url {
    margin-right: 10px;
    color: var(--cyan);
  }
  .mount {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr) auto;
    gap: 8px;
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
  .muted {
    color: var(--text-2);
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
</style>
