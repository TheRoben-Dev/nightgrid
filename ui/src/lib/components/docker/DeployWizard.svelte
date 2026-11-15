<script lang="ts">
  /**
   * Deploy wizard (M10f): an image (with ports, volumes and env suggested from
   * its registry metadata) or a pasted compose file → review → written to
   * <stack folder>/<name>/ on the host and checked with `docker compose
   * config`. Env values go to .env (mode 600), never into compose.yaml, and
   * are masked until revealed (rule 6). Existing files are only replaced after
   * a confirmation naming them (rule 5).
   */
  import { api, errorText, type RemoteImage, type Stack } from "../../api";
  import {
    composeYaml,
    envText,
    formFromImage,
    formProblems,
    nameFromImage,
    slug,
    type ServiceForm,
  } from "../../docker/composeGen";
  import { docker } from "../../stores/docker.svelte";
  import { stacks } from "../../stores/stacks.svelte";
  import Modal from "../Modal.svelte";

  type Step = "source" | "settings" | "review" | "done";
  let step = $state<Step>("source");
  let mode = $state<"image" | "paste">("image");
  let image = $state("");
  let info = $state<RemoteImage | null>(null);
  let looking = $state(false);
  let lookError = $state<string | null>(null);
  let form = $state<ServiceForm>({ name: "", image: "", restart: "unless-stopped", ports: [], volumes: [], env: [] });
  let pasted = $state("");
  let pastedEnv = $state("");
  let name = $state("");
  let parent = $state("");
  let yaml = $state("");
  let env = $state<string | null>(null);
  let reveal = $state(false);
  let deploying = $state(false);
  let deployError = $state<string | null>(null);
  let exists = $state<string | null>(null);
  let created = $state<Stack | null>(null);

  const hostName = $derived(docker.host?.name ?? "");
  const problems = $derived(mode === "image" ? formProblems(form) : []);

  $effect(() => {
    if (stacks.wizardOpen) reset();
  });

  function reset() {
    step = "source";
    mode = "image";
    image = "";
    info = null;
    lookError = null;
    pasted = "";
    pastedEnv = "";
    name = "";
    parent = stacks.dirs[0]?.path ?? "";
    deployError = null;
    exists = null;
    created = null;
    reveal = false;
  }

  async function lookup() {
    const id = docker.hostId;
    if (id === null || !image.trim()) return;
    looking = true;
    lookError = null;
    try {
      info = await api.dockerImageLookup(id, image.trim());
      form = formFromImage(info);
    } catch (e) {
      // Still usable: the user can fill the form in by hand.
      lookError = errorText(e);
      info = null;
      form = { name: nameFromImage(image), image: image.trim(), restart: "unless-stopped", ports: [], volumes: [], env: [] };
    } finally {
      looking = false;
      step = "settings";
    }
  }

  function toReview() {
    if (mode === "image") {
      name = slug(form.name);
      yaml = composeYaml(form);
      env = envText(form);
    } else {
      name = slug(name || "stack");
      yaml = pasted;
      env = pastedEnv.trim() ? pastedEnv : null;
    }
    deployError = null;
    exists = null;
    step = "review";
  }

  async function deploy(overwrite: boolean) {
    const id = docker.hostId;
    if (id === null) return;
    deploying = true;
    deployError = null;
    try {
      created = await api.dockerStackDeploy(id, { parent: parent.trim(), name, compose: yaml, env, overwrite });
      exists = null;
      step = "done";
      if (!stacks.dirs.some((d) => d.path === parent.trim())) await stacks.addDir(parent.trim());
      await stacks.load(true);
    } catch (e) {
      const msg = errorText(e);
      if (msg.startsWith("already exists:")) exists = msg.slice("already exists:".length).trim();
      else deployError = msg;
    } finally {
      deploying = false;
    }
  }

  function startIt() {
    const s = created;
    stacks.wizardOpen = false;
    if (s) void stacks.start(s, "up");
  }

  const close = () => (stacks.wizardOpen = false);
  const target = $derived(`${parent.trim().replace(/\/+$/, "")}/${name || "…"}`);
</script>

