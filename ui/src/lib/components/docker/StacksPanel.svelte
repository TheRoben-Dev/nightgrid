<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { ComposeOp, Stack } from "../../api";
  import { docker } from "../../stores/docker.svelte";
  import { files } from "../../stores/files.svelte";
  import { composeFile, envFile, opConsequence, opLabel, stacks } from "../../stores/stacks.svelte";
  import { layout } from "../../stores/layout.svelte";
  import Modal from "../Modal.svelte";
  import Resizer from "../Resizer.svelte";

  onMount(() => {
    void stacks.load();
    const t = setInterval(() => void stacks.load(true), 15000);
    return () => clearInterval(t);
  });

  const ops: ComposeOp[] = ["up", "pull", "update", "restart", "down"];
  const hostName = $derived(docker.host?.name ?? "");
  const run = $derived(stacks.run);
  const pending = $derived(stacks.pending);

  let newDir = $state("");
  let dirError = $state<string | null>(null);
  async function addDir(e: SubmitEvent) {
    e.preventDefault();
    if (!newDir.trim()) return;
    dirError = await stacks.addDir(newDir.trim());
    if (!dirError) newDir = "";
  }

  function edit(s: Stack, which: "compose" | "env") {
    if (docker.hostId === null) return;
    files.openHostFile(docker.hostId, which === "compose" ? composeFile(s) : envFile(s), which === "env");
  }

  let consoleEl: HTMLPreElement | undefined = $state();
  let follow = true;
  $effect(() => {
    void run?.lines.length;
    if (follow && consoleEl) void tick().then(() => consoleEl && (consoleEl.scrollTop = consoleEl.scrollHeight));
  });
  function onScroll() {
    if (consoleEl) follow = consoleEl.scrollTop + consoleEl.clientHeight >= consoleEl.scrollHeight - 8;
  }

  const runState = $derived(
    !run ? "" : run.running ? "RUNNING" : run.error ? "FAILED" : run.code === 0 ? "DONE" : run.code === 130 ? "CANCELLED" : `EXIT ${run.code}`,
  );
</script>

