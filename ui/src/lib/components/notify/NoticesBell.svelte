<script lang="ts">
  /** The inbox (1.1): a bell in the top bar with the unread count and the latest notices. */
  import { onMount } from "svelte";
  import { notices } from "../../stores/notices.svelte";
  import { ago, levelTone, sourceLabel } from "../../notices";
  import { ui } from "../../stores/ui.svelte";

  let root = $state<HTMLElement | null>(null);
  let onlyUnread = $state(false);
  const shown = $derived(notices.list.filter((n) => !onlyUnread || !n.read).slice(0, 80));

  onMount(() => notices.start());

  function onDoc(e: MouseEvent) {
    if (notices.open && root && !root.contains(e.target as Node)) notices.open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (notices.open && e.key === "Escape") notices.open = false;
  }
  function settingsOpen() {
    notices.open = false;
    ui.settingsTab = "alerts";
    ui.go("settings");
  }
</script>

<svelte:document onmousedown={onDoc} onkeydown={onKey} />

<div class="bellwrap" bind:this={root}>
  <button
    type="button"
    class="ind bell"
    class:on={notices.open}
    data-guide="notices"
    title="Notifications"
    aria-haspopup="dialog"
    aria-expanded={notices.open}
    onclick={() => (notices.open = !notices.open)}
    ><span class="glyph" aria-hidden="true">◈</span> INBOX{#if notices.unread}<b class="count">{notices.unread > 99 ? "99+" : notices.unread}</b>{/if}</button
  >
  {#if notices.open}
    <div class="pop" role="dialog" aria-label="Notifications">
      <div class="head">
        <span class="label">NOTIFICATIONS</span>
        <span class="sp"></span>
        <label class="muted small chk"><input type="checkbox" bind:checked={onlyUnread} /> unread</label>
        <button type="button" class="link small" disabled={!notices.unread} onclick={() => notices.markRead(null)}>Mark all read</button>
        <button type="button" class="link small" disabled={!notices.list.length} onclick={() => notices.clear()}>Clear</button>
      </div>
      <div class="list">
        {#each shown as n (n.id)}
          <button type="button" class="item {levelTone(n.level)}" class:unread={!n.read} onclick={() => notices.follow(n)}>
            <span class="dot" aria-hidden="true"></span>
            <span class="txt">
              <span class="t">{n.title}</span>
              {#if n.body}<span class="b muted">{n.body}</span>{/if}
              <span class="m muted">{sourceLabel(n.source)} · {ago(n.at)}</span>
            </span>
          </button>
        {:else}
          <p class="muted small pad">{onlyUnread ? "Nothing unread." : "Nothing yet. Alerts, restore drills, rotations due and posture drops show up here."}</p>
        {/each}
      </div>
      <div class="foot">
        <button type="button" class="link small" onclick={settingsOpen}>Where notifications go…</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .bellwrap {
    position: relative;
  }
  .bell {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .bell:focus-visible {
    outline: 1px solid var(--accent);
  }
  .bell.on {
    color: var(--magenta);
  }
  .glyph {
    color: var(--cyan);
  }
  .count {
    padding: 0 4px;
    background: var(--magenta);
    color: var(--bg-0);
    font-weight: 600;
  }
  .pop {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    width: min(420px, 90vw);
    max-height: min(560px, 75vh);
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--line);
    box-shadow: 0 10px 30px rgb(0 0 0 / 45%);
    z-index: 60;
    letter-spacing: normal;
  }
  .head,
  .foot {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line);
  }
  .foot {
    border-bottom: 0;
    border-top: 1px solid var(--line);
  }
  .sp {
    flex: 1;
  }
  .chk {
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .list {
    overflow: auto;
    min-height: 0;
  }
  .item {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    gap: 10px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    margin-top: 6px;
    background: var(--text-2);
  }
  .item.bad .dot {
    background: var(--danger);
  }
  .item.warn .dot {
    background: var(--warn);
  }
  .item.ok .dot {
    background: var(--ok);
  }
  .item.info .dot {
    background: var(--cyan);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .t {
    color: var(--text-1);
  }
  .unread .t {
    color: var(--text-0);
    font-weight: 600;
  }
  .b {
    font-size: var(--fs-xs);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .m {
    font-size: var(--fs-xs);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
  }
  .link:disabled {
    color: var(--text-2);
    cursor: default;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 14px 10px;
    margin: 0;
  }
</style>
