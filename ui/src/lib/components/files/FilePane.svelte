<script module lang="ts">
  import type { Side as DragSide } from "../../api";
  type DragLoad = { side: DragSide; paths: string[]; from: string };
  /** One drag at a time across both panes. */
  const sharedDrag: { load: DragLoad | null } = { load: null };
</script>

<script lang="ts">
  import { scramble } from "../../scramble";
  import type { Entry, Side } from "../../api";
  import { formatMode, formatSize, formatTime, moveCursor, selectClick } from "../../files/util";
  import { files, type Pane } from "../../stores/files.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { ui } from "../../stores/ui.svelte";
  import AuthPrompt from "../terminal/AuthPrompt.svelte";
  import HostKeyPrompt from "../terminal/HostKeyPrompt.svelte";

  interface Props {
    pane: Pane;
    active: boolean;
    onactivate: () => void;
    onopenfile: (e: Entry) => void;
    ondelete: () => void;
    onrename: (e: Entry) => void;
    onmkdir: () => void;
    oncopy: () => void;
  }
  let { pane, active, onactivate, onopenfile, ondelete, onrename, onmkdir, oncopy }: Props = $props();

  const DRAG_TYPE = "application/x-nightgrid-files";
  /*
   * The drag in flight, shared by both panes. WebKit may hide custom
   * dataTransfer types (and their data until drop), so the panes read this
   * instead; the dataTransfer copy stays for engines that show it.
   */
  const dragNow = sharedDrag;
  const mac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

  let pathInput = $state("");
  let editingPath = $state(false);
  let dropTarget = $state<string | null>(null);
  let list: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!editingPath) pathInput = pane.path;
  });

  const order = $derived(pane.visible.map((e) => e.path));
  const selectedSize = $derived(pane.selectedEntries.reduce((a, e) => a + (e.kind === "dir" ? 0 : e.size), 0));
  const sideValue = $derived(pane.side.kind === "local" ? "local" : `host:${pane.side.id}`);

  function changeSide(v: string) {
    const side: Side = v === "local" ? { kind: "local" } : { kind: "host", id: Number(v.slice(5)) };
    void pane.open(side);
  }

  function isDirLike(e: Entry) {
    return e.kind === "dir" || e.targetKind === "dir";
  }

  function activate(e: Entry) {
    if (isDirLike(e)) void pane.load(e.path);
    else onopenfile(e);
  }

  function click(ev: MouseEvent, e: Entry) {
    onactivate();
    const r = selectClick(order, pane.selected, pane.anchor, e.path, {
      toggle: mac ? ev.metaKey : ev.ctrlKey,
      range: ev.shiftKey,
    });
    pane.selected = r.selected;
    pane.anchor = r.anchor;
    pane.cursor = e.path;
  }

  function keydown(ev: KeyboardEvent) {
    const mod = mac ? ev.metaKey : ev.ctrlKey;
    const cur = pane.visible.find((e) => e.path === pane.cursor) ?? null;
    const move = (delta: number) => {
      const next = moveCursor(order, pane.cursor, delta);
      if (!next) return;
      if (ev.shiftKey) {
        const r = selectClick(order, pane.selected, pane.anchor ?? pane.cursor ?? next, next, { toggle: false, range: true });
        pane.selected = r.selected;
      } else {
        pane.selected = [next];
        pane.anchor = next;
      }
      pane.cursor = next;
      list?.querySelector<HTMLElement>(`[data-path="${CSS.escape(next)}"]`)?.scrollIntoView({ block: "nearest" });
    };
    switch (ev.key) {
      case "ArrowDown":
        move(1);
        break;
      case "ArrowUp":
        move(-1);
        break;
      case "PageDown":
        move(15);
        break;
      case "PageUp":
        move(-15);
        break;
      case "Home":
        move(-1e9);
        break;
      case "End":
        move(1e9);
        break;
      case "Enter":
        if (cur) activate(cur);
        break;
      case "Backspace":
        void pane.up();
        break;
      case "Delete":
        if (pane.selected.length) ondelete();
        break;
      case "F2":
        if (cur) onrename(cur);
        break;
      case "F5":
        if (pane.selected.length) oncopy();
        break;
      case "F7":
        onmkdir();
        break;
      case " ":
        if (cur) {
          pane.selected = pane.selected.includes(cur.path)
            ? pane.selected.filter((p) => p !== cur.path)
            : [...pane.selected, cur.path];
          pane.anchor = cur.path;
        }
        break;
      case "a":
        if (!mod) return;
        pane.selected = [...order];
        break;
      default:
        return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }

  // --- drag & drop between panes (and onto folders) ---
  function dragstart(ev: DragEvent, e: Entry) {
    if (!pane.selected.includes(e.path)) {
      pane.selected = [e.path];
      pane.anchor = e.path;
    }
    const load: DragLoad = { side: pane.side, paths: [...pane.selected], from: pane.id };
    dragNow.load = load;
    if (ev.dataTransfer) {
      ev.dataTransfer.setData(DRAG_TYPE, JSON.stringify(load));
      // WebKit won't start a drag without a standard type.
      ev.dataTransfer.setData("text/plain", load.paths.join("\n"));
      ev.dataTransfer.effectAllowed = "copy";
    }
  }

  function dragend() {
    dragNow.load = null;
    dropTarget = null;
  }

  function dragover(ev: DragEvent, target: string) {
    if (!dragNow.load && !ev.dataTransfer?.types.includes(DRAG_TYPE)) return;
    ev.preventDefault();
    if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
    dropTarget = target;
  }

  function drop(ev: DragEvent, dir: string) {
    dropTarget = null;
    let d = dragNow.load;
    if (!d) {
      const raw = ev.dataTransfer?.getData(DRAG_TYPE);
      if (!raw) return;
      d = JSON.parse(raw) as DragLoad;
    }
    dragNow.load = null;
    ev.preventDefault();
    ev.stopPropagation();
    // Dropping onto the same folder it came from does nothing.
    const sameDir = d.from === pane.id && dir === pane.path;
    const intoSelf = d.from === pane.id && d.paths.includes(dir);
    if (sameDir || intoSelf) return;
    onactivate();
    void files.copyTo(d.side, d.paths, pane.side, dir);
  }

  function glyph(e: Entry): string {
    if (e.kind === "dir") return "▣";
    if (e.kind === "symlink") return e.targetKind === "dir" ? "⇢▣" : "⇢";
    if (e.kind === "other") return "◇";
    return "▤";
  }
</script>

<section
  class="pane"
  class:active
  aria-label="{pane.label} file pane"
  onfocusin={onactivate}
  onpointerdown={onactivate}
>
  <header class="bar">
    <select class="side" aria-label="Location" value={sideValue} onchange={(e) => changeSide(e.currentTarget.value)}>
      <option value="local">◆ LOCAL</option>
      {#each hosts.hosts as h (h.id)}
        <option value="host:{h.id}">{h.name}</option>
      {/each}
    </select>
    <div class="nav">
      <button type="button" class="tool" title="Back" aria-label="Back" onclick={() => pane.back()}>‹</button>
      <button type="button" class="tool" title="Up (Backspace)" aria-label="Up" disabled={!pane.parent} onclick={() => pane.up()}
        >↑</button
      >
      <button type="button" class="tool" title="Refresh" aria-label="Refresh" onclick={() => pane.refresh()}>⟳</button>
    </div>
    <form
      class="pathform"
      onsubmit={(e) => {
        e.preventDefault();
        editingPath = false;
        void pane.load(pathInput.trim() || "~");
      }}
    >
      <input
        class="path"
        aria-label="Path"
        spellcheck="false"
        autocomplete="off"
        bind:value={pathInput}
        onfocus={() => (editingPath = true)}
        onblur={() => (editingPath = false)}
      />
    </form>
    <button
      type="button"
      class="tool star"
      class:on={!!pane.bookmarked}
      title={pane.bookmarked ? "Remove bookmark" : "Bookmark this folder"}
      aria-pressed={!!pane.bookmarked}
      aria-label="Bookmark this folder"
      onclick={() => pane.toggleBookmark()}>★</button
    >
  </header>

  <div class="sub">
    {#if pane.bookmarks.length}
      <div class="marks" aria-label="Bookmarks">
        {#each pane.bookmarks as b (b.id)}
          <button type="button" class="mark" class:here={b.path === pane.path} title={b.path} onclick={() => pane.load(b.path)}
            >{b.label}</button
          >
        {/each}
      </div>
    {:else}
      <span class="muted hint">No bookmarks for {pane.label}. ★ saves the current folder.</span>
    {/if}
    <div class="spacer"></div>
    <input class="filter" placeholder="filter" aria-label="Filter" bind:value={pane.filter} />
    <button
      type="button"
      class="tool small"
      class:on={pane.showHidden}
      aria-pressed={pane.showHidden}
      title="Show hidden files"
      aria-label="Show hidden files"
      onclick={() => (pane.showHidden = !pane.showHidden)}>.*</button
    >
    <button type="button" class="tool small" title="New folder (F7)" onclick={onmkdir}>+DIR</button>
  </div>

  <div
    class="list"
    class:droppable={dropTarget === pane.path}
    role="grid"
    aria-label="Files in {pane.path}"
    aria-multiselectable="true"
    tabindex="0"
    bind:this={list}
    onkeydown={keydown}
    ondragover={(e) => dragover(e, pane.path)}
    ondragleave={() => (dropTarget = null)}
    ondrop={(e) => drop(e, pane.path)}
  >
    <div class="row head" role="row">
      <span role="columnheader" class="c-name">NAME</span>
      <span role="columnheader" class="c-size">SIZE</span>
      <span role="columnheader" class="c-time">MODIFIED</span>
      <span role="columnheader" class="c-mode">MODE</span>
    </div>
    {#if pane.parent}
      <div
        class="row up"
        role="row"
        tabindex="-1"
        ondblclick={() => pane.up()}
        ondragover={(e) => pane.parent && dragover(e, pane.parent)}
        ondrop={(e) => pane.parent && drop(e, pane.parent)}
        class:droppable={dropTarget === pane.parent}
      >
        <span class="c-name" role="gridcell"><span class="g">↰</span> ..</span>
      </div>
    {/if}
    {#each pane.visible as e (e.path)}
      {@const sel = pane.selected.includes(e.path)}
      <!-- Keyboard selection is handled by the grid (arrows, space, enter). -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="row"
        class:sel
        class:cursor={pane.cursor === e.path}
        class:dir={isDirLike(e)}
        class:droppable={dropTarget === e.path}
        role="row"
        aria-selected={sel}
        tabindex="-1"
        data-path={e.path}
        draggable="true"
        onclick={(ev) => click(ev, e)}
        ondblclick={() => activate(e)}
        ondragstart={(ev) => dragstart(ev, e)}
        ondragend={dragend}
        ondragover={(ev) => isDirLike(e) && dragover(ev, e.path)}
        ondrop={(ev) => isDirLike(e) && drop(ev, e.path)}
      >
        <span class="c-name" role="gridcell"><span class="g" aria-hidden="true">{glyph(e)}</span>{e.name}</span>
        <span class="c-size" role="gridcell">{isDirLike(e) ? "—" : formatSize(e.size)}</span>
        <span class="c-time" role="gridcell">{formatTime(e.mtime)}</span>
        <span class="c-mode" role="gridcell">{formatMode(e.mode, e.kind === "dir")}</span>
      </div>
    {/each}
    {#if !pane.loading && !pane.error && !pane.prompt && !pane.visible.length}
      <p class="muted empty">{pane.filter ? "Nothing matches the filter." : "Empty folder."}</p>
    {/if}
    {#if pane.error}
      <p class="error-text empty" role="alert" use:scramble={pane.error}></p>
    {/if}
  </div>

  <footer class="foot">
    <span>{pane.visible.length} items</span>
    {#if pane.selected.length}<span class="selinfo">{pane.selected.length} selected · {formatSize(selectedSize)}</span>{/if}
    <span class="spacer"></span>
    {#if pane.loading}<span class="loading">LOADING…</span>{/if}
    <span class="side-tag" class:remote={pane.side.kind === "host"}>{pane.label}</span>
  </footer>

  {#if pane.prompt}
    <div class="overlay">
      {#if pane.prompt.kind === "hostKeyUnknown" || pane.prompt.kind === "hostKeyChanged"}
        <HostKeyPrompt prompt={pane.prompt} onaccept={() => pane.acceptHostKey()} oncancel={() => pane.cancelPrompt()} />
      {:else if pane.prompt.kind === "passwordRequired" || pane.prompt.kind === "passphraseRequired"}
        <AuthPrompt
          prompt={pane.prompt}
          canRemember={ui.keychain}
          onsubmit={(s, r) => pane.provideSecret(s, r)}
          oncancel={() => pane.cancelPrompt()}
        />
      {:else}
        <p class="error-text">{pane.prompt.kind}</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .pane {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .pane.active {
    border-color: color-mix(in srgb, var(--magenta) 70%, var(--line));
    box-shadow: inset 0 2px 0 var(--magenta);
  }
  .bar,
  .sub {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
  }
  .sub {
    padding: 4px 8px;
    min-height: 30px;
  }
  .side {
    background: var(--bg-1);
    color: var(--text-0);
    border: 1px solid var(--line);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    text-transform: uppercase;
    padding: 4px 6px;
    max-width: 150px;
  }
  .nav {
    display: flex;
    gap: 2px;
  }
  .tool {
    min-width: 26px;
    height: 26px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: var(--fs-sm);
    padding: 0 6px;
  }
  .tool:hover:not(:disabled) {
    color: var(--text-0);
    border-color: var(--text-1);
  }
  .tool:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tool.on {
    color: var(--magenta);
    border-color: var(--magenta);
  }
  .tool.small {
    height: 22px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .star.on {
    text-shadow: var(--glow-active);
  }
  .pathform {
    flex: 1;
    min-width: 0;
  }
  .path,
  .filter {
    width: 100%;
    background: var(--bg-1);
    border: 1px solid var(--line);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    padding: 4px 8px;
  }
  .path:focus,
  .filter:focus {
    outline: none;
    border-color: var(--magenta);
  }
  .filter {
    width: 110px;
    padding: 2px 6px;
    font-size: var(--fs-xs);
  }
  .marks {
    display: flex;
    gap: 4px;
    overflow-x: auto;
    min-width: 0;
  }
  .mark {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--cyan);
    font-size: var(--fs-xs);
    padding: 2px 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .mark.here {
    border-color: var(--cyan);
    background: color-mix(in srgb, var(--cyan) 10%, transparent);
  }
  .hint {
    font-size: var(--fs-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .spacer {
    flex: 1;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    outline: none;
    font-size: var(--fs-sm);
  }
  .list:focus-visible {
    box-shadow: inset 0 0 0 1px var(--magenta);
  }
  .list.droppable {
    background: color-mix(in srgb, var(--mint) 6%, transparent);
    box-shadow: inset 0 0 0 1px var(--mint);
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 84px 118px 92px;
    gap: 8px;
    align-items: center;
    padding: 3px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 45%, transparent);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }
  .row.head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-0);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    border-bottom: 1px solid var(--line);
  }
  .row:focus {
    outline: none;
  }
  .row:not(.head):hover {
    background: color-mix(in srgb, var(--text-0) 4%, transparent);
  }
  .row.sel {
    background: color-mix(in srgb, var(--magenta) 16%, transparent);
    color: var(--text-0);
  }
  .row.cursor {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .row.droppable {
    background: color-mix(in srgb, var(--mint) 18%, transparent);
    box-shadow: inset 0 0 0 1px var(--mint);
  }
  .c-name {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-0);
  }
  .row.dir .c-name {
    color: var(--cyan);
  }
  .g {
    display: inline-block;
    width: 22px;
    color: var(--text-1);
    font-size: 11px;
  }
  .row.dir .g {
    color: var(--cyan);
  }
  .c-size,
  .c-time,
  .c-mode {
    color: var(--text-1);
    font-size: var(--fs-xs);
    font-family: var(--font-mono);
  }
  .c-size {
    text-align: right;
  }
  .row.head .c-size,
  .row.head .c-time,
  .row.head .c-mode {
    font-family: var(--font-display);
  }
  .empty {
    padding: 16px 12px;
    margin: 0;
  }
  .foot {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 4px 10px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.06em;
  }
  .selinfo {
    color: var(--magenta);
  }
  .loading {
    color: var(--warn);
  }
  .side-tag {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--mint);
  }
  .side-tag.remote {
    color: var(--cyan);
  }
  .overlay {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    padding: 16px;
    overflow: auto;
    background: color-mix(in srgb, var(--bg-0) 92%, transparent);
  }
</style>
