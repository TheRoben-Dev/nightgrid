<script lang="ts">
  import Barcode from "../components/Barcode.svelte";
  import SplitNode from "../components/terminal/SplitNode.svelte";
  import { DEFAULT_KEYMAP, formatChord } from "../keys";
  import HostIcon from "../components/HostIcon.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { terms } from "../stores/terms.svelte";
  import { leaves } from "../term/layout";

  const tab = $derived(terms.current);

  function tabTitle(id: number): string {
    const t = terms.tabs.find((x) => x.id === id);
    if (!t) return "";
    const all = leaves(t.root);
    const s = terms.session(t.active) ?? terms.session(all[0]);
    return (s?.title ?? "term") + (all.length > 1 ? ` +${all.length - 1}` : "");
  }

  function tabState(id: number): string {
    const t = terms.tabs.find((x) => x.id === id);
    return (t && terms.session(t.active)?.state) || "idle";
  }

  const k = (a: keyof typeof DEFAULT_KEYMAP) => formatChord(DEFAULT_KEYMAP[a]);
  const recent = $derived(hosts.hosts.filter((h) => h.favorite).concat(hosts.hosts.filter((h) => !h.favorite)).slice(0, 6));
</script>

<div class="term-view">
  <div class="tabs" role="tablist" aria-label="Terminal tabs">
    {#each terms.tabs as t (t.id)}
      <div class="tab" class:active={t.id === terms.activeTab}>
        <button
          type="button"
          role="tab"
          class="tab-btn"
          aria-selected={t.id === terms.activeTab}
          onclick={() => (terms.activeTab = t.id)}
        >
          <span class="dot {tabState(t.id)}" aria-hidden="true"></span>
          <span class="tname">{tabTitle(t.id)}</span>
        </button>
        <button type="button" class="tclose" aria-label="Close tab {tabTitle(t.id)}" onclick={() => terms.closeTab(t.id)}
          >×</button
        >
      </div>
    {/each}
    <button type="button" class="new" title="New local terminal ({k('term.newLocal')})" onclick={() => terms.openLocal()}
      >+ LOCAL</button
    >
    <div class="spacer"></div>
    {#if tab}
      <div class="tools">
        <button type="button" class="tool" title="Split right ({k('term.splitRight')})" onclick={() => terms.split("row")}
          >▥</button
        >
        <button type="button" class="tool" title="Split down ({k('term.splitDown')})" onclick={() => terms.split("col")}
          >▤</button
        >
        <button
          type="button"
          class="tool"
          title="Find ({k('term.find')})"
          onclick={() => {
            const s = terms.currentSession;
            if (s) s.searchOpen = true;
          }}>⌕</button
        >
      </div>
    {/if}
  </div>

  <div class="stage">
    {#each terms.tabs as t (t.id)}
      {#if t.id === terms.activeTab}
        <div class="tabbody">
          <SplitNode node={t.root} tabId={t.id} active={t.active} multi={t.root.kind === "split"} />
        </div>
      {/if}
    {/each}
    {#if !tab}
      <div class="empty">
        <div class="card">
          <div class="label"><span class="cyan">//</span> NO ACTIVE SESSIONS</div>
          <p>Open a shell on this machine, or pick a host to connect over SSH.</p>
          <div class="actions">
            <button type="button" class="btn primary" onclick={() => terms.openLocal()}>Local shell</button>
            <span class="muted kbd">{k("term.newLocal")}</span>
          </div>
          {#if recent.length}
            <div class="label sub"><span class="cyan">&gt;</span> CONNECT</div>
            <div class="hostgrid">
              {#each recent as h (h.id)}
                <button type="button" class="hosttile" onclick={() => terms.openHost(h.id)}>
                  <span class="stripe" style:background={h.color ?? "var(--line)"}></span>
                  <HostIcon art={h.icon} color={h.color} />
                  <span class="hn">{h.name}</span>
                  <span class="hh muted">{h.hostname}</span>
                </button>
              {/each}
            </div>
          {:else}
            <p class="muted small">
              No saved hosts yet. Use <strong>IMP</strong> or <strong>+</strong> next to NODES in the sidebar.
            </p>
          {/if}
          <Barcode seed="term-empty" bars={48} />
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .term-view {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .tabs {
    display: flex;
    align-items: stretch;
    gap: 2px;
    border-bottom: 1px solid var(--line);
    min-height: 30px;
    overflow-x: auto;
  }
  .tab {
    display: flex;
    align-items: stretch;
    border: 1px solid var(--line);
    border-bottom: 0;
    max-width: 220px;
  }
  .tab.active {
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 12%, transparent);
    box-shadow: inset 0 2px 0 var(--magenta);
  }
  .tab-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 0 6px 0 10px;
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  .tab.active .tab-btn {
    color: var(--text-0);
  }
  .tname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tclose {
    width: 22px;
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .tclose:hover {
    color: var(--text-0);
  }
  .dot {
    width: 6px;
    height: 6px;
    flex: none;
    border: 1px solid var(--text-1);
  }
  .dot.connected {
    background: var(--mint);
    border-color: var(--mint);
  }
  .dot.connecting,
  .dot.reconnecting,
  .dot.prompt {
    border-color: var(--warn);
  }
  .dot.failed,
  .dot.disconnected {
    border-color: var(--danger);
  }
  .new {
    padding: 0 10px;
    border: 1px dashed var(--line);
    border-bottom: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-weight: 600;
    letter-spacing: 0.12em;
    font-size: var(--fs-xs);
    cursor: pointer;
    white-space: nowrap;
  }
  .new:hover {
    color: var(--text-0);
    border-color: var(--mint);
  }
  .spacer {
    flex: 1;
  }
  .tools {
    display: flex;
    gap: 2px;
    align-items: center;
  }
  .tool {
    width: 28px;
    height: 24px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .tool:hover {
    color: var(--text-0);
    border-color: var(--mint);
  }
  .stage {
    flex: 1;
    min-height: 0;
    position: relative;
    margin-top: 6px;
  }
  .tabbody {
    position: absolute;
    inset: 0;
    display: flex;
  }
  .empty {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .card {
    width: min(520px, 100%);
    padding: 18px 20px;
    border: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg-1) 92%, transparent);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .card p {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .cyan {
    color: var(--cyan);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .sub {
    margin-top: 4px;
  }
  .hostgrid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 6px;
  }
  .hosttile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px 8px 14px;
    border: 1px solid var(--line);
    background: var(--bg-0);
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  .hosttile:hover {
    border-color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .stripe {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
  }
  .hn {
    font-size: var(--fs-sm);
  }
  .hh {
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
