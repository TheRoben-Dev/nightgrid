<script lang="ts">
  /**
   * The .env vault (M10w): a stack's variables, encrypted in Nightgrid and
   * masked until revealed (rule 6). Deploying shows which names change, then
   * writes the host's .env (mode 600) after a stack snapshot. Removals are
   * confirmed naming the variable; the deploy names the stack and host (rule 5).
   */
  import { scramble } from "../../scramble";
  import { diffEmpty, diffSummary, looksSecret, MASK, splitPaste, validName } from "../../envvault";
  import { docker } from "../../stores/docker.svelte";
  import { envVault } from "../../stores/envvault.svelte";
  import { envFile, stacks } from "../../stores/stacks.svelte";
  import type { EnvVar } from "../../api";
  import Modal from "../Modal.svelte";

  const s = $derived(envVault.stack);
  const hostName = $derived(docker.host?.name ?? "");
  const busy = $derived(envVault.busy);

  // Add / edit form. `editing` is the variable's old name when changing one.
  let editing = $state<string | null>(null);
  let name = $state("");
  let value = $state("");
  let secret = $state(true);
  let secretTouched = false;
  /** Editing keeps the stored value unless a new one is typed. */
  let keepValue = $state(false);
  let formError = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let confirmDelete = $state<EnvVar | null>(null);
  let confirmImport = $state(false);

  function resetForm() {
    editing = null;
    name = "";
    value = "";
    secret = true;
    secretTouched = false;
    keepValue = false;
    formError = null;
  }

  function startEdit(v: EnvVar) {
    editing = v.name;
    name = v.name;
    secret = v.secret;
    secretTouched = true;
    keepValue = true;
    value = "";
    formError = null;
  }

  function onName() {
    const p = splitPaste(name);
    if (p && name.includes("=")) {
      name = p.name;
      value = p.value;
      keepValue = false;
    }
    if (!secretTouched && !editing) secret = looksSecret(name);
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const n = name.trim();
    if (!validName(n)) {
      formError = "Use letters, digits and _, not starting with a digit.";
      return;
    }
    if (n !== editing && envVault.vars.some((v) => v.name === n)) {
      formError = `${n} is already in this vault: use Edit to change it.`;
      return;
    }
    const err = await envVault.save(editing, n, keepValue ? null : value, secret);
    if (err) formError = err;
    else {
      notice = editing ? `${n} saved.` : `${n} added.`;
      resetForm();
    }
  }

  async function doImport() {
    confirmImport = false;
    const n = await envVault.importHost();
    if (n !== null) notice = `Imported ${n} variable${n === 1 ? "" : "s"} from the host.`;
  }

  async function doDelete() {
    const v = confirmDelete;
    confirmDelete = null;
    if (!v) return;
    await envVault.remove(v.id);
    notice = `${v.name} removed from the vault.`;
    if (editing === v.name) resetForm();
  }

  function close() {
    resetForm();
    notice = null;
    confirmDelete = null;
    confirmImport = false;
    envVault.close();
  }

  function upNow() {
    const st = s;
    if (!st) return;
    close();
    stacks.request(st, "up");
  }

  const when = (ts: number | null) => (ts ? new Date(ts * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "");
</script>

<Modal open={!!s} title={s ? `.ENV VAULT · ${s.name}` : ""} width={760} onclose={close}>
  {#if s}
    <p class="muted small">
      Variables for <code>{envFile(s)}</code> on {hostName}, kept encrypted in Nightgrid with a key in this computer's keychain. Masked ones stay hidden
      until you reveal them. Nothing reaches the host until you deploy.
    </p>
    {#if envVault.error}<p class="error-text" role="alert" use:scramble={envVault.error}></p>{/if}
    {#if notice}<p class="notice small" role="status">{notice}</p>{/if}

    {#if confirmDelete}
      <div class="confirm" role="alertdialog" aria-label="Confirm">
        <p>Remove <strong>{confirmDelete.name}</strong> from the {s.name} vault? The host's .env keeps it until the next deploy.</p>
        <button type="button" class="btn ghost tiny" onclick={() => (confirmDelete = null)}>Keep</button>
        <button type="button" class="btn danger tiny" disabled={busy} onclick={() => void doDelete()}>Remove {confirmDelete.name}</button>
      </div>
    {/if}
    {#if confirmImport}
      <div class="confirm" role="alertdialog" aria-label="Confirm">
        <p>Read <code>{envFile(s)}</code> from {hostName} into the vault? Its values replace the vault's for the same names; other vault variables stay.</p>
        <button type="button" class="btn ghost tiny" onclick={() => (confirmImport = false)}>Cancel</button>
        <button type="button" class="btn primary tiny" disabled={busy} onclick={() => void doImport()}>Import</button>
      </div>
    {/if}

    <div class="vars" role="table" aria-label="Variables">
      {#each envVault.vars as v, i (v.id)}
        {@const shown = envVault.revealed[v.id]}
        <div class="var" role="row" class:editing={editing === v.name}>
          <span class="nm" role="cell" title={v.updatedAt ? `Changed ${when(v.updatedAt)}` : ""}>{v.name}</span>
          <span class="val" role="cell">
            {#if v.locked}
              <span class="tag warn" title="Written with vault key {v.keyId}, which isn't in this computer's keychain. Import the profile that has it.">LOCKED</span>
            {:else if shown !== undefined}
              <code class="data" class:empty={shown === ""}>{shown === "" ? "(empty)" : shown}</code>
            {:else}
              <span class="mask" aria-label="hidden">{MASK}</span>
            {/if}
          </span>
          <span class="acts" role="cell">
            {#if v.secret && !v.locked}
              {#if shown !== undefined}
                <button type="button" class="btn ghost tiny" onclick={() => envVault.hide(v.id)}>Hide</button>
              {:else}
                <button type="button" class="btn ghost tiny" onclick={() => void envVault.reveal(v.id)}>Reveal</button>
              {/if}
            {/if}
            <button type="button" class="btn ghost tiny icon" aria-label="Move {v.name} up" disabled={i === 0} onclick={() => void envVault.move(v.id, -1)}>↑</button>
            <button type="button" class="btn ghost tiny icon" aria-label="Move {v.name} down" disabled={i === envVault.vars.length - 1} onclick={() => void envVault.move(v.id, 1)}>↓</button>
            <button type="button" class="btn ghost tiny" disabled={v.locked} onclick={() => startEdit(v)}>Edit</button>
            <button type="button" class="btn ghost tiny dangertext" onclick={() => (confirmDelete = v)}>Delete…</button>
          </span>
        </div>
      {:else}
        <p class="muted small empty-list">
          {envVault.loading ? "Loading…" : "No variables yet. Add them below, or import the host's current .env."}
        </p>
      {/each}
    </div>

    <form class="add" onsubmit={submit} data-guide="env-vault-add">
      <input class="input mono" placeholder="NAME (or paste NAME=value)" aria-label="Name" bind:value={name} oninput={onName} spellcheck="false" autocomplete="off" />
      {#if keepValue}
        <button type="button" class="btn ghost tiny keep" title="The stored value is kept" onclick={() => (keepValue = false)}>Keep value · change…</button>
      {:else}
        <input
          class="input mono"
          type={secret ? "password" : "text"}
          placeholder="value"
          aria-label="Value"
          bind:value
          spellcheck="false"
          autocomplete="off"
        />
      {/if}
      <label class="check" title="Masked in the list until revealed">
        <input type="checkbox" bind:checked={secret} onchange={() => (secretTouched = true)} />masked
      </label>
      <button type="submit" class="btn primary tiny" disabled={busy || !name.trim()}>{editing ? "Save" : "Add"}</button>
      {#if editing}<button type="button" class="btn ghost tiny" onclick={resetForm}>Cancel</button>{/if}
    </form>
    {#if formError}<p class="error-text small">{formError}</p>{/if}

    {#if envVault.preview}
      {@const d = envVault.preview}
      <section class="deploy" aria-label="Deploy preview">
        <h4>DEPLOY TO {hostName.toUpperCase()}</h4>
        <p class="small">
          {d.exists ? "" : "The host has no .env yet; one is created. "}{diffSummary(d)}. A stack snapshot is saved first, so Snapshots can put the old file back.
        </p>
        {#if d.added.length}<p class="small"><span class="tag ok">ADDED</span> {d.added.join(", ")}</p>{/if}
        {#if d.changed.length}<p class="small"><span class="tag">CHANGED</span> {d.changed.join(", ")}</p>{/if}
        {#if d.removed.length}
          <p class="small warn-text"><span class="tag warn">REMOVED</span> {d.removed.join(", ")}: on the host but not in the vault, so deploying drops {d.removed.length === 1 ? "it" : "them"}. Import first to keep {d.removed.length === 1 ? "it" : "them"}.</p>
        {/if}
        <div class="row">
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" onclick={() => (envVault.preview = null)}>Cancel</button>
          <button type="button" class="btn danger tiny" disabled={busy || diffEmpty(d)} onclick={() => void envVault.deploy()}
            >{diffEmpty(d) ? "Already up to date" : `Write .env to ${hostName}`}</button
          >
        </div>
      </section>
    {/if}
    {#if envVault.deployed}
      {@const r = envVault.deployed}
      <section class="deploy ok" role="status">
        <p class="small">
          Wrote <code>{r.path}</code> (mode 600).
          {r.snapshot ? "A snapshot was saved first." : `No snapshot: ${r.snapshotError ?? "unknown error"}.`}
          Running containers keep the old values until the stack is brought up again.
        </p>
        <div class="row">
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" onclick={() => (envVault.deployed = null)}>Later</button>
          <button type="button" class="btn primary tiny" disabled={stacks.busy} onclick={upNow}>Up {s.name} now</button>
        </div>
      </section>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" disabled={busy} onclick={() => (confirmImport = true)}>Import from host .env</button>
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={close}>Close</button>
    <button
      type="button"
      class="btn primary"
      data-guide="env-vault-deploy"
      disabled={busy || envVault.previewing || !envVault.vars.length || envVault.vars.some((v) => v.locked)}
      title={envVault.vars.some((v) => v.locked) ? "Some variables are locked: their key isn't on this computer" : ""}
      onclick={() => void envVault.loadPreview()}>{envVault.previewing ? "Comparing…" : "Deploy .env…"}</button
    >
  {/snippet}
</Modal>

<style>
  .notice {
    color: var(--ok);
    margin: 0 0 6px;
  }
  .confirm,
  .row,
  .add {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .confirm {
    border: 1px solid var(--danger);
    padding: 6px 8px;
    font-size: var(--fs-sm);
    margin-bottom: 8px;
  }
  .confirm p {
    margin: 0;
    flex: 1 1 300px;
  }
  .vars {
    border: 1px solid var(--line);
    max-height: 320px;
    overflow: auto;
    margin-bottom: 8px;
  }
  .var {
    display: grid;
    grid-template-columns: minmax(120px, 220px) 1fr auto;
    gap: 8px;
    align-items: center;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .var:last-child {
    border-bottom: 0;
  }
  .var.editing {
    background: color-mix(in srgb, var(--cyan) 8%, transparent);
  }
  .nm {
    font-family: var(--font-mono);
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .val {
    min-width: 0;
    overflow: hidden;
  }
  .val code {
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .val code.empty,
  .mask {
    color: var(--text-1);
  }
  .acts {
    display: flex;
    gap: 4px;
  }
  .icon {
    min-width: 22px;
    padding-inline: 4px;
  }
  .dangertext {
    color: var(--danger);
  }
  .empty-list {
    padding: 8px;
    margin: 0;
  }
  .add .input {
    flex: 1 1 180px;
    min-width: 0;
  }
  .keep {
    flex: 1 1 180px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .check {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .deploy {
    border: 1px solid var(--warn);
    padding: 8px;
    margin-top: 10px;
  }
  .deploy.ok {
    border-color: var(--ok);
  }
  .deploy h4 {
    margin: 0 0 4px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .deploy p {
    margin: 0 0 4px;
  }
  .tag {
    display: inline-block;
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 4px;
    margin-right: 4px;
  }
  .tag.ok {
    color: var(--ok);
    border-color: var(--ok);
  }
  .tag.warn,
  .warn-text {
    color: var(--warn);
  }
  .tag.warn {
    border-color: var(--warn);
  }
  .spacer {
    flex: 1;
  }
</style>
