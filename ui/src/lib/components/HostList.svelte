<script lang="ts">
  import type { Host } from "../api";
  import { hosts } from "../stores/hosts.svelte";
  import { metrics } from "../stores/metrics.svelte";

  const PLANNED: Record<string, string> = { suspended: "suspended", rebooting: "rebooting", shuttingDown: "shut down", asleep: "asleep (sleep window)", away: "away (client device)" };
  import { terms } from "../stores/terms.svelte";

  let collapsedGroups = $state<Record<string, boolean>>({});
  const groups = $derived(hosts.grouped);
  const total = $derived(hosts.hosts.length);

  function status(h: Host): "online" | "offline" | "unknown" | "connected" | "asleep" {
    const p = hosts.pings[h.id];
    if (!p) return "unknown";
    if (p.connected) return "connected";
    if (p.state !== "online" && PLANNED[metrics.get(h.id).state]) return "asleep";
    // A client device that isn't answering is away, not down.
    if (p.state === "offline" && h.role === "client" && !h.alertOffline) return "asleep";
    return p.state === "online" ? "online" : p.state === "offline" ? "offline" : "unknown";
  }

  function title(h: Host): string {
    const p = hosts.pings[h.id];
    const target = `${h.username ? `${h.username}@` : ""}${h.hostname}${h.port !== 22 ? `:${h.port}` : ""}`;
    const planned = PLANNED[metrics.get(h.id).state] ?? (h.role === "client" && !h.alertOffline ? PLANNED.away : undefined);
    const state = !p
      ? "not checked yet"
      : planned && !p.connected && p.state !== "online"
        ? planned
      : p.connected
        ? "connected"
        : p.state === "online"
          ? `reachable${p.latencyMs !== null ? ` (${p.latencyMs} ms)` : ""}`
          : p.state === "offline"
            ? "unreachable"
            : "behind a jump host";
    const tags = h.tags.length ? ` · #${h.tags.join(" #")}` : "";
    const open = terms.shellFor(h.id) ? "Click: go to its terminal · Alt-click: new terminal" : "Click: open a terminal";
    return `${target} · ${state}${tags}\n${open}`;
  }
</script>

<div class="hosts">
  <div class="head">
    <div class="label"><span class="cyan">&gt;</span> NODES <span class="muted">[{total}]</span></div>
    <div class="tools">
      <button type="button" class="tool" data-guide="hosts-import" title="Import from ~/.ssh/config" onclick={() => (hosts.importOpen = true)}
        >IMP</button
      >
      <button type="button" class="tool add" data-guide="hosts-add" title="Add host" aria-label="Add host" onclick={() => (hosts.editing = 0)}
        >+</button
      >
    </div>
  </div>

  {#if total > 0}
    <input class="search input" type="search" placeholder="filter · #tag" aria-label="Filter hosts" bind:value={hosts.query} />
  {/if}

  <div class="list">
    {#if hosts.loaded && total === 0}
      <div class="empty">
        <p>No hosts yet.</p>
        <button type="button" class="btn ghost" onclick={() => (hosts.importOpen = true)}>Import ~/.ssh/config</button>
        <button type="button" class="btn ghost" onclick={() => (hosts.editing = 0)}>Add host</button>
      </div>
    {:else if total > 0 && groups.length === 0}
      <p class="none">No match.</p>
    {/if}
    {#each groups as g (g.id ?? "loose")}
      {@const key = String(g.id)}
      <div class="group">
        <button
          type="button"
          class="gname"
          aria-expanded={!collapsedGroups[key]}
          onclick={() => (collapsedGroups[key] = !collapsedGroups[key])}
        >
          <span class="caret" aria-hidden="true">{collapsedGroups[key] ? "▸" : "▾"}</span>
          {#if g.color}<i class="gdot" style:background={g.color}></i>{/if}
          <span>{g.name}</span>
          <span class="muted">{g.hosts.length}</span>
        </button>
        {#if !collapsedGroups[key]}
          <ul>
            {#each g.hosts as h (h.id)}
              {@const st = status(h)}
              <li class="row">
                <button
                  type="button"
                  class="host"
                  title={title(h)}
                  onclick={(e) => terms.goToHost(h.id, e.altKey || e.metaKey || e.ctrlKey)}
                  aria-label="Connect to {h.name} ({st})"
                >
                  <span class="stripe" style:background={h.color ?? "transparent"} aria-hidden="true"></span>
                  <span class="dot {st}" aria-hidden="true"></span>
                  <span class="name">{h.name}</span>
                  {#if h.favorite}<span class="star" aria-label="favorite">★</span>{/if}
                </button>
                <button type="button" class="edit" aria-label="Edit {h.name}" title="Edit" onclick={() => (hosts.editing = h.id)}
                  >⋯</button
                >
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .hosts {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    padding: 12px 8px 4px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 6px 6px;
  }
  .cyan {
    color: var(--cyan);
  }
  .tools {
    display: flex;
    gap: 4px;
  }
  .tool {
    min-width: 24px;
    height: 22px;
    padding: 0 6px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-size: 10px;
    letter-spacing: 0.08em;
    cursor: pointer;
  }
  .tool:hover {
    color: var(--text-0);
    border-color: var(--mint);
  }
  .tool.add {
    font-size: 14px;
    line-height: 1;
  }
  .search {
    margin: 0 4px 6px;
    min-height: 26px;
    font-size: var(--fs-sm);
  }
  .list {
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }
  .empty {
    margin: 0 6px;
    padding: 10px;
    border: 1px dashed var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .empty p,
  .none {
    margin: 0 0 4px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .none {
    padding: 0 8px;
  }
  .gname {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    text-transform: uppercase;
    cursor: pointer;
  }
  .gname .muted {
    margin-left: auto;
  }
  .caret {
    width: 10px;
  }
  .gdot {
    width: 6px;
    height: 6px;
  }
  ul {
    list-style: none;
    margin: 0 0 6px;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: stretch;
    position: relative;
  }
  .host {
    flex: 1;
    min-width: 0;
    min-height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 6px 0 0;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-0);
    cursor: pointer;
    text-align: left;
  }
  .host:hover {
    background: var(--bg-2);
    border-color: var(--line);
  }
  .stripe {
    width: 3px;
    align-self: stretch;
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    border: 1px solid var(--text-1);
  }
  .dot.online {
    background: var(--mint);
    border-color: var(--mint);
  }
  .dot.connected {
    background: var(--magenta);
    border-color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .dot.offline {
    border-color: var(--danger);
    background: transparent;
  }
  .dot.asleep {
    border-color: var(--cyan);
    border-style: dashed;
    background: transparent;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
  }
  .star {
    color: var(--accent);
    font-size: 10px;
  }
  .edit {
    width: 24px;
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .edit,
  .edit:focus-visible {
    opacity: 1;
  }
  .edit:hover {
    color: var(--text-0);
  }
</style>
