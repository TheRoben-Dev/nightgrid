<script lang="ts">
  import { untrack } from "svelte";
  import { errorText, type ServiceInput } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { emptyInput, services, toInput } from "../../stores/services.svelte";
  import Modal from "../Modal.svelte";
  import Toggle from "../Toggle.svelte";

  const open = $derived(services.editing !== null);
  const existing = $derived(services.editing ? services.byId(services.editing) : undefined);

  let form = $state<ServiceInput>(emptyInput());
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmDelete = $state(false);

  $effect(() => {
    if (services.editing === null) return;
    const s = services.editing ? services.byId(services.editing) : undefined;
    const draft = untrack(() => services.draft);
    form = s ? toInput(s) : { ...emptyInput(), ...(draft ?? {}) };
    untrack(() => (services.draft = null));
    error = null;
    confirmDelete = false;
  });

  const urlOk = $derived(/^https?:\/\/[^/\s@?#]+/i.test(form.url?.trim() ?? ""));
  const isHttps = $derived(/^https:/i.test(form.url?.trim() ?? ""));

  function close() {
    services.editing = null;
  }

  async function save(e: Event) {
    e.preventDefault();
    if (!urlOk || !form.name?.trim()) return;
    busy = true;
    error = null;
    try {
      await services.save(services.editing ?? 0, form);
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
      await services.remove(existing.id);
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {open} title={existing ? `EDIT SERVICE :: ${existing.name}` : "NEW SERVICE"} width={600} onclose={close}>
  {#if confirmDelete && existing}
    <div class="confirm">
      <p>Remove service <strong>{existing.name}</strong> ({existing.url}) from the board?</p>
      <p class="muted">
        Its check history is deleted too. Nothing changes on the service itself{existing.source === "homepage"
          ? ", and a later Homepage re-sync adds it back if it's still in services.yaml"
          : ""}.
      </p>
    </div>
  {:else}
    <form class="grid" id="service-form" onsubmit={save}>
      <label class="field span3">
        <span>Name</span>
        <input class="input" bind:value={form.name} placeholder="Media server" required autocomplete="off" />
      </label>
      <label class="field span3">
        <span>Group</span>
        <input class="input" bind:value={form.group} list="service-groups" placeholder="Media" autocomplete="off" />
        <datalist id="service-groups">
          {#each services.groupNames as g (g)}<option value={g}></option>{/each}
        </datalist>
      </label>
      <label class="field span6">
        <span>URL</span>
        <input
          class="input"
          bind:value={form.url}
          placeholder="https://service.example.lan:8443/"
          required
          autocomplete="off"
          aria-invalid={!!form.url?.trim() && !urlOk}
        />
        <span class="field-hint">Opened in your browser from the card, and checked with a GET request. Redirects aren't followed.</span>
      </label>
      <label class="field span6">
        <span>Description</span>
        <input class="input" bind:value={form.description} placeholder="(optional)" autocomplete="off" />
      </label>
      <label class="field span3">
        <span>Runs on</span>
        <select
          class="input"
          value={form.hostId ?? ""}
          onchange={(e) => (form.hostId = e.currentTarget.value ? Number(e.currentTarget.value) : null)}
        >
          <option value="">(no host)</option>
          {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
        </select>
      </label>
      <label class="field span3">
        <span>Healthy status codes</span>
        <input class="input" bind:value={form.expectStatus} placeholder="200-399" autocomplete="off" />
        <span class="field-hint">Codes or ranges, e.g. <code>200-399,401</code> or <code>2xx</code>.</span>
      </label>
      <div class="span6 toggles">
        <Toggle
          label="Health checks"
          hint="Check this service in the background."
          checked={!!form.checkEnabled}
          onchange={(v) => (form.checkEnabled = v)}
        />
        {#if isHttps}
          <Toggle
            label="Allow untrusted certificate"
            hint="Self-signed or internal CA: shown as a warning instead of marking the service down."
            checked={!!form.allowUntrusted}
            onchange={(v) => (form.allowUntrusted = v)}
          />
        {/if}
      </div>
    </form>
  {/if}
  {#if error}<p class="error-text" role="alert">{error}</p>{/if}

  {#snippet footer()}
    {#if confirmDelete}
      <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      <button type="button" class="btn danger" disabled={busy} onclick={remove}>Remove {existing?.name}</button>
    {:else}
      {#if existing}
        <button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Remove…</button>
      {/if}
      <button type="button" class="btn ghost" onclick={close}>Cancel</button>
      <button type="submit" form="service-form" class="btn primary" disabled={busy || !urlOk || !form.name?.trim()}>
        {existing ? "Save" : "Add service"}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 12px;
  }
  .span3 {
    grid-column: span 3;
  }
  .span6 {
    grid-column: span 6;
  }
  .toggles {
    display: grid;
    gap: 6px;
  }
  code {
    color: var(--cyan);
  }
  .confirm p {
    margin: 0 0 10px;
    line-height: 1.5;
  }
  .error-text {
    margin: 12px 0 0;
  }
  :global(footer) .left {
    margin-right: auto;
  }
</style>