<div class="stacks">
  <div class="dirs">
    <button type="button" class="dirs-toggle label" aria-expanded={stacks.dirsOpen} onclick={() => (stacks.dirsOpen = !stacks.dirsOpen)}>
      <span aria-hidden="true">{stacks.dirsOpen ? "▾" : "▸"}</span> STACK FOLDERS
    </button>
    {#each stacks.dirs as d (d.id)}
      <span class="chip"><code>{d.path}</code><button type="button" class="x" aria-label="Remove {d.path}" onclick={() => stacks.removeDir(d.id)}>×</button></span>
    {:else}
      {#if !stacks.dirsOpen}<span class="muted">only stacks with containers are listed; add a folder to find the rest</span>{/if}
    {/each}
    {#if stacks.dirsOpen}
      <form class="adddir" onsubmit={addDir}>
        <input class="input" bind:value={newDir} placeholder="/opt/stacks or ~/stacks" spellcheck="false" aria-label="Stack folder" />
        <button type="submit" class="btn ghost tiny" disabled={!newDir.trim()}>Add</button>
        {#if dirError}<span class="error-text">{dirError}</span>{/if}
      </form>
      <p class="muted hint">Nightgrid looks for compose files in each folder and one level below it (e.g. /opt/stacks/media/compose.yaml).</p>
    {/if}
  </div>

  <div class="table" role="table" aria-label="Compose stacks">
    {#if stacks.error}
      <div class="empty">
        <p class="error-text" role="alert">{stacks.error}</p>
        <button type="button" class="btn primary" onclick={() => stacks.load()}>Retry</button>
      </div>
    {:else if !stacks.loaded}
      <p class="muted pad">Looking for compose stacks on {hostName}…</p>
    {:else if !stacks.visible.length}
      <p class="muted pad">{stacks.stacks.length ? "No stacks match." : "No compose stacks found. Add a stack folder above."}</p>
    {:else}
      <div class="row head" role="row">
        <span role="columnheader">STACK</span>
        <span role="columnheader">FOLDER</span>
        <span role="columnheader" class="num">UP</span>
        <span role="columnheader">FILES</span>
        <span role="columnheader">ACTIONS</span>
      </div>
      {#each stacks.visible as s (s.name + s.dir)}
        {@const active = run?.stack.name === s.name && run.stack.dir === s.dir}
        <div class="row" class:active role="row">
          <span class="name" role="cell"><span class="dot" class:on={s.running > 0} aria-hidden="true"></span>{s.name}</span>
          <span class="dir muted" role="cell" title={s.dir}>{s.dir}</span>
          <span class="num data" role="cell"><span class:mint={s.running > 0}>{s.running}</span>/{s.total}</span>
          <span class="files" role="cell">
            <button type="button" class="btn ghost tiny" title="Edit {composeFile(s)}" onclick={() => edit(s, "compose")}
              >{composeFile(s).split("/").pop()}</button
            >
            <button type="button" class="btn ghost tiny" title="Edit {envFile(s)} (created on save if missing)" onclick={() => edit(s, "env")}
              >.env</button
            >
          </span>
          <span class="ops" role="cell">
            {#each ops as op (op)}
              <button
                type="button"
                class="btn tiny"
                class:ghost={op !== "down"}
                class:danger={op === "down"}
                class:warnbtn={op === "update" || op === "restart"}
                disabled={stacks.busy}
                title={opConsequence[op]}
                onclick={() => stacks.request(s, op)}>{opLabel[op]}</button
              >
            {/each}
          </span>
        </div>
      {/each}
    {/if}
  </div>

  {#if run}
    <Resizer key="stacksConsole" axis="y" invert label="Resize compose output" />
    <section class="console" aria-label="Compose output" style:flex-basis="{layout.get('stacksConsole')}%">
      <header>
        <span class="label">// {opLabel[run.op].toUpperCase()} {run.stack.name}</span>
        <span class="muted">on {hostName}</span>
        <span class="state" class:ok={runState === "DONE"} class:bad={runState === "FAILED" || runState.startsWith("EXIT")} class:live={run.running}
          >{runState}</span
        >
        <span class="spacer"></span>
        {#if run.running}
          <button type="button" class="btn ghost tiny" onclick={() => stacks.cancel()}>Cancel (Ctrl-C)</button>
        {:else}
          <button type="button" class="btn ghost tiny" onclick={() => (stacks.run = null)}>Close</button>
        {/if}
      </header>
      {#if run.changes}
        <div class="changes" role="status">
          {#if run.changes.length}
            <span class="label mint">UPDATED</span>
            {#each run.changes as c (c.service)}
              <span class="change"><strong>{c.service}</strong> <span class="muted">{c.image}</span> <code>{c.before ?? "new"}</code> → <code class="data">{c.after}</code></span>
            {/each}
          {:else}
            <span class="muted">All images were already up to date; nothing was recreated.</span>
          {/if}
        </div>
      {/if}
      {#if run.error}<p class="error-text pad" role="alert">{run.error}</p>{/if}
      <pre bind:this={consoleEl} onscroll={onScroll}>{run.lines.join("\n")}</pre>
    </section>
  {/if}
</div>

<Modal
  open={!!pending}
  title={pending ? `${opLabel[pending.op].toUpperCase()} STACK` : ""}
  tone="danger"
  width={540}
  onclose={() => (stacks.pending = null)}
>
  {#if pending}
    <p>{opLabel[pending.op]} the stack <strong>{pending.stack.name}</strong> on <strong>{hostName}</strong>?</p>
    <p class="muted"><code>{pending.stack.dir}</code></p>
    <p class="muted">{opConsequence[pending.op]}</p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (stacks.pending = null)}>Cancel</button>
    <button type="button" class="btn danger" onclick={() => pending && stacks.start(pending.stack, pending.op)}>
      {pending ? `${opLabel[pending.op]} ${pending.stack.name}` : ""}
    </button>
  {/snippet}
</Modal>

<style>
  .stacks {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .dirs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  .dirs-toggle {
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    padding: 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 1px solid var(--line);
    padding: 1px 4px 1px 6px;
  }
  .chip code {
    color: var(--cyan);
  }
  .x {
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .adddir {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-basis: 100%;
  }
  .adddir .input {
    width: 280px;
  }
  .hint {
    flex-basis: 100%;
    margin: 0;
  }
  .table {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(120px, 1fr) minmax(140px, 1.4fr) 56px minmax(150px, auto) auto;
    gap: 10px;
    align-items: center;
    padding: 3px 8px;
    min-height: 32px;
    font-size: var(--fs-sm);
    border-bottom: 1px solid color-mix(in srgb, var(--line) 45%, transparent);
  }
  .row.active {
    border-left: 2px solid var(--magenta);
    background: color-mix(in srgb, var(--magenta) 6%, transparent);
  }
  .head {
    position: sticky;
    top: 0;
    background: var(--bg-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    min-height: 24px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    min-width: 0;
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    background: var(--line);
  }
  .dot.on {
    background: var(--mint);
    box-shadow: 0 0 6px var(--mint);
  }
  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .num {
    text-align: right;
  }
  .files,
  .ops {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .warnbtn {
    background: transparent;
    border-color: color-mix(in srgb, var(--accent) 60%, var(--line));
    color: var(--accent);
  }
  .mint {
    color: var(--mint);
  }
  .data {
    color: var(--cyan);
  }
  .pad {
    padding: 12px;
  }
  .empty {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
  }
  .console {
    flex: 0 0 40%;
    min-height: 160px;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--magenta);
    background: var(--bg-0);
  }
  .console header {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border-bottom: 1px solid var(--line);
  }
  .console .label {
    color: var(--magenta);
  }
  .state {
    letter-spacing: 0.12em;
    color: var(--text-1);
  }
  .state.live {
    color: var(--accent);
  }
  .state.ok {
    color: var(--mint);
  }
  .state.bad {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .changes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border-bottom: 1px solid var(--line);
  }
  .change code {
    font-size: var(--fs-xs);
  }
  pre {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 6px 8px;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    line-height: 1.45;
    color: var(--text-0);
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
