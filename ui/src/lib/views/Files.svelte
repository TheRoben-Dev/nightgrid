<script lang="ts">
  import { typedMatches } from "../confirm";
  import { scramble } from "../scramble";
  import { api, errorText, type Entry } from "../api";
  import Modal from "../components/Modal.svelte";
  import Resizer from "../components/Resizer.svelte";
  import EditorTab from "../components/files/EditorTab.svelte";
  import FilePane from "../components/files/FilePane.svelte";
  import TransferQueue from "../components/files/TransferQueue.svelte";
  import { files, type Pane } from "../stores/files.svelte";
  import { layout } from "../stores/layout.svelte";

  $effect(() => {
    files.start();
  });

  // --- dialogs ---
  let del = $state<{ pane: Pane; entries: Entry[] } | null>(null);
  let delTyped = $state("");
  let nameDlg = $state<{ pane: Pane; mode: "mkdir" | "rename"; entry?: Entry } | null>(null);
  let nameValue = $state("");
  let dlgError = $state<string | null>(null);
  let busy = $state(false);
  let closing = $state<number | null>(null);

  /** Bulk deletes on a host need the host name typed. */
  const delNeedsTyping = $derived(!!del && del.pane.side.kind === "host" && del.entries.length > 1);
  const delOk = $derived(!delNeedsTyping || typedMatches(delTyped, del?.pane.label));

  function askDelete(p: Pane) {
    if (!p.selectedEntries.length) return;
    del = { pane: p, entries: p.selectedEntries };
    delTyped = "";
    dlgError = null;
  }

  async function doDelete() {
    if (!del || !delOk) return;
    busy = true;
    try {
      await api.fsDelete(
        del.pane.side,
        del.entries.map((e) => e.path),
      );
      const p = del.pane;
      del = null;
      p.selected = [];
      await p.refresh();
    } catch (e) {
      dlgError = errorText(e);
    } finally {
      busy = false;
    }
  }

  function askName(pane: Pane, mode: "mkdir" | "rename", entry?: Entry) {
    nameDlg = { pane, mode, entry };
    nameValue = entry?.name ?? "";
    dlgError = null;
  }

  async function doName(e: Event) {
    e.preventDefault();
    if (!nameDlg || !nameValue.trim()) return;
    busy = true;
    try {
      const { pane, mode, entry } = nameDlg;
      const created =
        mode === "mkdir"
          ? await api.fsMkdir(pane.side, pane.path, nameValue.trim())
          : await api.fsRename(pane.side, entry!.path, nameValue.trim());
      nameDlg = null;
      await pane.refresh();
      pane.selected = [created];
      pane.cursor = created;
      pane.anchor = created;
    } catch (err) {
      dlgError = errorText(err);
    } finally {
      busy = false;
    }
  }

  function copySel(p: Pane) {
    void files.copy(p, p.selectedEntries);
  }

  function tryClose(id: number) {
    const d = files.docs.find((x) => x.id === id);
    if (d?.dirty) closing = id;
    else files.closeDoc(id);
  }

  /** "Save & return" / "Close without saving" on a tab opened from a terminal. */
  async function saveAndReturn(id: number, save: boolean) {
    const d = files.docs.find((x) => x.id === id);
    if (!d) return;
    if (save) {
      if (d.dirty || d.isNew) await d.save();
      // Keep the tab if the save failed or hit a conflict.
      if (d.error || d.conflict || d.dirty) return;
      files.closeDoc(id);
    } else tryClose(id);
  }

  const ap = $derived(files.activePane);
  const other = $derived(files.other(ap));
</script>

