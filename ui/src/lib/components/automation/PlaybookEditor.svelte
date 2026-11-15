<script lang="ts">
  import { errorText, type AutomationDockerAction, type PlaybookInput, type Step } from "../../api";
  import { automation, emptyPlaybook, newStep, STEP_LABEL, toPlaybookInput } from "../../stores/automation.svelte";
  import Modal from "../Modal.svelte";

  const open = $derived(automation.editingPlaybook !== null);
  const existing = $derived(automation.editingPlaybook ? automation.playbooks.find((p) => p.id === automation.editingPlaybook) : undefined);

  let form = $state<PlaybookInput>(emptyPlaybook());
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmDelete = $state(false);

  $effect(() => {
    if (automation.editingPlaybook === null) return;
    const p = automation.editingPlaybook ? automation.playbooks.find((x) => x.id === automation.editingPlaybook) : undefined;
    form = p ? toPlaybookInput(p) : emptyPlaybook();
    error = null;
    confirmDelete = false;
  });

  const TYPES: Step["type"][] = ["command", "upload", "waitPort", "docker"];
  const ACTIONS: AutomationDockerAction[] = ["start", "stop", "restart", "pull"];

  function move(i: number, d: -1 | 1) {
    const j = i + d;
    if (j < 0 || j >= form.steps.length) return;
    const s = [...form.steps];
    [s[i], s[j]] = [s[j], s[i]];
    form.steps = s;
  }
  const removeStep = (i: number) => (form.steps = form.steps.filter((_, k) => k !== i));
  const add = (t: Step["type"]) => (form.steps = [...form.steps, newStep(t)]);

  function setMode(s: Extract<Step, { type: "upload" }>, v: string) {
    const t = v.trim();
    s.mode = /^[0-7]{3,4}$/.test(t) ? parseInt(t, 8) : null;
  }
  const num = (v: string, min: number, max: number) => Math.max(min, Math.min(max, Math.trunc(Number(v)) || min));

  function close() {
    automation.editingPlaybook = null;
  }

  async function save(e: Event) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      const p = await automation.savePlaybook(automation.editingPlaybook ?? 0, $state.snapshot(form));
      automation.pick({ kind: "playbook", id: p.id });
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
      await automation.deletePlaybook(existing.id);
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {open} title={existing ? "EDIT PLAYBOOK" : "NEW PLAYBOOK"} width={720} onclose={close}>
  <form id="playbook-form" class="grid" onsubmit={save}>
    <div class="two">
      <label class="field">
        <span>NAME</span>
        <input class="input" bind:value={form.name} placeholder="Deploy app" maxlength="80" />
      </label>
      <label class="field">
        <span>DESCRIPTION · OPTIONAL</span>
        <input class="input" bind:value={form.description} />
      </label>
    </div>

    <ol class="steps">
      {#each form.steps as s, i (i)}
        <li class="step">
          <header>
            <span class="no">{String(i + 1).padStart(2, "0")}</span>
            <span class="label">{STEP_LABEL[s.type]}</span>
            <span class="spacer"></span>
            <button type="button" class="icon" aria-label="Move step {i + 1} up" disabled={i === 0} onclick={() => move(i, -1)}>↑</button>
            <button type="button" class="icon" aria-label="Move step {i + 1} down" disabled={i === form.steps.length - 1} onclick={() => move(i, 1)}>↓</button>
            <button type="button" class="icon" aria-label="Remove step {i + 1}" onclick={() => removeStep(i)}>×</button>
          </header>
          {#if s.type === "command"}
            <textarea class="input mono" rows="2" bind:value={s.command} spellcheck="false" aria-label="Command for step {i + 1}" placeholder={"cd /opt/{{app}} && git pull"}></textarea>
            <label class="check"><input type="checkbox" bind:checked={s.sudo} /> Run as root (sudo)</label>
          {:else if s.type === "upload"}
            <div class="two">
              <label class="field"><span>FROM THIS COMPUTER</span><input class="input mono" bind:value={s.localPath} placeholder="~/deploy/app.env" /></label>
              <label class="field"><span>TO THE HOST</span><input class="input mono" bind:value={s.remotePath} placeholder={"/opt/{{app}}/.env"} /></label>
            </div>
            <label class="field narrow">
              <span>PERMISSIONS · OPTIONAL</span>
              <input class="input mono" value={s.mode != null ? s.mode.toString(8) : ""} placeholder="644" onchange={(e) => setMode(s, e.currentTarget.value)} />
            </label>
            <small class="muted">Uploaded over SFTP as the login user. Replaces the file if it exists.</small>
          {:else if s.type === "waitPort"}
            <div class="three">
              <label class="field"><span>HOST (AS THE SERVER SEES IT)</span><input class="input mono" bind:value={s.host} /></label>
              <label class="field"><span>PORT</span><input class="input" type="number" min="1" max="65535" value={s.port} onchange={(e) => (s.port = num(e.currentTarget.value, 1, 65535))} /></label>
              <label class="field"><span>TIMEOUT (S)</span><input class="input" type="number" min="1" max="3600" value={s.timeoutSecs} onchange={(e) => (s.timeoutSecs = num(e.currentTarget.value, 1, 3600))} /></label>
            </div>
          {:else if s.type === "docker"}
            <div class="two">
              <label class="field">
                <span>ACTION</span>
                <select class="input" value={s.action} onchange={(e) => (s.action = e.currentTarget.value as AutomationDockerAction)}>
                  {#each ACTIONS as a (a)}<option value={a}>{a === "pull" ? "pull image" : `${a} container`}</option>{/each}
                </select>
              </label>
              <label class="field"><span>{s.action === "pull" ? "IMAGE" : "CONTAINER"}</span><input class="input mono" bind:value={s.target} placeholder={s.action === "pull" ? "nginx:latest" : "web"} /></label>
            </div>
          {/if}
        </li>
      {/each}
    </ol>

    <div class="add">
      <span class="label">ADD STEP</span>
      {#each TYPES as t (t)}<button type="button" class="btn ghost tiny" disabled={form.steps.length >= 50} onclick={() => add(t)}>+ {STEP_LABEL[t]}</button>{/each}
    </div>

    <label class="check"><input type="checkbox" bind:checked={form.stopOnError} /> Stop at the first failed step</label>
    <small class="muted">All text fields can use <code>{"{{variables}}"}</code>; they're asked for once per run.</small>
    {#if error}<p class="error-text" role="alert">{error}</p>{/if}
  </form>
  {#snippet footer()}
    {#if existing}
      {#if confirmDelete}
        <span class="del">Delete “{existing.name}”?</span>
        <button type="button" class="btn danger" disabled={busy} onclick={remove}>Delete</button>
        <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      {:else}
        <button type="button" class="btn ghost" onclick={() => (confirmDelete = true)}>Delete…</button>
      {/if}
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={close}>Cancel</button>
    <button type="submit" form="playbook-form" class="btn primary" disabled={busy || !form.name.trim() || !form.steps.length}>Save</button>
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    gap: 12px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .three {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr;
    gap: 8px;
  }
  .narrow {
    max-width: 180px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  textarea.input {
    resize: vertical;
    width: 100%;
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .step {
    border: 1px solid var(--line);
    padding: 8px 10px 10px;
    display: grid;
    gap: 8px;
  }
  .step header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .no {
    color: var(--cyan);
    font-size: var(--fs-xs);
  }
  .step .label {
    color: var(--text-1);
  }
  .spacer {
    flex: 1;
  }
  .icon {
    width: 24px;
    height: 24px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .icon:hover:not(:disabled) {
    border-color: var(--line);
    color: var(--text-0);
  }
  .icon:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .add {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .add .label {
    margin-right: 4px;
    color: var(--text-1);
  }
  .tiny {
    min-height: 20px;
    padding: 0 8px;
    font-size: var(--fs-xs);
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
  small {
    font-size: var(--fs-xs);
  }
  .del {
    font-size: var(--fs-sm);
    color: var(--danger);
    align-self: center;
  }
</style>
