<script lang="ts">
  import { api, errorText, type IntegrationInput, type IntegrationKind } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { emptyIntegrationInput, integrations, KIND_META, kindsByGroup, summary, toIntegrationInput } from "../../stores/integrations.svelte";
  import Modal from "../Modal.svelte";
  import Toggle from "../Toggle.svelte";

  const open = $derived(integrations.editing !== null);
  const existing = $derived(integrations.editing ? integrations.byId(integrations.editing) : undefined);

  let form = $state<IntegrationInput>(emptyIntegrationInput());
  let secret = $state("");
  let reveal = $state(false);
  let clearSecret = $state(false);
  let error = $state<string | null>(null);
  let tested = $state<string | null>(null);
  let busy = $state(false);
  let confirmDelete = $state(false);
  let kindFilter = $state("");
  const groups = $derived(kindsByGroup(kindFilter));

  $effect(() => {
    if (integrations.editing === null) return;
    const v = integrations.editing ? integrations.byId(integrations.editing) : undefined;
    form = v ? toIntegrationInput(v.integration) : emptyIntegrationInput();
    secret = "";
    reveal = false;
    clearSecret = false;
    error = null;
    tested = null;
    confirmDelete = false;
    kindFilter = "";
  });

  const kind = $derived(form.kind ?? "plex");
  const meta = $derived(KIND_META[kind]);
  const needsSecret = $derived(!!meta.secret);
  const secretLabel = $derived(meta.secret?.label ?? "Password");
  const urlOk = $derived(/^https?:\/\/[^/\s@?#]+/i.test(form.baseUrl?.trim() ?? ""));

  function pick(k: IntegrationKind) {
    form = { ...emptyIntegrationInput(k), baseUrl: form.baseUrl };
  }

  /** null keeps the saved secret, "" removes it. */
  const secretArg = (): string | null => (clearSecret ? "" : secret.trim() ? secret.trim() : null);

  function close() {
    integrations.editing = null;
  }

  async function showSecret() {
    if (reveal) {
      reveal = false;
      return;
    }
    if (existing?.hasSecret && !secret) {
      try {
        secret = (await api.integrationRevealSecret(existing.integration.id)) ?? "";
      } catch (e) {
        error = errorText(e);
      }
    }
    reveal = true;
  }

  async function test() {
    busy = true;
    error = null;
    tested = null;
    try {
      const s = await api.integrationTest(existing?.integration.id ?? null, form, secret.trim() || null);
      tested = `Connected: ${summary(s)}`;
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function save(e: Event) {
    e.preventDefault();
    if (!urlOk) return;
    busy = true;
    error = null;
    try {
      await integrations.save(integrations.editing ?? 0, form, secretArg());
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
      await integrations.remove(existing.integration.id);
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {open} title={existing ? `EDIT INTEGRATION :: ${existing.integration.name}` : "NEW INTEGRATION"} width={600} onclose={close}>
  {#if confirmDelete && existing}
    <p>Remove <strong>{existing.integration.name}</strong> ({existing.integration.baseUrl})?</p>
    <p class="muted">Its saved token or password is deleted from the keychain. Nothing changes in the app itself.</p>
  {:else}
    <form class="grid" id="integration-form" onsubmit={save}>
      {#if !existing}
        <div class="span6 picker">
          <input class="input filter" bind:value={kindFilter} placeholder="Find an app…" aria-label="Find an integration type" autocomplete="off" spellcheck="false" />
          <div class="groups" role="radiogroup" aria-label="Integration type">
            {#each groups as g (g.group)}
              <div class="group">
                <span class="glabel">{g.group}</span>
                <div class="kinds">
                  {#each g.kinds as k (k)}
                    <button type="button" role="radio" aria-checked={kind === k} class:on={kind === k} onclick={() => pick(k)}>{KIND_META[k].name}</button>
                  {/each}
                </div>
              </div>
            {:else}
              <p class="muted small">No integration matches "{kindFilter}".</p>
            {/each}
          </div>
        </div>
      {/if}
      <p class="span6 muted help"><strong>{meta.name}</strong>: {meta.help}</p>
      <label class="field span2">
        <span>Name</span>
        <input class="input" bind:value={form.name} placeholder={meta.name} autocomplete="off" />
      </label>
      <label class="field span4">
        <span>URL</span>
        <input class="input" bind:value={form.baseUrl} placeholder={meta.placeholder} required autocomplete="off" spellcheck="false" aria-invalid={!!form.baseUrl?.trim() && !urlOk} />
      </label>
      {#if meta.username}
        <label class="field span3">
          <span>{meta.username.label}</span>
          <input class="input" bind:value={form.username} autocomplete="off" spellcheck="false" required={meta.username.required} />
        </label>
      {/if}
      {#if kind === "kuma"}
        <label class="field span3">
          <span>Status page slug</span>
          <input class="input" bind:value={form.slug} placeholder="home" autocomplete="off" spellcheck="false" required />
          <span class="field-hint">From the page URL: <code>/status/<b>home</b></code></span>
        </label>
      {/if}
      {#if kind === "portainer"}
        <label class="field span3">
          <span>Environment id</span>
          <input class="input" type="number" min="1" bind:value={form.endpointId} />
          <span class="field-hint">The number after <code>#!/</code> in Portainer's URLs; 1 for the local one.</span>
        </label>
        <label class="field span3">
          <span>Docker host</span>
          <select class="input" value={form.hostId ?? ""} onchange={(e) => (form.hostId = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
            <option value="">(none)</option>
            {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
          </select>
        </label>
      {/if}
      {#if needsSecret}
        <div class="field {meta.username ? 'span3' : 'span6'}">
          <span>{secretLabel}</span>
          <div class="secret">
            <input
              class="input"
              type={reveal ? "text" : "password"}
              bind:value={secret}
              placeholder={existing?.hasSecret && !clearSecret ? "•••••••• saved in the keychain" : kind === "plex" ? "X-Plex-Token" : meta.secret?.optional ? "optional" : ""}
              autocomplete="off"
              spellcheck="false"
              disabled={clearSecret}
              aria-label={secretLabel}
            />
            <button type="button" class="btn ghost tiny" onclick={showSecret}>{reveal ? "Hide" : "Show"}</button>
            {#if existing?.hasSecret}
              <button type="button" class="btn ghost tiny" onclick={() => ((clearSecret = !clearSecret), (secret = ""))}>{clearSecret ? "Keep" : "Remove"}</button>
            {/if}
          </div>
          <span class="field-hint">{meta.secret?.hint ?? "Kept in the OS keychain only."}</span>
        </div>
      {/if}
      {#if /^https:/i.test(form.baseUrl ?? "")}
        <div class="span6">
          <Toggle label="Allow untrusted certificate" hint="Self-signed or internal CA." checked={!!form.allowUntrusted} onchange={(v) => (form.allowUntrusted = v)} />
        </div>
      {/if}
    </form>
  {/if}
  {#if tested}<p class="ok" role="status">{tested}</p>{/if}
  {#if error}<p class="error-text" role="alert">{error}</p>{/if}

  {#snippet footer()}
    {#if confirmDelete}
      <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      <button type="button" class="btn danger" disabled={busy} onclick={remove}>Remove {existing?.integration.name}</button>
    {:else}
      {#if existing}<button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Remove…</button>{/if}
      <button type="button" class="btn ghost" disabled={busy || !urlOk} onclick={test}>{busy ? "…" : "Test"}</button>
      <button type="button" class="btn ghost" onclick={close}>Cancel</button>
      <button type="submit" form="integration-form" class="btn primary" disabled={busy || !urlOk}>{existing ? "Save" : "Add"}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 12px;
  }
  .span2 {
    grid-column: span 2;
  }
  .span3 {
    grid-column: span 3;
  }
  .span4 {
    grid-column: span 4;
  }
  .span6 {
    grid-column: span 6;
  }
  .picker {
    display: grid;
    gap: 8px;
  }
  .groups {
    display: grid;
    gap: 6px;
    max-height: 210px;
    overflow: auto;
  }
  .group {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 8px;
    align-items: start;
  }
  .glabel {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    padding-top: 5px;
  }
  .small {
    font-size: var(--fs-sm);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .kinds button {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 8px;
    cursor: pointer;
  }
  .kinds button.on {
    color: var(--magenta);
    border-color: var(--magenta);
  }
  .help {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .secret {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .secret .input {
    flex: 1;
    min-width: 0;
  }
  code {
    color: var(--cyan);
  }
  p {
    margin: 0 0 8px;
  }
  .ok {
    color: var(--ok);
    margin-top: 12px;
  }
  .error-text {
    margin: 12px 0 0;
  }
  :global(footer) .left {
    margin-right: auto;
  }
  .tiny {
    min-height: 22px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
</style>
