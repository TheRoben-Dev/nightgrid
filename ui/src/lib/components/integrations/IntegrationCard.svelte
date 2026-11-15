<script lang="ts">
  import type { IntegrationView } from "../../api";
  import { openExternal } from "../../clipboard";
  import { integrations, KIND_LABEL, summary, toneOf } from "../../stores/integrations.svelte";

  interface Props {
    view: IntegrationView;
  }
  let { view }: Props = $props();

  const i = $derived(view.integration);
  const e = $derived(integrations.entries[i.id]);
  const s = $derived(e?.status ?? null);
  const tone = $derived(toneOf(e));
  const needsSecret = $derived((i.kind === "plex" || i.kind === "amp") && !view.hasSecret);
</script>

<article class="card {tone}">
  <header>
    <span class="kind">{KIND_LABEL[i.kind]}</span>
    <button type="button" class="name" title="Open {i.baseUrl}" onclick={() => openExternal(i.baseUrl)}>{i.name}</button>
    <span class="spacer"></span>
    <button type="button" class="icon" title="Refresh" aria-label="Refresh {i.name}" disabled={e?.loading} onclick={() => integrations.refresh(i.id)}>↻</button>
    <button type="button" class="icon" title="Edit" aria-label="Edit {i.name}" onclick={() => (integrations.editing = i.id)}>…</button>
  </header>

  {#if needsSecret}
    <p class="muted small">No {i.kind === "plex" ? "token" : "password"} saved. <button type="button" class="link" onclick={() => (integrations.editing = i.id)}>Add it</button></p>
  {:else if e?.error}
    <p class="err small" title={e.error}>{e.error}</p>
  {:else if !s}
    <p class="muted small">{e?.loading ? "Reading…" : ""}</p>
  {:else}
    <p class="sum">{summary(s)}</p>
    {#if s.kind === "plex"}
      {#if s.streams.length}
        <ul class="rows">
          {#each s.streams as st, n (n)}
            <li>
              <span class="dot {st.state}" aria-hidden="true"></span>
              <span class="main" title={st.title}>{st.title}</span>
              <span class="meta">{st.user} · {st.player}{st.quality ? ` · ${st.quality}` : ""}{st.transcode ? " · TRANSCODE" : ""}</span>
              {#if st.progress != null}<span class="bar" aria-hidden="true"><i style:width="{st.progress}%"></i></span>{/if}
            </li>
          {/each}
        </ul>
      {/if}
      <p class="foot muted">{s.server}{s.version ? ` · v${s.version}` : ""}</p>
    {:else if s.kind === "amp"}
      <ul class="rows">
        {#each s.instances as inst (inst.name)}
          <li class="amp">
            <span class="dot {inst.running && inst.state === 'ready' ? 'playing' : inst.running ? 'buffering' : 'off'}" aria-hidden="true"></span>
            <span class="main" title="{inst.friendlyName} ({inst.name})">{inst.friendlyName || inst.name}</span>
            <span class="meta">
              {inst.module} · {inst.state}{inst.users != null ? ` · ${inst.users}${inst.maxUsers ? `/${inst.maxUsers}` : ""} players` : ""}
            </span>
            <span class="acts">
              {#if inst.running}
                <button type="button" class="btn ghost tiny" onclick={() => integrations.request({ id: i.id, instance: inst.name, friendly: inst.friendlyName, action: "restart" })}>Restart</button>
                <button type="button" class="btn ghost tiny" onclick={() => integrations.request({ id: i.id, instance: inst.name, friendly: inst.friendlyName, action: "stop" })}>Stop</button>
              {:else}
                <button type="button" class="btn ghost tiny" onclick={() => integrations.request({ id: i.id, instance: inst.name, friendly: inst.friendlyName, action: "start" })}>Start</button>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {:else if s.kind === "kuma"}
      <ul class="rows kuma">
        {#each s.monitors as m (m.id)}
          <li title={m.message}>
            <span class="dot k-{m.state}" aria-hidden="true"></span>
            <span class="main">{m.name}</span>
            <span class="meta">{m.group}{m.pingMs != null ? ` · ${m.pingMs} ms` : ""}{m.uptime24h != null ? ` · ${m.uptime24h.toFixed(1)}%` : ""}</span>
          </li>
        {/each}
      </ul>
    {:else if s.kind === "portainer"}
      <p class="muted small">
        {i.hostId != null ? "Containers on the linked host get an Open in Portainer link in Docker." : "Link a host in its settings to get container links in Docker."}
      </p>
    {/if}
  {/if}
</article>

<style>
  .card {
    --tone: var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-left: 2px solid var(--tone);
    min-width: 0;
  }
  .card.ok {
    --tone: var(--ok);
  }
  .card.warn {
    --tone: var(--warn);
  }
  .card.bad {
    --tone: var(--danger);
  }
  header {
    display: flex;
    gap: 8px;
    align-items: center;
    min-width: 0;
  }
  .kind {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--mint);
  }
  .name {
    border: 0;
    background: transparent;
    padding: 0;
    color: var(--text-0);
    font: inherit;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name:hover {
    color: var(--accent);
  }
  .spacer {
    flex: 1;
  }
  .icon {
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    padding: 0 4px;
  }
  .icon:hover {
    color: var(--text-0);
  }
  p {
    margin: 0;
  }
  .small,
  .foot {
    font-size: var(--fs-xs);
  }
  .err {
    color: var(--danger);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sum {
    color: var(--cyan);
    font-size: var(--fs-sm);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    max-height: 220px;
    overflow: auto;
  }
  .rows li {
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr);
    column-gap: 8px;
    align-items: center;
    font-size: var(--fs-sm);
  }
  .rows li.amp {
    grid-template-columns: 8px minmax(0, 1fr) auto;
  }
  .main {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    grid-column: 2;
    font-size: var(--fs-xs);
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .acts {
    grid-column: 3;
    grid-row: 1 / span 2;
    display: flex;
    gap: 4px;
  }
  .kuma li {
    grid-template-columns: 8px minmax(0, 1fr) auto;
  }
  .kuma .meta {
    grid-column: 3;
  }
  .bar {
    grid-column: 2;
    height: 2px;
    background: var(--line);
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--cyan);
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--text-1);
  }
  .dot.playing,
  .dot.k-up {
    background: var(--ok);
  }
  .dot.paused,
  .dot.buffering,
  .dot.k-pending,
  .dot.k-maintenance {
    background: var(--warn);
  }
  .dot.k-down {
    background: var(--danger);
  }
  .dot.off {
    background: var(--line);
  }
  .link {
    border: 0;
    background: transparent;
    color: var(--accent);
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .tiny {
    min-height: 20px;
    padding: 0 6px;
    font-size: var(--fs-xs);
  }
</style>
