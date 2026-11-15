<script lang="ts">
  import { errorText, type SnippetInput } from "../../api";
  import { automation, emptySnippet, toSnippetInput } from "../../stores/automation.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import Modal from "../Modal.svelte";

  const open = $derived(automation.editingSnippet !== null);
  const existing = $derived(automation.editingSnippet ? automation.snippets.find((s) => s.id === automation.editingSnippet) : undefined);

  let form = $state<SnippetInput>(emptySnippet());
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmDelete = $state(false);

  $effect(() => {
    if (automation.editingSnippet === null) return;
    const s = automation.editingSnippet ? automation.snippets.find((x) => x.id === automation.editingSnippet) : undefined;
    form = s ? toSnippetInput(s) : (automation.snippetDraft ?? emptySnippet());
    error = null;
    confirmDelete = false;
  });

  type ScopeKind = SnippetInput["scope"]["kind"];
  function setScope(kind: ScopeKind) {
    if (kind === "global") form.scope = { kind };
    else if (kind === "host") form.scope = { kind, hostId: hosts.hosts[0]?.id ?? 0 };
    else form.scope = { kind, groupId: hosts.groups[0]?.id ?? 0 };
  }

  const vars = $derived([...form.command.matchAll(/\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:=[^}\n]*)?\}\}/g)].map((m) => m[1]));
  const asked = $derived([...new Set(vars.filter((v) => !["host", "hostname", "user"].includes(v)))]);

  function close() {
    automation.editingSnippet = null;
    automation.snippetDraft = null;
  }

  async function save(e: Event) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      const s = await automation.saveSnippet(automation.editingSnippet ?? 0, $state.snapshot(form));
      automation.pick({ kind: "snippet", id: s.id });
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!existing) return;
    busy = true;
    try {
      await automation.deleteSnippet(existing.id);
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {open} title={existing ? "EDIT SNIPPET" : "NEW SNIPPET"} width={600} onclose={close}>
  <form id="snippet-form" class="grid" onsubmit={save}>
    <label class="field">
      <span>NAME</span>
      <input class="input" bind:value={form.name} placeholder="Tail container logs" maxlength="80" />
    </label>
    <label class="field">
      <span>COMMAND</span>
      <textarea class="input mono" rows="4" bind:value={form.command} spellcheck="false" placeholder={"docker logs --tail {{lines=100}} {{container}}"}></textarea>
      <small class="muted">
        <code>{"{{name}}"}</code> asks for a value when it runs, <code>{"{{name=default}}"}</code> pre-fills it.
        <code>{"{{host}}"}</code>, <code>{"{{hostname}}"}</code> and <code>{"{{user}}"}</code> are filled per host. Values are
        shell-quoted, so don't put quotes around placeholders.
        {#if asked.length}<br />Asks for: <strong>{asked.join(", ")}</strong>{/if}
      </small>
    </label>
    <label class="field">
      <span>DESCRIPTION · OPTIONAL</span>
      <input class="input" bind:value={form.description} />
    </label>
    <div class="field">
      <span>SHOW FOR</span>
      <div class="seg" role="radiogroup" aria-label="Scope">
        {#each [["global", "All hosts"], ["host", "One host"], ["group", "A group"]] as [k, l] (k)}
          <button type="button" role="radio" aria-checked={form.scope.kind === k} class:on={form.scope.kind === k} disabled={(k === "host" && !hosts.hosts.length) || (k === "group" && !hosts.groups.length)} onclick={() => setScope(k as ScopeKind)}>{l}</button>
        {/each}
      </div>
      {#if form.scope.kind === "host"}
        {@const sc = form.scope}
        <select class="input" value={sc.hostId} onchange={(e) => (form.scope = { kind: "host", hostId: Number(e.currentTarget.value) })} aria-label="Host">
          {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
        </select>
      {:else if form.scope.kind === "group"}
        {@const sc = form.scope}
        <select class="input" value={sc.groupId} onchange={(e) => (form.scope = { kind: "group", groupId: Number(e.currentTarget.value) })} aria-label="Group">
          {#each hosts.groups as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
        </select>
      {/if}
      <small class="muted">Where it's offered in the palette. It can still run on any host from here.</small>
    </div>
    <label class="check"><input type="checkbox" bind:checked={form.sudo} /> Run as root (sudo)</label>
    {#if error}<p class="error-text" role="alert">{error}</p>{/if}
  </form>
  {#snippet footer()}
    {#if existing}
      {#if confirmDelete}
        <span class="del">Delete “{existing.name}”?</span>
        <button type="button" class="btn danger" disabled={busy} onclick={remove}>Delete</button>
        <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      {:else}
        <button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Delete…</button>
      {/if}
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={close}>Cancel</button>
    <button type="submit" form="snippet-form" class="btn primary" disabled={busy || !form.name.trim() || !form.command.trim()}>Save</button>
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    gap: 12px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  textarea.input {
    resize: vertical;
    width: 100%;
  }
  small {
    font-size: var(--fs-xs);
    line-height: 1.5;
  }
  .seg {
    display: flex;
    gap: 0;
    margin-bottom: 6px;
  }
  .seg button {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    padding: 4px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .seg button + button {
    border-left: 0;
  }
  .seg button.on {
    color: var(--text-0);
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 12%, transparent);
  }
  .seg button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  input[type="checkbox"] {
    accent-color: var(--magenta);
  }
  .spacer {
    flex: 1;
  }
  .del {
    font-size: var(--fs-sm);
    color: var(--danger);
    align-self: center;
  }
</style>
