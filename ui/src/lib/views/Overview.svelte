<script lang="ts">
  /**
   * Overview (M10o): named dashboards of widgets on a 12-column grid. Edit
   * mode moves (drag the title bar or arrow keys), resizes (corner handle or
   * Shift+arrows), adds and removes widgets; layouts live in the settings.
   */
  import type { AppInfo } from "../api";
  import Modal from "../components/Modal.svelte";
  import Panel from "../components/Panel.svelte";
  import DashWidget from "../components/dashboard/DashWidget.svelte";
  import CmdWidgetEditor from "../components/dashboard/CmdWidgetEditor.svelte";
  import { blank, type CmdWidget } from "../cmdwidgets";
  import { cmdWidgets } from "../stores/cmdwidgets.svelte";
  import {
    add,
    BUILTIN_PRESETS,
    cmdId,
    cmdKind,
    COLUMNS,
    compact,
    defaultDashboards,
    fromPreset,
    gridDelta,
    MAX_DASHBOARDS,
    MAX_PRESETS,
    MAX_WIDGETS,
    place,
    readingOrder,
    remove,
    uniqueName,
    widgetInfo,
    WIDGETS,
    type Dashboard,
    type Widget,
  } from "../dashboard";
  import { alerts } from "../stores/alerts.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { metrics } from "../stores/metrics.svelte";
  import { settings } from "../stores/settings.svelte";

  let { info }: { info: AppInfo | null } = $props();

  const ROW = 72;
  const GAP = 12;
  const NARROW = 760;

  const boards = $derived<Dashboard[]>(settings.value.dashboards?.length ? settings.value.dashboards : defaultDashboards());
  const presets = $derived<Dashboard[]>(settings.value.dashboardPresets ?? []);
  const active = $derived(Math.max(0, Math.min(settings.value.dashboardActive ?? 0, boards.length - 1)));
  const board = $derived(boards[active]);

  let editing = $state(false);
  let draft = $state<Widget[] | null>(null);
  let undo = $state<Widget[][]>([]);
  let menu = $state<"add" | "presets" | null>(null);
  let gridEl = $state<HTMLDivElement | null>(null);
  let width = $state(1200);

  const widgets = $derived(draft ?? board.widgets);
  const narrow = $derived(width < NARROW);
  const shown = $derived(narrow ? readingOrder(widgets) : widgets);
  const rows = $derived(widgets.reduce((n, w) => Math.max(n, w.y + w.h), 0));

  // Host tiles poll at the fast interval only while a NODES widget is shown.
  $effect(() => {
    const ids = widgets.some((w) => w.kind === "nodes") ? hosts.hosts.map((h) => h.id) : [];
    metrics.setVisible("overview", ids);
    return () => metrics.setVisible("overview", []);
  });

  $effect(() => {
    const el = gridEl;
    if (!el) return;
    const ro = new ResizeObserver(() => (width = el.clientWidth));
    ro.observe(el);
    width = el.clientWidth;
    return () => ro.disconnect();
  });

  function saveBoards(next: Dashboard[], activeIdx = active) {
    void settings.update({ dashboards: next, dashboardActive: activeIdx });
  }

  function saveWidgets(ws: Widget[], remember = true) {
    if (remember) undo = [...undo.slice(-19), board.widgets];
    saveBoards(boards.map((b, i) => (i === active ? { ...b, widgets: ws } : b)));
  }

  function undoLast() {
    const prev = undo.at(-1);
    if (!prev) return;
    undo = undo.slice(0, -1);
    saveWidgets(prev, false);
  }

  function pick(i: number) {
    if (i === active) return;
    undo = [];
    draft = null;
    void settings.update({ dashboardActive: i });
  }

  // ---- Dragging and resizing ----
  let drag = $state<{ id: string; mode: "move" | "resize"; x0: number; y0: number; orig: Widget; base: Widget[]; pointer: number } | null>(null);

  function colWidth() {
    return (width - GAP * (COLUMNS - 1)) / COLUMNS;
  }

  function start(e: PointerEvent, w: Widget, mode: "move" | "resize") {
    if (!editing || narrow || e.button !== 0) return;
    const t = e.target as HTMLElement;
    if (mode === "move" && (!t.closest("header") || t.closest("button"))) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { id: w.id, mode, x0: e.clientX, y0: e.clientY, orig: w, base: board.widgets, pointer: e.pointerId };
    draft = board.widgets;
  }

  function moveDrag(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    const d = gridDelta(e.clientX - drag.x0, e.clientY - drag.y0, colWidth(), ROW, GAP);
    const o = drag.orig;
    const moved = drag.mode === "move" ? { ...o, x: o.x + d.dx, y: o.y + d.dy } : { ...o, w: o.w + d.dx, h: o.h + d.dy };
    draft = place(drag.base, moved);
  }

  function endDrag(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    const changed = draft && JSON.stringify(draft) !== JSON.stringify(drag.base);
    if (changed && draft) saveWidgets(draft);
    drag = null;
    draft = null;
  }

  function onKey(e: KeyboardEvent, w: Widget) {
    const step: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const s = step[e.key];
    if (!s) {
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        saveWidgets(remove(board.widgets, w.id));
      }
      return;
    }
    e.preventDefault();
    const moved = e.shiftKey ? { ...w, w: w.w + s[0], h: w.h + s[1] } : { ...w, x: w.x + s[0], y: w.y + s[1] };
    saveWidgets(place(board.widgets, moved));
    // Keep focus on the same widget after the grid re-renders.
    queueMicrotask(() => gridEl?.querySelector<HTMLElement>(`[data-widget="${w.id}"] .grip`)?.focus());
  }

  // ---- Widgets, presets and dashboards ----
  function newCommand() {
    menu = null;
    cmdWidgets.editing = blank(hosts.hosts[0]?.id ?? 0);
  }

  function cmdSaved(w: CmdWidget, isNew: boolean) {
    if (isNew && board.widgets.length < MAX_WIDGETS) saveWidgets(add(board.widgets, cmdKind(w.id)));
  }

  /** A deleted command widget leaves every dashboard. */
  function cmdDeleted(id: number) {
    const kind = cmdKind(id);
    saveBoards(boards.map((b) => ({ ...b, widgets: compact(b.widgets.filter((w) => w.kind !== kind)) })));
  }

  const onBoard = $derived(new Set(board.widgets.map((w) => w.kind)));
  const spareCmds = $derived(cmdWidgets.list.filter((c) => !onBoard.has(cmdKind(c.id))));
  $effect(() => {
    if (menu === "add" && !cmdWidgets.loaded) void cmdWidgets.load();
  });

  function addWidget(kind: string) {
    menu = null;
    if (board.widgets.length >= MAX_WIDGETS) return;
    saveWidgets(add(board.widgets, kind));
  }

  function applyPreset(p: Dashboard) {
    menu = null;
    saveWidgets(fromPreset(p).widgets);
  }

  function resetBoard() {
    saveWidgets(fromPreset(BUILTIN_PRESETS[0]).widgets);
  }

  function deletePreset(i: number) {
    void settings.update({ dashboardPresets: presets.filter((_, j) => j !== i) });
  }

  let dialog = $state<{ mode: "new" | "rename" | "preset"; name: string; from: string } | null>(null);
  let confirmDelete = $state(false);

  function openDialog(mode: "new" | "rename" | "preset") {
    menu = null;
    const names = mode === "preset" ? presets.map((p) => p.name) : boards.map((b) => b.name);
    const name = mode === "rename" ? board.name : uniqueName(mode === "preset" ? board.name : "Dashboard", names);
    dialog = { mode, name, from: "empty" };
  }

  const nameTaken = $derived.by(() => {
    if (!dialog) return false;
    const n = dialog.name.trim();
    if (dialog.mode === "preset") return presets.some((p) => p.name === n);
    return boards.some((b, i) => b.name === n && !(dialog?.mode === "rename" && i === active));
  });
  const nameOk = $derived(!!dialog && dialog.name.trim().length > 0 && dialog.name.trim().length <= 40 && !nameTaken);

  function submitDialog() {
    if (!dialog || !nameOk) return;
    const name = dialog.name.trim();
    if (dialog.mode === "rename") {
      saveBoards(boards.map((b, i) => (i === active ? { ...b, name } : b)));
    } else if (dialog.mode === "preset") {
      void settings.update({ dashboardPresets: [...presets, fromPreset(board, name)] });
    } else {
      const src = dialog.from.startsWith("b:") ? BUILTIN_PRESETS[+dialog.from.slice(2)] : dialog.from.startsWith("p:") ? presets[+dialog.from.slice(2)] : null;
      const next = src ? fromPreset(src, name) : { name, widgets: [] };
      undo = [];
      saveBoards([...boards, next], boards.length);
      editing = true;
    }
    dialog = null;
  }

  function deleteBoard() {
    confirmDelete = false;
    if (boards.length <= 1) return;
    undo = [];
    saveBoards(
      boards.filter((_, i) => i !== active),
      Math.max(0, active - 1),
    );
  }

  const titleOf = (w: Widget) => {
    const c = cmdId(w.kind);
    if (c != null) return cmdWidgets.byId(c)?.name.toUpperCase() ?? "COMMAND";
    return widgetInfo(w.kind)?.title ?? w.kind.toUpperCase();
  };
  const countOf = (w: Widget) => (w.kind === "nodes" ? hosts.hosts.length : w.kind === "alerts" ? alerts.roots.length : undefined);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && menu && (menu = null)} />

