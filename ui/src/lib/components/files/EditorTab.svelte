<script lang="ts">
  import { formatSize } from "../../files/util";
  import type { EditorDoc } from "../../stores/files.svelte";
  import Modal from "../Modal.svelte";

  interface Props {
    doc: EditorDoc;
  }
  let { doc }: Props = $props();

  let host: HTMLDivElement | undefined = $state();
  let confirmOverwrite = $state(false);

  // Create the CodeMirror view once the file has loaded. CodeMirror is
  // loaded on first use so it stays out of the main bundle.
  let creating = false;
  $effect(() => {
    if (!host || doc.loading || doc.editor || doc.error || creating) return;
    const el = host;
    creating = true;
    void import("../../files/editor").then(({ createEditor }) => {
      creating = false;
      if (doc.editor) return;
      doc.editor = createEditor(el, doc.content, doc.lang, {
        onsave: () => void doc.save(),
        onchange: () => (doc.dirty = doc.editor?.text() !== doc.content),
      });
      doc.editor.view.focus();
    });
  });

  const savedNote = $derived(
    doc.lastSave
      ? doc.lastSave.atomic
        ? "Saved (atomic: temp file + rename, mode and owner kept)"
        : "Saved in place (folder not writable or owner could not be kept)"
      : null,
  );
</script>

<div class="editor-tab">
  <div class="meta">
    <span class="where">{doc.where}</span>
    <code class="path">{doc.path}</code>
    {#if doc.stamp}<span class="muted">{formatSize(doc.stamp.size)}</span>{/if}
    <span class="lang">{doc.lang.toUpperCase()}</span>
    <span class="spacer"></span>
    {#if doc.dirty}<span class="dirty">● MODIFIED</span>{:else if savedNote}<span class="ok" title={savedNote}>✓ SAVED</span>{/if}
    <button type="button" class="btn ghost tiny" disabled={doc.saving} onclick={() => doc.load()} title="Reload from disk (discards changes)"
      >Reload</button
    >
    <button type="button" class="btn primary tiny" disabled={doc.saving || !doc.dirty} onclick={() => doc.save()}>
      {doc.saving ? "Saving…" : "Save"}
    </button>
  </div>
  {#if doc.conflict}
    <div class="banner warn" role="alert">
      <span><strong>{doc.path}</strong> changed on {doc.where} since you opened it.</span>
      <button type="button" class="btn ghost tiny" onclick={() => doc.load()}>Reload theirs</button>
      <button type="button" class="btn danger tiny" onclick={() => (confirmOverwrite = true)}>Overwrite…</button>
    </div>
  {/if}
  {#if doc.error}
    <div class="banner err" role="alert"><span>{doc.error}</span></div>
  {/if}
  {#if doc.loading && !doc.editor}
    <p class="muted pad">Loading {doc.name}…</p>
  {/if}
  <div class="cm" bind:this={host}></div>
  {#if doc.stamp && savedNote}
    <div class="status muted">{savedNote}</div>
  {/if}
</div>

<Modal open={confirmOverwrite} title="OVERWRITE CHANGED FILE" tone="danger" width={520} onclose={() => (confirmOverwrite = false)}>
  <p>
    Replace <code>{doc.where}:{doc.path}</code> with your version? The changes made on the server since you opened it will be lost.
  </p>
  <p class="muted">The file's permissions and owner are kept.</p>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (confirmOverwrite = false)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      onclick={() => {
        confirmOverwrite = false;
        void doc.save(true);
      }}>Overwrite {doc.name}</button
    >
  {/snippet}
</Modal>

<style>
  .editor-tab {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--line);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
    white-space: nowrap;
  }
  .where {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--cyan);
  }
  .path {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .lang {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .spacer {
    flex: 1;
  }
  .dirty {
    color: var(--magenta);
    letter-spacing: 0.1em;
  }
  .ok {
    color: var(--mint);
    letter-spacing: 0.1em;
  }
  .tiny {
    padding: 2px 10px;
    font-size: var(--fs-xs);
  }
  .banner {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    font-size: var(--fs-sm);
    border-bottom: 1px solid var(--line);
  }
  .banner.warn {
    background: color-mix(in srgb, var(--warn) 12%, transparent);
    border-color: var(--warn);
  }
  .banner.err {
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    color: var(--danger);
  }
  .pad {
    padding: 12px;
  }
  .cm {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .status {
    padding: 3px 8px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  code {
    color: var(--cyan);
  }
</style>