<div class="files-view">
  <div class="tabs" role="tablist" aria-label="Files tabs">
    <div class="tab" class:active={files.tab === "browser"}>
      <button type="button" role="tab" class="tab-btn" aria-selected={files.tab === "browser"} onclick={() => (files.tab = "browser")}>
        <span class="dot" aria-hidden="true"></span>BROWSER
      </button>
    </div>
    {#each files.docs as d (d.id)}
      <div class="tab" class:active={files.tab === d.id}>
        <button type="button" role="tab" class="tab-btn" aria-selected={files.tab === d.id} title="{d.where}:{d.path}" onclick={() => (files.tab = d.id)}>
          <span class="dot" class:dirty={d.dirty} aria-hidden="true"></span>
          <span class="tname">{d.name}</span>
          <span class="tw">{d.where}</span>
        </button>
        <button type="button" class="tclose" aria-label="Close {d.name}" onclick={() => tryClose(d.id)}>×</button>
      </div>
    {/each}
    <div class="spacer"></div>
    {#if files.tab === "browser"}
      <div class="actions">
        <button type="button" class="act" disabled={!ap.selected.length} title="Copy to the other pane (F5)" onclick={() => copySel(ap)}>
          COPY {files.active === 0 ? "→" : "←"} {other.label}
        </button>
        <button
          type="button"
          class="act"
          disabled={ap.selectedEntries.length !== 1 || ap.selectedEntries[0]?.kind === "dir"}
          title="Edit in the built-in editor"
          onclick={() => ap.selectedEntries[0] && files.openDoc(ap.side, ap.selectedEntries[0])}>EDIT</button
        >
        <button
          type="button"
          class="act"
          disabled={ap.selectedEntries.length !== 1}
          title="Rename (F2)"
          onclick={() => ap.selectedEntries[0] && askName(ap, "rename", ap.selectedEntries[0])}>RENAME</button
        >
        <button type="button" class="act danger" disabled={!ap.selected.length} title="Delete (Del)" onclick={() => askDelete(ap)}>DELETE</button>
      </div>
    {/if}
  </div>

  <div class="stage">
    <div class="browser" class:hidden={files.tab !== "browser"}>
      <div class="panes" style:grid-template-columns="minmax(0, {layout.get('filesSplit')}fr) auto minmax(0, {100 - layout.get('filesSplit')}fr)">
        {#each files.panes as p, i (p.id)}
          {#if i === 1}<Resizer key="filesSplit" axis="x" label="Resize file panes" />{/if}
          <FilePane
            pane={p}
            active={files.active === i}
            onactivate={() => (files.active = i as 0 | 1)}
            onopenfile={(e) => files.openDoc(p.side, e)}
            ondelete={() => askDelete(p)}
            onrename={(e) => askName(p, "rename", e)}
            onmkdir={() => askName(p, "mkdir")}
            oncopy={() => copySel(p)}
          />
        {/each}
      </div>
      {#if files.queueOpen}<Resizer key="filesQueue" axis="y" invert label="Resize transfer queue" />{/if}
      <TransferQueue />
    </div>
    {#each files.docs as d (d.id)}
      <div class="doc" class:hidden={files.tab !== d.id}>
        <EditorTab doc={d} onreturn={(save) => void saveAndReturn(d.id, save)} />
      </div>
    {/each}
  </div>
</div>

<Modal open={!!del} title="DELETE" tone="danger" width={560} onclose={() => (del = null)}>
  {#if del}
    <p>
      Permanently delete {del.entries.length === 1 ? "this item" : `these ${del.entries.length} items`} on
      <strong>{del.pane.label}</strong>? Folders are deleted with everything inside. This can't be undone.
    </p>
    <ul class="targets">
      {#each del.entries.slice(0, 12) as e (e.path)}
        <li><code>{e.path}</code>{e.kind === "dir" ? " (folder)" : ""}</li>
      {/each}
      {#if del.entries.length > 12}<li class="muted">…and {del.entries.length - 12} more</li>{/if}
    </ul>
    {#if delNeedsTyping}
      <label class="field">
        <span>Type <strong class="verbatim">{del.pane.label}</strong> to confirm</span>
        <input class="input" bind:value={delTyped} autocomplete="off" spellcheck="false" />
      </label>
    {/if}
    {#if dlgError}<p class="error-text" role="alert" use:scramble={dlgError}></p>{/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (del = null)}>Cancel</button>
    <button type="button" class="btn danger" disabled={busy || !delOk} onclick={doDelete}>
      Delete {del?.entries.length === 1 ? del.entries[0].name : `${del?.entries.length ?? 0} items`}
    </button>
  {/snippet}
</Modal>

<Modal
  open={!!nameDlg}
  title={nameDlg?.mode === "mkdir" ? "NEW FOLDER" : "RENAME"}
  width={480}
  onclose={() => (nameDlg = null)}
>
  {#if nameDlg}
    <form id="name-form" onsubmit={doName}>
      <label class="field">
        <span>{nameDlg.mode === "mkdir" ? `In ${nameDlg.pane.label}:${nameDlg.pane.path}` : `Rename ${nameDlg.entry?.name}`}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" bind:value={nameValue} autocomplete="off" spellcheck="false" autofocus />
      </label>
      {#if dlgError}<p class="error-text" role="alert" use:scramble={dlgError}></p>{/if}
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (nameDlg = null)}>Cancel</button>
    <button type="submit" form="name-form" class="btn primary" disabled={busy || !nameValue.trim()}>
      {nameDlg?.mode === "mkdir" ? "Create" : "Rename"}
    </button>
  {/snippet}
</Modal>

<Modal open={closing !== null} title="UNSAVED CHANGES" width={480} onclose={() => (closing = null)}>
  <p>{files.docs.find((d) => d.id === closing)?.name} has unsaved changes. Close it anyway?</p>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (closing = null)}>Keep editing</button>
    <button
      type="button"
      class="btn danger"
      onclick={() => {
        if (closing !== null) files.closeDoc(closing);
        closing = null;
      }}>Discard changes</button
    >
  {/snippet}
</Modal>

<style>
  .files-view {
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
    max-width: 240px;
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
    padding: 0 10px;
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: var(--fs-sm);
    letter-spacing: 0.06em;
  }
  .tab.active .tab-btn {
    color: var(--text-0);
  }
  .tname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tw {
    font-size: 10px;
    color: var(--cyan);
    letter-spacing: 0.1em;
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--mint);
    flex: none;
  }
  .dot.dirty {
    background: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .tclose {
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    padding: 0 8px;
  }
  .tclose:hover {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .actions {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 0 2px 3px;
  }
  .act {
    height: 24px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-0);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 0 10px;
    cursor: pointer;
    white-space: nowrap;
  }
  .act:hover:not(:disabled) {
    border-color: var(--magenta);
    color: var(--magenta);
  }
  .act.danger:hover:not(:disabled) {
    border-color: var(--danger);
    color: var(--danger);
  }
  .act:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding-top: 8px;
  }
  .browser,
  .doc {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .hidden {
    display: none;
  }
  .browser > :global(.resizer.y) {
    margin: -8px 0;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
  }
  .targets {
    margin: 8px 0 12px;
    padding-left: 18px;
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-sm);
  }
  code {
    color: var(--danger);
  }
</style>
