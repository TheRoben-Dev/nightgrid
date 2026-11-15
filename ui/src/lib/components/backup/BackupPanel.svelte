<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, type BackupFolder, type BackupImported, type BackupPreview, type ImportMode } from "../../api";
  import { scramble } from "../../scramble";
  import { ago } from "../../stores/alerts.svelte";
  import { fmtDate, passphraseProblem, summaryLines } from "../../stores/backup";
  import Modal from "../Modal.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";

  const errMsg = (e: unknown) => (e instanceof Error ? e.message : typeof e === "object" && e && "message" in e ? String(e.message) : String(e));

  let folder: BackupFolder | null = $state(null);

  // Export
  let pass = $state("");
  let confirm = $state("");
  let withSecrets = $state(false);
  let exporting = $state(false);
  let exportMsg = $state<string | null>(null);
  let exportErr = $state<string | null>(null);
  const problem = $derived(passphraseProblem(pass, confirm));

  // Import
  let importOpen = $state(false);
  let fromFolder = $state(false);
  let ipass = $state("");
  let busy = $state(false);
  let preview: BackupPreview | null = $state(null);
  let mode: ImportMode = $state("merge");
  let typed = $state("");
  let done: BackupImported | null = $state(null);
  let importErr = $state<string | null>(null);
  const importOk = $derived(mode === "merge" || typed.trim().toLowerCase() === "replace");

  onMount(() => {
    void api.backupFolder().then((f) => (folder = f)).catch(() => {});
  });

  async function doExport(toFolder: boolean) {
    if (problem) return;
    exporting = true;
    exportMsg = exportErr = null;
    try {
      const r = await api.backupExport(pass, withSecrets, toFolder);
      if (r) {
        exportMsg = `Saved ${r.path}${r.secrets ? ` with ${r.secrets} saved secret${r.secrets === 1 ? "" : "s"}` : ""}.`;
        pass = confirm = "";
        if (toFolder) folder = await api.backupFolder();
      }
    } catch (e) {
      exportErr = errMsg(e);
    } finally {
      exporting = false;
    }
  }

  function startImport(useFolder: boolean) {
    fromFolder = useFolder;
    ipass = typed = "";
    preview = done = importErr = null;
    mode = "merge";
    importOpen = true;
  }

  async function open() {
    busy = true;
    importErr = null;
    try {
      preview = await api.backupOpen(ipass, fromFolder);
    } catch (e) {
      importErr = errMsg(e);
    } finally {
      busy = false;
    }
  }

  async function doImport() {
    if (!importOk) return;
    busy = true;
    importErr = null;
    try {
      done = await api.backupImport(ipass, mode);
      ipass = "";
    } catch (e) {
      importErr = errMsg(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    if (busy) return;
    if (done) {
      void api.appRestart();
      return;
    }
    importOpen = false;
    ipass = "";
  }

  async function pickFolder() {
    try {
      const f = await api.backupPickFolder();
      if (f) folder = f;
    } catch (e) {
      exportErr = errMsg(e);
    }
  }

  async function clearFolder() {
    await api.backupClearFolder();
    folder = null;
  }
</script>

<Panel title="BACKUP" guide="settings-backup">
  <p class="muted lead">
    Moves your setup between machines: hosts, groups, tags, snippets, playbooks, services, tunnels, integrations,
    bookmarks and settings, in one file encrypted with a passphrase (age). Metric and check history stay here.
  </p>

  <form class="export" onsubmit={(e) => (e.preventDefault(), void doExport(false))}>
    <span class="label sub">EXPORT</span>
    <div class="row2">
      <label class="field">
        <span>PASSPHRASE</span>
        <input class="input" type="password" autocomplete="new-password" bind:value={pass} />
      </label>
      <label class="field">
        <span>AGAIN</span>
        <input class="input" type="password" autocomplete="new-password" bind:value={confirm} aria-invalid={!!confirm && pass !== confirm} />
      </label>
    </div>
    {#if pass && problem}<p class="field-hint">{problem}</p>{/if}
    <Toggle
      label="Include saved passwords and tokens"
      hint="Copies keychain entries (host passwords, key passphrases, sudo passwords, integration tokens, the alert webhook) into the encrypted file. Off: the other machine asks for them again."
      checked={withSecrets}
      onchange={(v) => (withSecrets = v)}
    />
    <div class="actions">
      <button type="submit" class="btn primary" disabled={!!problem || exporting}>{exporting ? "Exporting…" : "Export…"}</button>
      {#if folder}
        <button type="button" class="btn" disabled={!!problem || exporting} onclick={() => void doExport(true)}>Export to sync folder</button>
      {/if}
    </div>
    {#if exportMsg}<p class="ok" role="status" use:scramble={exportMsg}></p>{/if}
    {#if exportErr}<p class="error" role="alert">{exportErr}</p>{/if}
  </form>

  <div class="block">
    <span class="label sub">IMPORT</span>
    <div class="actions">
      <button type="button" class="btn" onclick={() => startImport(false)}>Import…</button>
      {#if folder}
        <button type="button" class="btn" disabled={folder.modified === null} onclick={() => startImport(true)}>Import from sync folder</button>
      {/if}
    </div>
  </div>

  <div class="block">
    <span class="label sub">SYNC FOLDER</span>
    {#if folder}
      <p class="path data">{folder.path}</p>
      <p class="muted small">
        {#if folder.modified === null}No backup in it yet.{:else}Last backup {ago(Math.floor(folder.modified))} ago ({fmtDate(folder.modified)}).{/if}
      </p>
    {:else}
      <p class="muted small">
        Optional: pick a folder that syncs between your machines (Syncthing, a cloud drive, a git repo). Export and import
        then use <code>nightgrid-backup.age</code> in it without a file dialog. The passphrase is never stored.
      </p>
    {/if}
    <div class="actions">
      <button type="button" class="btn ghost" onclick={() => void pickFolder()}>{folder ? "Change…" : "Choose folder…"}</button>
      {#if folder}<button type="button" class="btn ghost" onclick={() => void clearFolder()}>Forget</button>{/if}
    </div>
    {#if !inTauri()}<p class="muted small">Browser preview: nothing is read or written.</p>{/if}
  </div>
</Panel>

<Modal open={importOpen} title={done ? "IMPORT DONE" : "IMPORT BACKUP"} tone={preview && mode === "replace" && !done ? "danger" : "default"} width={540} onclose={close}>
  {#if done}
    <p use:scramble={`Added ${done.added} item${done.added === 1 ? "" : "s"}${done.skipped ? `, skipped ${done.skipped} already here` : ""}${done.secrets ? `, saved ${done.secrets} secret${done.secrets === 1 ? "" : "s"} to the keychain` : ""}.`}></p>
    {#if done.secretsError}<p class="error" role="alert">The keychain step didn't finish ({done.secretsError}). Saved passwords and tokens may need to be entered again.</p>{/if}
    <p class="muted">Nightgrid restarts to load everything{done.settings ? ", including the imported settings" : ""}.</p>
  {:else if !preview}
    <form id="backup-open" onsubmit={(e) => (e.preventDefault(), void open())}>
      <p class="muted">
        {fromFolder ? `Reads nightgrid-backup.age from ${folder?.path ?? "the sync folder"}.` : "Enter the passphrase, then pick the file."}
        Nothing changes until you confirm on the next step.
      </p>
      <label class="field">
        <span>PASSPHRASE</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" type="password" autocomplete="off" bind:value={ipass} autofocus />
      </label>
    </form>
  {:else}
    {@const s = preview.summary}
    <form id="backup-import" onsubmit={(e) => (e.preventDefault(), void doImport())}>
      <p class="path data">{preview.path}</p>
      <p class="muted small">Made by Nightgrid {s.appVersion} on {fmtDate(s.createdAt)}.</p>
      <ul class="contents">{#each summaryLines(s) as l (l)}<li>{l}</li>{/each}</ul>
      <div class="modes" role="radiogroup" aria-label="Import mode">
        <button type="button" role="radio" aria-checked={mode === "merge"} class="mode" class:active={mode === "merge"} onclick={() => (mode = "merge")}>
          <span class="name">MERGE</span>
          <span class="muted small">Keep everything here and add what's new. Items that already exist (same name, alias or address) are skipped, along with their saved secrets. Your settings stay.</span>
        </button>
        <button type="button" role="radio" aria-checked={mode === "replace"} class="mode" class:active={mode === "replace"} class:danger={mode === "replace"} onclick={() => (mode = "replace")}>
          <span class="name">REPLACE</span>
          <span class="muted small">Delete all hosts, groups, snippets, playbooks, services, tunnels, integrations, bookmarks and settings here, with their saved secrets and history, then load the backup.</span>
        </button>
      </div>
      {#if mode === "replace"}
        <label class="field">
          <span>TYPE <code>replace</code> TO CONTINUE</span>
          <input class="input" bind:value={typed} autocomplete="off" aria-label="Type replace to confirm" />
        </label>
      {/if}
    </form>
  {/if}
  {#if importErr}<p class="error" role="alert">{importErr}</p>{/if}
  {#snippet footer()}
    {#if done}
      <button type="button" class="btn primary" onclick={() => void api.appRestart()}>Restart now</button>
    {:else if !preview}
      <button type="button" class="btn ghost" onclick={close} disabled={busy}>Cancel</button>
      <button type="submit" form="backup-open" class="btn primary" disabled={busy || !ipass}>
        {busy ? "Decrypting…" : fromFolder ? "Open" : "Choose file…"}
      </button>
    {:else}
      <button type="button" class="btn ghost" onclick={close} disabled={busy}>Cancel</button>
      <button type="submit" form="backup-import" class="btn {mode === 'replace' ? 'danger' : 'primary'}" disabled={busy || !importOk}>
        {busy ? "Importing…" : mode === "replace" ? "Replace" : "Merge"}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .lead {
    margin: 0 0 10px;
    font-size: var(--fs-sm);
  }
  .sub {
    display: block;
    margin-bottom: 6px;
    color: var(--text-1);
  }
  .export,
  .block {
    border-top: 1px solid var(--line);
    padding-top: 10px;
    margin-top: 10px;
  }
  .row2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
  .small {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
  }
  .path {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .data {
    color: var(--cyan);
  }
  .ok {
    color: var(--mint);
    margin: 8px 0 0;
    overflow-wrap: anywhere;
  }
  .error {
    color: var(--danger);
    margin: 8px 0 0;
  }
  .contents {
    margin: 10px 0;
    padding-left: 18px;
    columns: 2;
  }
  .modes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-bottom: 10px;
  }
  .mode {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--line);
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .mode .name {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.14em;
  }
  .mode.active {
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .mode.active .name {
    color: var(--magenta);
  }
  .mode.active.danger {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .mode.active.danger .name {
    color: var(--danger);
  }
</style>