<div class="dash" class:editing>
  <div class="bar">
    <div class="tabs" role="tablist" aria-label="Dashboards">
      {#each boards as b, i (i)}
        <button type="button" role="tab" class="tab" class:on={i === active} aria-selected={i === active} onclick={() => pick(i)}>{b.name}</button>
      {/each}
      {#if editing && boards.length < MAX_DASHBOARDS}
        <button type="button" class="tab add" data-guide="dashboard-new" title="New dashboard" onclick={() => openDialog("new")}>+ New</button>
      {/if}
    </div>
    <span class="spacer"></span>
    {#if editing}
      <div class="pop">
        <button type="button" class="btn tiny" data-guide="dashboard-add" disabled={board.widgets.length >= MAX_WIDGETS} aria-expanded={menu === "add"} onclick={() => (menu = menu === "add" ? null : "add")}>+ Widget</button>
        {#if menu === "add"}
          <ul class="menu" role="menu">
            {#each WIDGETS as wi (wi.kind)}
              <li><button type="button" role="menuitem" onclick={() => addWidget(wi.kind)}><span class="mt">{wi.title}</span><span class="muted">{wi.what}</span></button></li>
            {/each}
            <li class="sep"><button type="button" role="menuitem" data-guide="dashboard-add-command" onclick={newCommand}><span class="mt">COMMAND…</span><span class="muted">Any read-only command's output, with thresholds</span></button></li>
            {#if spareCmds.length}<li class="hd muted">YOUR COMMANDS</li>{/if}
            {#each spareCmds as c (c.id)}
              <li><button type="button" role="menuitem" onclick={() => addWidget(cmdKind(c.id))}><span class="mt">{c.name.toUpperCase()}</span><span class="muted mono">{c.command}</span></button></li>
            {/each}
          </ul>
        {/if}
      </div>
      <div class="pop">
        <button type="button" class="btn tiny" data-guide="dashboard-presets" aria-expanded={menu === "presets"} onclick={() => (menu = menu === "presets" ? null : "presets")}>Presets</button>
        {#if menu === "presets"}
          <ul class="menu" role="menu">
            <li class="hd muted">BUILT IN</li>
            {#each BUILTIN_PRESETS as p (p.name)}
              <li><button type="button" role="menuitem" onclick={() => applyPreset(p)}><span class="mt">{p.name}</span><span class="muted">{p.widgets.length} widgets</span></button></li>
            {/each}
            {#if presets.length}<li class="hd muted">SAVED</li>{/if}
            {#each presets as p, i (p.name)}
              <li class="saved">
                <button type="button" role="menuitem" onclick={() => applyPreset(p)}><span class="mt">{p.name}</span><span class="muted">{p.widgets.length} widgets</span></button>
                <button type="button" class="x" aria-label="Delete preset {p.name}" title="Delete this preset" onclick={() => deletePreset(i)}>×</button>
              </li>
            {/each}
            <li class="sep"><button type="button" role="menuitem" disabled={presets.length >= MAX_PRESETS} onclick={() => openDialog("preset")}>Save this layout as a preset…</button></li>
          </ul>
        {/if}
      </div>
      <button type="button" class="btn tiny ghost" disabled={!undo.length} onclick={undoLast}>Undo</button>
      <button type="button" class="btn tiny ghost" title="Back to the built-in Overview layout" onclick={resetBoard}>Reset</button>
      <button type="button" class="btn tiny ghost" onclick={() => openDialog("rename")}>Rename</button>
      {#if boards.length > 1}
        {#if confirmDelete}
          <span class="muted small">Delete “{board.name}”?</span>
          <button type="button" class="btn tiny danger" onclick={deleteBoard}>Delete</button>
          <button type="button" class="btn tiny ghost" onclick={() => (confirmDelete = false)}>Keep</button>
        {:else}
          <button type="button" class="btn tiny ghost" onclick={() => (confirmDelete = true)}>Delete</button>
        {/if}
      {/if}
    {/if}
    <button
      type="button"
      class="btn tiny"
      class:primary={editing}
      data-guide="dashboard-edit"
      onclick={() => {
        editing = !editing;
        menu = null;
        confirmDelete = false;
      }}>{editing ? "Done" : "Edit"}</button
    >
  </div>

  {#if editing}
    <p class="hint muted small">
      {narrow ? "Widen the window to arrange widgets." : "Drag a title bar to move, the corner to resize. Focused: arrows move, Shift+arrows resize, Delete removes."}
    </p>
  {/if}

  <div
    class="grid"
    class:narrow
    bind:this={gridEl}
    style:--row="{ROW}px"
    style:--gap="{GAP}px"
    style:min-height={editing && !narrow ? `${(rows + 3) * (ROW + GAP)}px` : null}
  >
    {#each shown as w (w.id)}
      <div
        class="cell"
        class:dragging={drag?.id === w.id}
        data-widget={w.id}
        role="group"
        aria-label={titleOf(w)}
        style:grid-column={narrow ? null : `${w.x + 1} / span ${w.w}`}
        style:grid-row={narrow ? null : `${w.y + 1} / span ${w.h}`}
        style:height={narrow ? `${Math.min(w.h, 4) * ROW + (Math.min(w.h, 4) - 1) * GAP}px` : null}
        onpointerdown={(e) => start(e, w, "move")}
        onpointermove={moveDrag}
        onpointerup={endDrag}
        onpointercancel={endDrag}
      >
        <Panel title={titleOf(w)} prefix={w.kind === "nodes" || w.kind === "alerts" ? ">" : "//"} count={countOf(w)} reveal={!editing}>
          {#snippet actions()}
            {#if editing && !narrow}
              <button type="button" class="grip" aria-label="Move or resize {titleOf(w)} with the arrow keys" title="Arrows move, Shift+arrows resize, Delete removes" onkeydown={(e) => onKey(e, w)}>⠿</button>
              <button type="button" class="x" aria-label="Remove {titleOf(w)}" title="Remove" onclick={() => saveWidgets(remove(board.widgets, w.id))}>×</button>
            {/if}
          {/snippet}
          <div class="content"><DashWidget kind={w.kind} {info} /></div>
        </Panel>
        {#if editing && !narrow}
          <span class="resize" aria-hidden="true" onpointerdown={(e) => start(e, w, "resize")} onpointermove={moveDrag} onpointerup={endDrag}></span>
        {/if}
      </div>
    {:else}
      <div class="blank">
        <p class="label">EMPTY DASHBOARD</p>
        <p class="muted">{editing ? "Add a widget or apply a preset." : "Press Edit to add widgets."}</p>
      </div>
    {/each}
  </div>
</div>

<CmdWidgetEditor onsaved={cmdSaved} ondeleted={cmdDeleted} />

<Modal
  open={dialog !== null}
  title={dialog?.mode === "rename" ? "RENAME DASHBOARD" : dialog?.mode === "preset" ? "SAVE AS PRESET" : "NEW DASHBOARD"}
  width={440}
  onclose={() => (dialog = null)}
>
  {#if dialog}
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        submitDialog();
      }}
    >
      <label class="field"><span class="label">NAME</span><input class="input" maxlength="40" bind:value={dialog.name} /></label>
      {#if nameTaken}<p class="error-text small">That name is taken.</p>{/if}
      {#if dialog.mode === "new"}
        <label class="field">
          <span class="label">START FROM</span>
          <select class="input" bind:value={dialog.from}>
            <option value="empty">Empty</option>
            {#each BUILTIN_PRESETS as p, i (p.name)}<option value="b:{i}">{p.name} (built in)</option>{/each}
            {#each presets as p, i (p.name)}<option value="p:{i}">{p.name}</option>{/each}
          </select>
        </label>
      {:else if dialog.mode === "preset"}
        <p class="muted small">Saves the widgets of “{board.name}” so any dashboard can start from them. Presets travel in profile exports.</p>
      {/if}
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (dialog = null)}>Cancel</button>
    <button type="button" class="btn primary" disabled={!nameOk} onclick={submitDialog}>{dialog?.mode === "rename" ? "Rename" : dialog?.mode === "preset" ? "Save" : "Create"}</button>
  {/snippet}
</Modal>

<style>
  .dash {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .bar {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .tabs {
    display: flex;
    gap: 2px;
    flex-wrap: wrap;
    min-width: 0;
  }
  .tab {
    background: none;
    border: 1px solid transparent;
    border-bottom-color: var(--line);
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    text-transform: uppercase;
    padding: 5px 12px;
    cursor: pointer;
    max-width: 24ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tab:hover {
    color: var(--text-0);
  }
  .tab.on {
    color: var(--mint);
    border-color: var(--line);
    border-bottom-color: var(--mint);
    background: color-mix(in srgb, var(--mint) 6%, transparent);
  }
  .tab.add {
    color: var(--cyan);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .hint {
    margin: -4px 0 0;
  }

  .pop {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 20;
    list-style: none;
    margin: 0;
    padding: 4px;
    min-width: 280px;
    max-height: 60vh;
    overflow: auto;
    background: var(--bg-1);
    border: 1px solid var(--line);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
  }
  .menu button {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    text-align: left;
    background: none;
    border: 0;
    color: var(--text-0);
    padding: 6px 8px;
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-sm);
  }
  .menu button:hover:not(:disabled),
  .menu button:focus-visible {
    background: color-mix(in srgb, var(--mint) 10%, transparent);
  }
  .menu button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .menu .muted {
    font-size: var(--fs-xs);
  }
  .mt {
    letter-spacing: 0.06em;
  }
  .menu .mono {
    font-family: var(--font-mono);
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .menu .hd {
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    padding: 6px 8px 2px;
  }
  .menu .saved {
    display: flex;
  }
  .menu .saved .x {
    width: auto;
    padding: 0 10px;
    align-self: center;
  }
  .menu .sep {
    border-top: 1px solid var(--line);
    margin-top: 4px;
    padding-top: 4px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    grid-auto-rows: var(--row);
    gap: var(--gap);
    position: relative;
  }
  .grid.narrow {
    grid-template-columns: minmax(0, 1fr);
    grid-auto-rows: auto;
  }
  .editing .grid:not(.narrow) {
    background-image:
      linear-gradient(to right, color-mix(in srgb, var(--line) 35%, transparent) 1px, transparent 1px),
      linear-gradient(to bottom, color-mix(in srgb, var(--line) 35%, transparent) 1px, transparent 1px);
    background-size: calc((100% + var(--gap)) / 12) calc(var(--row) + var(--gap));
  }
  .cell {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    transition:
      transform var(--dur) var(--ease),
      opacity var(--dur) var(--ease);
  }
  .cell > :global(.panel) {
    flex: 1;
    min-height: 0;
  }
  .cell :global(.panel > .body) {
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .content {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    gap: 6px;
  }
  .editing .cell :global(.panel > header) {
    cursor: grab;
    user-select: none;
  }
  .editing .cell :global(.panel) {
    border-style: dashed;
  }
  .editing .cell .content {
    pointer-events: none;
    opacity: 0.75;
  }
  .cell.dragging {
    z-index: 5;
    opacity: 0.9;
  }
  .cell.dragging :global(.panel) {
    border-color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .grip,
  .x {
    background: none;
    border: 0;
    color: var(--text-1);
    cursor: pointer;
    font: inherit;
    padding: 0 4px;
    line-height: 1;
  }
  .grip {
    cursor: move;
  }
  .grip:hover,
  .grip:focus-visible {
    color: var(--mint);
  }
  .x:hover {
    color: var(--bad);
  }
  .resize {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 16px;
    height: 16px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, var(--mint) 50%, var(--mint) 60%, transparent 60%, transparent 70%, var(--mint) 70%, var(--mint) 80%, transparent 80%);
    touch-action: none;
  }
  .blank {
    grid-column: 1 / -1;
    grid-row: span 3;
    border: 1px dashed var(--line);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
  }
  .blank p {
    margin: 0;
  }
  .blank .label {
    color: var(--text-0);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .error-text {
    margin: 0;
  }
</style>