<Modal open={stacks.wizardOpen} title="NEW STACK" width={760} onclose={close}>
  <ol class="steps" aria-label="Steps">
    {#each [["source", "Source"], ["settings", "Settings"], ["review", "Review"], ["done", "Deployed"]] as [id, label], i (id)}
      <li class:on={step === id}><span class="n">{i + 1}</span>{label}</li>
    {/each}
  </ol>

  {#if step === "source"}
    <div class="seg" role="radiogroup" aria-label="Start from">
      <label class:on={mode === "image"}><input type="radio" bind:group={mode} value="image" /> An image</label>
      <label class:on={mode === "paste"}><input type="radio" bind:group={mode} value="paste" /> A compose file</label>
    </div>
    {#if mode === "image"}
      <form
        class="row"
        onsubmit={(e) => {
          e.preventDefault();
          void lookup();
        }}
      >
        <input class="input grow" placeholder="e.g. ghcr.io/owner/app:latest or nginx:1.27" aria-label="Image" bind:value={image} spellcheck="false" />
        <button type="submit" class="btn primary" disabled={!image.trim() || looking}>{looking ? "Looking up…" : "Next"}</button>
      </form>
      <p class="muted small">Nightgrid reads the image's declared ports, volumes and settings from its registry (nothing is pulled) and suggests them.</p>
    {:else}
      <label class="field">
        <span class="label">NAME</span>
        <input class="input" bind:value={name} placeholder="my-stack" spellcheck="false" />
      </label>
      <label class="field">
        <span class="label">COMPOSE.YAML</span>
        <textarea class="input code" rows="12" bind:value={pasted} spellcheck="false" placeholder="services:&#10;  app:&#10;    image: …"></textarea>
      </label>
      <label class="field">
        <span class="label">.ENV (OPTIONAL, WRITTEN WITH MODE 600)</span>
        <textarea class="input code" class:masked={!reveal} rows="3" bind:value={pastedEnv} spellcheck="false"></textarea>
      </label>
      <label class="small"><input type="checkbox" bind:checked={reveal} /> Show values</label>
    {/if}
  {:else if step === "settings"}
    {#if lookError}<p class="warn small">Couldn't read the image's metadata ({lookError}). Fill the settings in yourself.</p>{/if}
    {#if info?.description}<p class="muted small">{info.description}</p>{/if}
    <div class="grid2">
      <label class="field"><span class="label">NAME</span><input class="input" bind:value={form.name} spellcheck="false" /></label>
      <label class="field"
        ><span class="label">RESTART</span>
        <select class="input" bind:value={form.restart}>
          <option value="unless-stopped">unless-stopped</option>
          <option value="always">always</option>
          <option value="on-failure">on-failure</option>
          <option value="no">no</option>
        </select>
      </label>
    </div>
    <div class="sect">
      <span class="label">PORTS <span class="muted">HOST : CONTAINER</span></span>
      {#each form.ports as p, i (i)}
        <div class="row">
          <input class="input port" bind:value={p.host} aria-label="Host port" placeholder="host" />
          <span>:</span>
          <input class="input port" bind:value={p.container} aria-label="Container port" placeholder="container" />
          <select class="input" bind:value={p.proto} aria-label="Protocol"><option value="tcp">tcp</option><option value="udp">udp</option></select>
          <button type="button" class="btn ghost tiny" aria-label="Remove port" onclick={() => form.ports.splice(i, 1)}>×</button>
        </div>
      {/each}
      <button type="button" class="btn ghost tiny" onclick={() => form.ports.push({ host: "", container: "", proto: "tcp" })}>+ Port</button>
    </div>
    <div class="sect">
      <span class="label">VOLUMES <span class="muted">HOST PATH (RELATIVE TO THE STACK FOLDER) : CONTAINER PATH</span></span>
      {#each form.volumes as v, i (i)}
        <div class="row">
          <input class="input grow" bind:value={v.host} aria-label="Host path" />
          <span>:</span>
          <input class="input grow" bind:value={v.container} aria-label="Container path" />
          <button type="button" class="btn ghost tiny" aria-label="Remove volume" onclick={() => form.volumes.splice(i, 1)}>×</button>
        </div>
      {/each}
      <button type="button" class="btn ghost tiny" onclick={() => form.volumes.push({ host: "./data", container: "" })}>+ Volume</button>
    </div>
    <div class="sect">
      <span class="label">ENVIRONMENT <span class="muted">VALUES GO TO .ENV, NOT COMPOSE.YAML</span></span>
      {#each form.env as e, i (i)}
        <div class="row">
          <input class="input name" bind:value={e.name} aria-label="Variable name" spellcheck="false" />
          <span>=</span>
          <input class="input grow" type={reveal ? "text" : "password"} bind:value={e.value} aria-label="Value of {e.name}" autocomplete="off" />
          <button type="button" class="btn ghost tiny" aria-label="Remove variable" onclick={() => form.env.splice(i, 1)}>×</button>
        </div>
      {/each}
      <div class="row">
        <button type="button" class="btn ghost tiny" onclick={() => form.env.push({ name: "", value: "" })}>+ Variable</button>
        {#if form.env.length}<label class="small"><input type="checkbox" bind:checked={reveal} /> Show values</label>{/if}
      </div>
    </div>
    {#each problems as p (p)}<p class="error-text small">{p}</p>{/each}
  {:else if step === "review"}
    <div class="grid2">
      <label class="field"
        ><span class="label">STACK FOLDER</span>
        <input class="input" list="ng-stack-dirs" bind:value={parent} placeholder="/opt/stacks" spellcheck="false" />
        <datalist id="ng-stack-dirs">{#each stacks.dirs as d (d.id)}<option value={d.path}></option>{/each}</datalist>
      </label>
      <label class="field"><span class="label">NAME</span><input class="input" bind:value={name} spellcheck="false" /></label>
    </div>
    <p class="muted small">Creates <code>{target}/compose.yaml</code>{#if env}&nbsp;and <code>{target}/.env</code>{/if} on <strong>{hostName}</strong>, then checks it with <code>docker compose config</code>.</p>
    <label class="field"><span class="label">COMPOSE.YAML</span><textarea class="input code" rows="12" bind:value={yaml} spellcheck="false"></textarea></label>
    {#if env !== null}
      <label class="field"><span class="label">.ENV</span><textarea class="input code" class:masked={!reveal} rows="3" bind:value={env} spellcheck="false"></textarea></label>
      <label class="small"><input type="checkbox" bind:checked={reveal} /> Show values</label>
    {/if}
    {#if exists}
      <div class="danger-box" role="alert">
        <p>These already exist on {hostName} and would be replaced:</p>
        <p><code>{exists}</code></p>
        <button type="button" class="btn danger tiny" disabled={deploying} onclick={() => void deploy(true)}>Replace them in {name}</button>
      </div>
    {/if}
    {#if deployError}<pre class="error-text small err">{deployError}</pre>{/if}
  {:else if created}
    <p>
      <strong>{created.name}</strong> is ready in <code>{created.dir}</code> on {hostName}. Start it now, or edit its files first from the stack list.
    </p>
  {/if}

  {#snippet footer()}
    {#if step === "settings" || (step === "review" && !created)}
      <button type="button" class="btn ghost" onclick={() => (step = step === "review" ? (mode === "image" ? "settings" : "source") : "source")}>Back</button>
    {/if}
    <button type="button" class="btn ghost" onclick={close}>{step === "done" ? "Close" : "Cancel"}</button>
    {#if step === "source" && mode === "paste"}
      <button type="button" class="btn primary" disabled={!pasted.trim()} onclick={toReview}>Next</button>
    {:else if step === "settings"}
      <button type="button" class="btn primary" disabled={problems.length > 0} onclick={toReview}>Review</button>
    {:else if step === "review"}
      <button type="button" class="btn primary" disabled={deploying || !parent.trim().startsWith("/") || !yaml.trim()} onclick={() => void deploy(false)}
        >{deploying ? "Writing…" : "Create stack"}</button
      >
    {:else if step === "done"}
      <button type="button" class="btn primary" onclick={startIt}>Start it (up -d)</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .steps {
    display: flex;
    gap: 14px;
    list-style: none;
    padding: 0;
    margin: 0 0 12px;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-2);
  }
  .steps li.on {
    color: var(--mint);
  }
  .n {
    display: inline-block;
    width: 16px;
    text-align: center;
    margin-right: 4px;
    border: 1px solid currentColor;
  }
  .seg {
    display: flex;
    gap: 6px;
    margin-bottom: 10px;
  }
  .seg label {
    border: 1px solid var(--line);
    padding: 4px 10px;
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  .seg label.on {
    border-color: var(--mint);
    color: var(--mint);
  }
  .seg input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-bottom: 4px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .port {
    width: 80px;
  }
  .name {
    width: 180px;
    font-family: var(--font-mono);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 8px;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .sect {
    margin: 8px 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: flex-start;
  }
  .sect .row {
    width: 100%;
  }
  .code {
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    resize: vertical;
  }
  .masked {
    -webkit-text-security: disc;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .warn {
    color: var(--warn, var(--yellow));
  }
  .danger-box {
    border: 1px solid var(--danger);
    padding: 8px 10px;
    margin: 8px 0;
  }
  .danger-box p {
    margin: 0 0 6px;
  }
  .err {
    white-space: pre-wrap;
    margin: 6px 0 0;
  }
</style>
