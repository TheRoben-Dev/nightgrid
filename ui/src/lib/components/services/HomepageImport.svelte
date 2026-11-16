<script lang="ts">
  import {
    api,
    errorText,
    filesError,
    type HomepageCandidate,
    type HomepagePreview,
    type ImportSummary,
    type Side,
  } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { services } from "../../stores/services.svelte";
  import Modal from "../Modal.svelte";
  import Toggle from "../Toggle.svelte";

  const PREFS_KEY = "nightgrid.homepage.v1";

  /** "" = this computer, else a host id. */
  let source = $state<string>("");
  let dir = $state("");
  let candidates = $state<HomepageCandidate[]>([]);
  let detecting = $state(false);
  let preview = $state<HomepagePreview | null>(null);
  let summary = $state<ImportSummary | null>(null);
  let linkHost = $state(true);
  let prune = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const side = $derived<Side>(source ? { kind: "host", id: Number(source) } : { kind: "local" });
  const hostName = $derived(source ? (hosts.byId(Number(source))?.name ?? "host") : "this computer");

  // Remember the last source between imports (paths only, no secrets).
  $effect(() => {
    if (!services.importOpen) return;
    preview = null;
    summary = null;
    error = null;
    candidates = [];
    try {
      const p = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
      if (typeof p.dir === "string") dir = p.dir;
      if (typeof p.source === "string" && (p.source === "" || hosts.byId(Number(p.source)))) source = p.source;
      else if (!source && hosts.preferred("homepage")) source = String(hosts.preferred("homepage")!.id);
    } catch {
      /* ignore */
    }
  });

  function remember() {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify({ source, dir }));
    } catch {
      /* storage unavailable */
    }
  }

  function describe(e: unknown): string {
    const { connect } = filesError(e);
    if (connect && (connect.kind === "passwordRequired" || connect.kind === "passphraseRequired" || connect.kind.startsWith("hostKey"))) {
      return `${hostName} needs a login first. Open a terminal or Files on it once, then try again.`;
    }
    return errorText(e);
  }

  async function detect() {
    if (!source) return;
    detecting = true;
    error = null;
    try {
      candidates = await api.homepageDetect(Number(source));
      if (candidates.length && !dir.trim()) dir = candidates[0].dir;
      if (!candidates.length) error = `No Homepage container with a mounted /app/config found on ${hostName}.`;
    } catch (e) {
      error = describe(e);
    } finally {
      detecting = false;
    }
  }

  async function load(e?: Event) {
    e?.preventDefault();
    if (!dir.trim()) return;
    busy = true;
    error = null;
    summary = null;
    try {
      preview = await api.homepagePreview(side, dir.trim());
      remember();
    } catch (err) {
      preview = null;
      error = describe(err);
    } finally {
      busy = false;
    }
  }

  async function runImport() {
    busy = true;
    error = null;
    try {
      summary = await api.homepageImport(side, dir.trim(), linkHost && !!source, prune);
      await services.refresh();
      void services.loadHistory();
    } catch (err) {
      error = describe(err);
    } finally {
      busy = false;
    }
  }

  const groups = $derived.by(() => {
    if (!preview) return [];
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- local, rebuilt on each read
    const by = new Map<string, string[]>();
    for (const e of preview.parse.entries) by.set(e.group, [...(by.get(e.group) ?? []), e.name]);
    return [...by.entries()];
  });

  function close() {
    services.importOpen = false;
  }
</script>

