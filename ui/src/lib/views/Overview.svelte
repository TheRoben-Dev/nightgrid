<script lang="ts">
  import type { AppInfo } from "../api";
  import Barcode from "../components/Barcode.svelte";
  import Panel from "../components/Panel.svelte";
  import { DEFAULT_KEYMAP, formatChord } from "../keys";
  import HostTile from "../components/metrics/HostTile.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { metrics } from "../stores/metrics.svelte";
  import { ui } from "../stores/ui.svelte";

  interface Props {
    info: AppInfo | null;
  }
  let { info }: Props = $props();

  // Overview tiles poll at the fast interval while this view is shown.
  $effect(() => {
    metrics.setVisible("overview", hosts.hosts.map((h) => h.id));
    return () => metrics.setVisible("overview", []);
  });

  const ROADMAP = [
    ["M0", "Skeleton", "done"],
    ["M1", "Hosts + terminal", "done"],
    ["M2", "SFTP + editor", "done"],
    ["M3", "Docker", "done"],
    ["M4", "Monitoring + overview", "done"],
    ["M5", "Services, Homepage import, tunnels", "next"],
    ["M6", "systemd, journal, updates, SMART", "todo"],
    ["M7", "Integrations, alerts, tray", "todo"],
    ["M8", "Automation", "todo"],
    ["M9", "Polish + ship", "todo"],
  ] as const;
</script>

<div class="grid">
  <div class="nodes">
    <Panel title="NODES" prefix=">" count={hosts.hosts.length} reveal>
      {#if hosts.hosts.length}
        <div class="hostgrid">
          {#each hosts.hosts as h (h.id)}
            <HostTile host={h} ping={hosts.pings[h.id]} />
          {/each}
        </div>
      {:else}
      <div class="empty">
        <div class="tiles" aria-hidden="true">
          {#each Array(6) as _, i (i)}<span class="ghost"></span>{/each}
        </div>
        <div class="msg">
          <p class="label">NO NODES REGISTERED</p>
          <p class="muted">
            Add a host or import your <code>~/.ssh/config</code>. Open the palette with
            <kbd>{formatChord(DEFAULT_KEYMAP["palette.open"])}</kbd> to look around.
          </p>
          <div class="actions">
            <button class="btn primary chamfer-sm" type="button" onclick={() => (hosts.editing = 0)}>Add host</button>
            <button class="btn chamfer-sm" type="button" onclick={() => (hosts.importOpen = true)}>Import ssh config</button>
            <button class="btn chamfer-sm" type="button" onclick={() => (ui.paletteOpen = true)}>Command palette</button>
          </div>
        </div>
      </div>
      {/if}
    </Panel>
  </div>

  <Panel title="SYS.STATUS" reveal>
    <dl class="kv">
      <dt>BUILD</dt>
      <dd>{info ? `v${info.version}` : "…"}</dd>
      <dt>PLATFORM</dt>
      <dd>{info ? `${info.os} / ${info.arch}` : "…"}</dd>
      <dt>DATABASE</dt>
      <dd>{info ? `schema v${info.schemaVersion}` : "…"}</dd>
      <dt>KEYCHAIN</dt>
      <dd class:ok={info?.keychain.available} class:warn={info && !info.keychain.available}>
        {#if info}
          <span aria-hidden="true">{info.keychain.available ? "●" : "▲"}</span>
          {info.keychain.available ? "available" : (info.keychain.detail ?? "unavailable")}
        {:else}…{/if}
      </dd>
      <dt>DATA DIR</dt>
      <dd class="path">{info?.dataDir ?? "…"}</dd>
    </dl>
    <div class="code"><Barcode seed={info?.version ?? "boot"} bars={44} tone="mint" /></div>
  </Panel>

  <Panel title="ROADMAP" prefix=">" count={ROADMAP.length}>
    <ol class="roadmap">
      {#each ROADMAP as [id, name, state] (id)}
        <li class={state}>
          <span class="tick" aria-hidden="true"></span>
          <span class="id">{id}</span>
          <span class="name">{name}</span>
          <span class="state">{state === "done" ? "DONE" : state === "next" ? "NEXT" : ""}</span>
        </li>
      {/each}
    </ol>
  </Panel>
</div>

<style>
  .hostgrid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
    gap: 12px;
    align-content: start;
  }
  .nodes {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
  }
  .empty {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: 24px;
    align-items: center;
    padding: 8px 4px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .ghost {
    height: 64px;
    border: 1px dashed var(--line);
    background-image: radial-gradient(color-mix(in srgb, var(--line) 90%, transparent) 1px, transparent 1px);
    background-size: 6px 6px;
  }
  .msg p {
    margin: 0 0 10px;
    line-height: 1.6;
  }
  .msg .label {
    color: var(--text-0);
    font-size: var(--fs-lg);
  }
  code {
    color: var(--cyan);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .kv {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 8px 12px;
    margin: 0;
  }
  dt {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    padding-top: 1px;
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  .path {
    overflow-wrap: anywhere;
    color: var(--text-1);
  }
  dd.ok span {
    color: var(--ok);
  }
  dd.warn span {
    color: var(--warn);
  }
  .code {
    margin-top: 14px;
  }

  .roadmap {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .roadmap li {
    display: grid;
    grid-template-columns: 22px 30px 1fr auto;
    align-items: center;
    gap: 8px;
    min-height: 24px;
    color: var(--text-1);
  }
  .tick {
    height: 3px;
    background: var(--line);
  }
  .done .tick {
    background: var(--mint);
  }
  .next .tick {
    background: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .done,
  .next {
    color: var(--text-0);
  }
  .id {
    font-size: var(--fs-xs);
  }
  .state {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
  }
  .done .state {
    color: var(--mint);
  }
  .next .state {
    color: var(--magenta);
  }

  @media (max-width: 1100px) {
    .grid,
    .empty {
      grid-template-columns: 1fr;
    }
  }
</style>
