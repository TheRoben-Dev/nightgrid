<script lang="ts">
  import { formatSize } from "../../files/util";
  import type { EditorDoc } from "../../stores/files.svelte";
  import { MAX_SELECTION } from "../../helper/prompt";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import Modal from "../Modal.svelte";

  interface Props {
    doc: EditorDoc;
    /** Close the tab and go back to the terminal that opened it (optionally saving first). */
    onreturn?: (save: boolean) => void;
  }
  let { doc, onreturn }: Props = $props();

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

  /** The selected part of the file, or the file from the top (cut to what DRIFT takes). */
  function explainText(): { text: string; source: string } {
    const view = doc.editor?.view;
    const sel = view?.state.selection.main;
    const picked = view && sel && !sel.empty ? view.state.sliceDoc(sel.from, sel.to) : "";
    const text = picked || (doc.editor?.text() ?? doc.content);
    const room = MAX_SELECTION - 40;
    const cut = text.length > room;
    return {
      text: cut ? `${text.slice(0, room)}\n[rest cut]` : text,
      source: `file ${doc.path}${picked ? ", selected lines" : cut ? ", first part" : ""}`,
    };
  }

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
    {#if !doc.loading && !doc.error}
      <ExplainButton
        label="Ask DRIFT"
        source={`file ${doc.path}`}
        text={explainText}
        host={doc.where}
        question="Explain this file: what it does, and anything that looks wrong or risky."
      />
    {/if}
    <button type="button" class="btn ghost tiny" disabled={doc.saving} onclick={() => doc.load()} title="Reload from disk (discards changes)"
      >Reload</button
    >
    <button type="button" class="btn primary tiny" disabled={doc.saving || !doc.dirty} onclick={() => doc.save()}>
      {doc.saving ? "Saving…" : "Save"}
    </button>
  </div>
  {#if doc.returnTo}
    <div class="banner from-term" role="status">
      <span
        >Opened from the terminal on <strong>{doc.returnTo.label}</strong>{doc.isNew ? " (new file, saving creates it)" : ""}. Closing this
        tab lets the terminal continue.</span
      >
      <span class="spacer"></span>
      <button type="button" class="btn ghost tiny" onclick={() => onreturn?.(false)}>Close without saving</button>
      <button type="button" class="btn primary tiny" disabled={doc.saving} onclick={() => onreturn?.(true)}>Save &amp; return</button>
    </div>
  {/if}
  {#if doc.isNew && !doc.returnTo}
    <div class="banner from-term" role="status"><span><strong>{doc.name}</strong> doesn't exist yet; saving creates it.</span></div>
  {/if}
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
  .banner.from-term {
    background: color-mix(in srgb, var(--cyan) 10%, transparent);
    border-color: color-mix(in srgb, var(--cyan) 50%, var(--line));
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