<Modal open={services.importOpen} title="IMPORT :: HOMEPAGE" width={640} onclose={close}>
  {#if summary}
    <div class="summary" role="status">
      <p class="label ok">IMPORT COMPLETE</p>
      <p>
        {summary.added} added · {summary.updated} updated · {summary.unchanged} unchanged{summary.removed
          ? ` · ${summary.removed} removed`
          : ""}
      </p>
      {#if summary.stale.length}
        <p class="muted">
          No longer in services.yaml (kept, tick "remove" to drop them next time): {summary.stale.join(", ")}
        </p>
      {/if}
      <p class="muted">Run the import again any time to pick up changes. Your check settings are kept.</p>
    </div>
  {:else}
    <form class="grid" id="hp-form" onsubmit={load}>
      <label class="field">
        <span>Read from</span>
        <select class="input" bind:value={source} onchange={() => ((preview = null), (candidates = []))}>
          {#each hosts.hosts as h (h.id)}<option value={String(h.id)}>{h.name}</option>{/each}
          <option value="">This computer</option>
        </select>
      </label>
      <label class="field grow">
        <span>Config folder or services.yaml</span>
        <div class="row">
          <input class="input" bind:value={dir} placeholder="/opt/homepage/config" autocomplete="off" />
          {#if source}
            <button type="button" class="btn" disabled={detecting} onclick={detect} title="Look for a Homepage container's /app/config mount">
              {detecting ? "Looking…" : "Detect"}
            </button>
          {/if}
        </div>
        {#if candidates.length > 1}
          <div class="cands">
            {#each candidates as c (c.dir)}
              <button type="button" class="btn ghost tiny" class:on={dir === c.dir} onclick={() => (dir = c.dir)}>{c.container}: {c.dir}</button>
            {/each}
          </div>
        {/if}
        <span class="field-hint">Reads services.yaml, plus settings.yaml for group order (over SFTP for a host). Nothing is changed there.</span>
      </label>
    </form>

    {#if preview}
      <div class="preview">
        <p class="label">
          {preview.parse.entries.length} SERVICES IN {groups.length} GROUPS
          {#if preview.layout}<span class="muted">· ordered by settings.yaml</span>{/if}
        </p>
        <ul class="groups">
          {#each groups as [g, names] (g)}
            <li><span class="g">{g}</span> <span class="n">{names.join(" · ")}</span></li>
          {/each}
        </ul>
        {#if preview.parse.skipped.length}
          <details>
            <summary>{preview.parse.skipped.length} skipped</summary>
            <ul class="skipped">
              {#each preview.parse.skipped as s, i (i)}<li>{s}</li>{/each}
            </ul>
          </details>
        {/if}
        <div class="opts">
          {#if source}
            <Toggle label="Link to {hostName}" hint="New services are tied to this host." checked={linkHost} onchange={(v) => (linkHost = v)} />
          {/if}
          <Toggle
            label="Remove services no longer in the file"
            hint="Only ones imported from Homepage earlier; manual services are never touched."
            checked={prune}
            onchange={(v) => (prune = v)}
          />
        </div>
      </div>
    {/if}
  {/if}
  {#if error}<p class="error-text" role="alert">{error}</p>{/if}

  {#snippet footer()}
    {#if summary}
      <button type="button" class="btn primary" onclick={close}>Done</button>
    {:else}
      <button type="button" class="btn ghost" onclick={close}>Cancel</button>
      {#if preview}
        <button type="button" class="btn" disabled={busy} onclick={() => load()}>Reload</button>
        <button type="button" class="btn primary" disabled={busy || !preview.parse.entries.length} onclick={runImport}>
          Import {preview.parse.entries.length}
        </button>
      {:else}
        <button type="submit" form="hp-form" class="btn primary" disabled={busy || !dir.trim()}>{busy ? "Reading…" : "Preview"}</button>
      {/if}
    {/if}
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 12px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row .input {
    flex: 1;
    min-width: 0;
  }
  .cands {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .cands .on {
    color: var(--magenta);
    border-color: var(--magenta);
  }
  .tiny {
    min-height: 20px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
  .preview {
    margin-top: 14px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .preview > .label {
    margin: 0 0 8px;
  }
  .groups,
  .skipped {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    max-height: 180px;
    overflow: auto;
    font-size: var(--fs-sm);
  }
  .g {
    color: var(--magenta);
  }
  .n {
    color: var(--text-1);
  }
  details {
    margin-top: 8px;
    font-size: var(--fs-sm);
    color: var(--warn);
  }
  .skipped {
    color: var(--text-1);
    margin-top: 4px;
  }
  .opts {
    display: grid;
    gap: 6px;
    margin-top: 12px;
  }
  .summary p {
    margin: 0 0 8px;
  }
  .ok {
    color: var(--mint);
  }
  .error-text {
    margin: 12px 0 0;
  }
</style>
