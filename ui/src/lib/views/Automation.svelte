<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../components/Modal.svelte";
  import Panel from "../components/Panel.svelte";
  import { automation, fmtMs, scopeLabel, type HostRun } from "../stores/automation.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { terms } from "../stores/terms.svelte";

  onMount(() => {
    void automation.load();
  });

  let filter = $state("");
  let typed = $state("");

  const src = $derived(automation.source);
  const snippet = $derived(src.kind === "snippet" ? automation.snippets.find((s) => s.id === src.id) : undefined);
  const playbook = $derived(src.kind === "playbook" ? automation.playbooks.find((p) => p.id === src.id) : undefined);
  const inspect = $derived(automation.inspect);

  const q = $derived(filter.trim().toLowerCase());
  const shownSnippets = $derived(
    automation.snippets.filter((s) => !q || s.name.toLowerCase().includes(q) || s.command.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)),
  );
  const shownPlaybooks = $derived(automation.playbooks.filter((p) => !q || p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q)));

  const groups = $derived(
    hosts.groups
      .map((g) => ({ g, members: hosts.hosts.filter((h) => h.groupId === g.id) }))
      .filter((x) => x.members.length > 0),
  );
  const groupSelected = (id: number) => {
    const m = hosts.hosts.filter((h) => h.groupId === id).map((h) => h.id);
    return m.length > 0 && m.every((x) => automation.targets.includes(x));
  };

  // Re-inspect (variables, danger, preview) shortly after anything changes.
  $effect(() => {
    void [automation.source, automation.command, automation.commandSudo, automation.targets[0], JSON.stringify(automation.values)];
    const t = setTimeout(() => void automation.refreshInspect(), 150);
    return () => clearTimeout(t);
  });

  const run = $derived(automation.run);
  const runHosts = $derived(run ? run.hostIds.map((id) => run.hosts[id]).filter(Boolean) : []);
  const finished = $derived(runHosts.filter((h) => h.status === "ok" || h.status === "failed" || h.status === "stopped").length);
  const failedCount = $derived(runHosts.filter((h) => h.status === "failed" || h.status === "stopped").length);
  const shownHost = $derived(run && automation.shown !== null ? run.hosts[automation.shown] : undefined);

  const statusWord = (h: HostRun) =>
    h.status === "ok" ? "OK" : h.status === "failed" ? (h.code !== null ? `EXIT ${h.code}` : "FAILED") : h.status === "stopped" ? "STOPPED" : h.status === "running" ? "RUNNING" : "QUEUED";
  const hostName = (id: number) => hosts.byId(id)?.name ?? `host ${id}`;

  function saveAsSnippet() {
    automation.snippetDraft = {
      name: "",
      command: automation.command,
      description: "",
      scope: { kind: "global" },
      sudo: automation.commandSudo,
    };
    automation.editingSnippet = 0;
  }

  function onRunKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      void automation.start();
    }
  }

  $effect(() => {
    if (automation.confirm) typed = "";
  });
  const confirmOk = $derived(!automation.confirm?.word || typed.trim() === automation.confirm.word);

  // Keep the output scrolled to the bottom while new lines arrive.
  let outEl: HTMLPreElement | undefined = $state();
  let stick = true;
  $effect(() => {
    void shownHost?.lines.length;
    if (outEl && stick) queueMicrotask(() => outEl && (outEl.scrollTop = outEl.scrollHeight));
  });
  const onScroll = () => {
    if (outEl) stick = outEl.scrollHeight - outEl.scrollTop - outEl.clientHeight < 24;
  };
</script>

<div class="auto">
  <aside class="lib">
    <input class="input" placeholder="Filter snippets and playbooks…" bind:value={filter} aria-label="Filter snippets and playbooks" />
    {#if automation.error}<p class="error-text" role="alert">{automation.error}</p>{/if}

    <Panel title="SNIPPETS" prefix=">" count={automation.snippets.length}>
      {#snippet actions()}
        <button type="button" class="btn tiny" onclick={() => (automation.editingSnippet = 0)} data-guide="automation-new-snippet">New</button>
      {/snippet}
      {#if automation.loaded && !automation.snippets.length}
        <p class="muted small">
          Save commands you run often. Use <code>{"{{name}}"}</code> for values to fill in, and
          <code>{"{{host}}"}</code> for the host's name.
        </p>
      {/if}
      <ul class="items" aria-label="Snippets">
        {#each shownSnippets as s (s.id)}
          <li class:active={src.kind === "snippet" && src.id === s.id}>
            <button type="button" class="item" onclick={() => automation.pick({ kind: "snippet", id: s.id })}>
              <span class="row1">
                <span class="name">{s.name}</span>
                {#if s.sudo}<span class="chip root" title="Runs as root">ROOT</span>{/if}
                <span class="chip scope">{scopeLabel(s.scope)}</span>
              </span>
              <code class="cmd">{s.command.split("\n")[0]}</code>
            </button>
            <button type="button" class="btn ghost tiny edit" aria-label="Edit {s.name}" onclick={() => (automation.editingSnippet = s.id)}>Edit</button>
          </li>
        {/each}
      </ul>
    </Panel>

    <Panel title="PLAYBOOKS" prefix=">" count={automation.playbooks.length}>
      {#snippet actions()}
        <button type="button" class="btn tiny" onclick={() => (automation.editingPlaybook = 0)} data-guide="automation-new-playbook">New</button>
      {/snippet}
      {#if automation.loaded && !automation.playbooks.length}
        <p class="muted small">Steps in order: commands, file uploads, waiting for a port, Docker actions. Stops at the first failure.</p>
      {/if}
      <ul class="items" aria-label="Playbooks">
        {#each shownPlaybooks as p (p.id)}
          <li class:active={src.kind === "playbook" && src.id === p.id}>
            <button type="button" class="item" onclick={() => automation.pick({ kind: "playbook", id: p.id })}>
              <span class="row1">
                <span class="name">{p.name}</span>
                <span class="chip scope">{p.steps.length} STEP{p.steps.length === 1 ? "" : "S"}</span>
              </span>
              {#if p.description}<span class="desc">{p.description}</span>{/if}
            </button>
            <button type="button" class="btn ghost tiny edit" aria-label="Edit {p.name}" onclick={() => (automation.editingPlaybook = p.id)}>Edit</button>
          </li>
        {/each}
      </ul>
    </Panel>
  </aside>

  <div class="main">
    <Panel title="RUN" prefix="//">
      {#snippet actions()}
        {#if src.kind !== "command"}
          <button type="button" class="btn ghost tiny" onclick={() => automation.pick({ kind: "command" })}>Type a command instead</button>
        {/if}
      {/snippet}
      <div class="form">
        {#if src.kind === "command"}
          <label class="field">
            <span>COMMAND</span>
            <textarea
              class="input mono"
              rows="3"
              placeholder={"uptime   ·   docker ps   ·   journalctl -u {{unit}} -n 50"}
              bind:value={automation.command}
              onkeydown={onRunKey}
              spellcheck="false"
              aria-label="Command"
              data-guide="automation-command"
            ></textarea>
          </label>
          <div class="opts">
            <label class="check"><input type="checkbox" bind:checked={automation.commandSudo} /> Run as root (sudo)</label>
            <button type="button" class="link" disabled={!automation.command.trim()} onclick={saveAsSnippet}>Save as snippet</button>
          </div>
        {:else if snippet}
          <div class="picked">
            <span class="label">SNIPPET</span>
            <strong>{snippet.name}</strong>
            {#if snippet.sudo}<span class="chip root">ROOT</span>{/if}
            {#if snippet.description}<span class="muted">· {snippet.description}</span>{/if}
          </div>
          <pre class="code">{snippet.command}</pre>
        {:else if playbook}
          <div class="picked">
            <span class="label">PLAYBOOK</span>
            <strong>{playbook.name}</strong>
            <span class="muted">· {playbook.stopOnError ? "stops at the first failure" : "keeps going after a failure"}</span>
          </div>
          <ol class="steps">
            {#each inspect?.steps ?? [] as s, i (i)}<li><code>{s}</code></li>{/each}
          </ol>
        {/if}

        <div class="field">
          <span>HOSTS · {automation.targets.length} SELECTED</span>
          {#if !hosts.hosts.length}
            <p class="muted small">Add a host first.</p>
          {:else}
            <div class="targets" role="group" aria-label="Hosts to run on">
              {#each groups as { g, members } (g.id)}
                <button type="button" class="tchip group" class:on={groupSelected(g.id)} aria-pressed={groupSelected(g.id)} onclick={() => automation.toggleGroup(g.id)}>
                  {g.name} <span class="n">{members.length}</span>
                </button>
              {/each}
              {#if groups.length}<span class="sep" aria-hidden="true"></span>{/if}
              {#each hosts.hosts as h (h.id)}
                <button type="button" class="tchip" class:on={automation.targets.includes(h.id)} aria-pressed={automation.targets.includes(h.id)} onclick={() => automation.toggleHost(h.id)}>
                  {h.name}
                </button>
              {/each}
              <button type="button" class="link small" onclick={() => automation.setTargets(hosts.hosts.map((h) => h.id))}>All</button>
              <button type="button" class="link small" onclick={() => automation.setTargets([])}>None</button>
            </div>
          {/if}
        </div>

        {#if inspect?.variables.length}
          <div class="vars">
            {#each inspect.variables as v (v.name)}
              <label class="field">
                <span>{v.name.toUpperCase()}</span>
                <input class="input mono" placeholder={v.default ?? ""} value={automation.values[v.name] ?? ""} oninput={(e) => (automation.values = { ...automation.values, [v.name]: e.currentTarget.value })} onkeydown={onRunKey} />
              </label>
            {/each}
          </div>
        {/if}

        {#if inspect?.needsSudo}
          <label class="field sudo">
            <span>SUDO PASSWORD · OPTIONAL</span>
            <input class="input" type="password" autocomplete="off" bind:value={automation.sudoPassword} placeholder="Only for hosts that ask" />
            <small class="muted">Root and passwordless-sudo hosts don't need it; hosts with a remembered password use that. Sent to sudo on stdin, never stored.</small>
          </label>
        {/if}

        {#if inspect?.preview && automation.targets.length}
          <div class="field">
            <span>PREVIEW · {hostName(automation.targets[0])}</span>
            <pre class="code preview">{inspect.preview}</pre>
          </div>
        {/if}

        {#if inspect?.destructive.length}
          <p class="danger-line" role="note">⚠ Destructive: {inspect.destructive.join(" · ")}. You'll be asked to confirm.</p>
        {/if}
        {#if automation.inspectError}<p class="error-text" role="alert">{automation.inspectError}</p>{/if}
        {#if automation.runError}<p class="error-text" role="alert">{automation.runError}</p>{/if}

        <div class="go">
          <span class="muted small">{src.kind === "command" ? "Ctrl/Cmd+Enter runs" : ""}</span>
          {#if automation.running}
            <button type="button" class="btn danger" onclick={() => automation.stop()}>Stop</button>
          {/if}
          <button
            type="button"
            class="btn primary chamfer-sm"
            disabled={!automation.spec || !automation.targets.length || automation.running || automation.starting}
            onclick={() => automation.start()}
            data-guide="automation-run"
          >
            Run on {automation.targets.length || "…"} host{automation.targets.length === 1 ? "" : "s"}
          </button>
        </div>
      </div>
    </Panel>

    {#if run}
      <Panel title="RESULTS" prefix=">" count={run.hostIds.length}>
        {#snippet actions()}
          <span class="muted small">{run.done ? (run.stopped ? "stopped" : "done") : `${finished}/${run.hostIds.length}`}</span>
          {#if run.done && failedCount}
            <button type="button" class="btn tiny" onclick={() => automation.retryFailed()}>Select {failedCount} failed</button>
          {/if}
        {/snippet}
        <p class="rtitle"><code>{run.title}</code></p>
        <div class="results" role="list" aria-label="Results per host">
          {#each runHosts as h (h.hostId)}
            {@const total = run.stepLabels.length}
            <div class="hrow {h.status}" class:sel={automation.shown === h.hostId} role="listitem">
              <button type="button" class="hbtn" onclick={() => (automation.shown = h.hostId)}>
                <span class="dot" aria-hidden="true"></span>
                <span class="hn">{hostName(h.hostId)}</span>
                <span class="hs">{statusWord(h)}</span>
                {#if total > 1}<span class="muted small">{h.steps.filter((s) => s.status === "ok").length}/{total} steps</span>{/if}
                <span class="muted small ms">{h.ms ? fmtMs(h.ms) : ""}</span>
              </button>
              {#if h.error}<span class="herr" title={h.error}>{h.error}</span>{/if}
              {#if h.needs === "login"}
                <button type="button" class="btn tiny" onclick={() => terms.openHost(h.hostId)}>Connect</button>
              {:else if h.needs === "sudo"}
                <span class="muted small">Type the sudo password above and run again.</span>
              {/if}
            </div>
          {/each}
        </div>
        {#if shownHost}
          {#if run.stepLabels.length > 1}
            <ol class="psteps">
              {#each run.stepLabels as label, i (i)}
                {@const s = shownHost.steps[i]}
                {#if s}
                  <li class={s.status}>
                    <code>{s.label}</code>
                    <span class="muted small">{s.status === "running" ? "…" : (s.error ?? (s.code !== null ? `exit ${s.code}` : s.status))} {s.ms ? `· ${fmtMs(s.ms)}` : ""}</span>
                  </li>
                {:else}
                  <li class="skipped">
                    <code>{label}</code>
                    <span class="muted small">{shownHost.status === "running" || shownHost.status === "queued" ? "" : "not run"}</span>
                  </li>
                {/if}
              {/each}
            </ol>
          {/if}
          <pre class="out" bind:this={outEl} onscroll={onScroll} aria-label="Output from {hostName(shownHost.hostId)}">{#if shownHost.dropped}<span class="s-info">… {shownHost.dropped} earlier lines not kept
</span>{/if}{#each shownHost.lines as l, i (i)}<span class="s-{l.stream}">{l.text}
</span>{/each}{#if !shownHost.lines.length}<span class="s-info">{shownHost.status === "queued" ? "Waiting…" : shownHost.status === "running" ? "" : "No output."}</span>{/if}</pre>
        {/if}
      </Panel>
    {/if}
  </div>
</div>

<Modal open={!!automation.confirm} title="CONFIRM RUN" tone="danger" width={520} onclose={() => (automation.confirm = null)}>
  {#if automation.confirm}
    {@const c = automation.confirm}
    <form id="auto-confirm" class="confirm" onsubmit={(e) => (e.preventDefault(), confirmOk && automation.go())}>
      <p><strong>{c.title === "command" ? "This command" : c.title}</strong> will run on <strong>{c.hostNames.length === 1 ? c.hostNames[0] : `${c.hostNames.length} hosts`}</strong> and includes:</p>
      <ul class="dl">{#each c.destructive as d (d)}<li><code>{d}</code></li>{/each}</ul>
      {#if c.hostNames.length > 1}<p class="hostlist">{c.hostNames.join(", ")}</p>{/if}
      {#if c.word}
        <label class="field">
          <span>TYPE <code>{c.word}</code> TO CONTINUE{Number.isInteger(Number(c.word)) ? " (THE NUMBER OF HOSTS)" : " (THE GROUP)"}</span>
          <input class="input" bind:value={typed} autocomplete="off" aria-label="Type {c.word} to confirm" />
        </label>
      {/if}
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (automation.confirm = null)}>Cancel</button>
    <button type="submit" form="auto-confirm" class="btn danger" disabled={!confirmOk}>Run</button>
  {/snippet}
</Modal>

<style>
  .auto {
    display: grid;
    grid-template-columns: minmax(240px, 320px) minmax(0, 1fr);
    gap: 12px;
    align-items: start;
  }
  @media (max-width: 900px) {
    .auto {
      grid-template-columns: 1fr;
    }
  }
  .lib,
  .main {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .tiny {
    min-height: 20px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
  .items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .items li {
    position: relative;
    border: 1px solid transparent;
  }
  .items li:hover {
    border-color: var(--line);
  }
  .items li.active {
    border-color: var(--magenta);
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .item {
    width: 100%;
    display: grid;
    gap: 2px;
    text-align: left;
    background: transparent;
    border: 0;
    color: var(--text-0);
    padding: 6px 8px;
    cursor: pointer;
    min-width: 0;
  }
  .row1 {
    display: flex;
    gap: 6px;
    align-items: center;
    min-width: 0;
    padding-right: 44px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
  }
  .cmd,
  .desc {
    font-size: var(--fs-xs);
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmd {
    color: var(--cyan);
  }
  .edit {
    position: absolute;
    top: 4px;
    right: 4px;
    opacity: 0;
  }
  .items li:hover .edit,
  .edit:focus-visible {
    opacity: 1;
  }
  .chip {
    font-size: 10px;
    letter-spacing: var(--tracking-label);
    border: 1px solid var(--line);
    padding: 0 4px;
    color: var(--text-1);
    white-space: nowrap;
  }
  .chip.root {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 50%, var(--line));
  }
  .form {
    display: grid;
    gap: 12px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  textarea.input {
    resize: vertical;
    min-height: 64px;
    width: 100%;
  }
  .opts {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-top: -4px;
    font-size: var(--fs-sm);
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    cursor: pointer;
  }
  input[type="checkbox"] {
    accent-color: var(--magenta);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-sm);
  }
  .link:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .link.small {
    font-size: var(--fs-xs);
  }
  .picked {
    display: flex;
    gap: 8px;
    align-items: baseline;
    flex-wrap: wrap;
    font-size: var(--fs-sm);
  }
  .picked .label {
    color: var(--cyan);
  }
  .code {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    font-size: var(--fs-sm);
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-0);
  }
  .preview {
    color: var(--cyan);
  }
  .steps,
  .psteps {
    margin: 0;
    padding-left: 22px;
    display: grid;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .targets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .tchip {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    padding: 3px 8px;
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .tchip:hover {
    color: var(--text-0);
  }
  .tchip.on {
    border-color: var(--magenta);
    color: var(--text-0);
    background: color-mix(in srgb, var(--magenta) 12%, transparent);
  }
  .tchip.group {
    text-transform: uppercase;
    letter-spacing: var(--tracking-label);
    font-size: var(--fs-xs);
  }
  .tchip .n {
    color: var(--cyan);
    margin-left: 4px;
  }
  .sep {
    width: 1px;
    align-self: stretch;
    background: var(--line);
    margin: 0 2px;
  }
  .vars {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 8px;
  }
  .sudo small {
    font-size: var(--fs-xs);
  }
  .danger-line {
    margin: 0;
    color: var(--danger);
    font-size: var(--fs-sm);
  }
  .go {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
  }
  .go .muted {
    margin-right: auto;
  }
  .rtitle {
    margin: 0 0 8px;
    font-size: var(--fs-sm);
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .results {
    display: grid;
    gap: 2px;
    margin-bottom: 10px;
  }
  .hrow {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid transparent;
    padding-right: 6px;
    min-width: 0;
  }
  .hrow.sel {
    border-color: var(--magenta);
  }
  .hbtn {
    display: flex;
    align-items: center;
    gap: 10px;
    background: transparent;
    border: 0;
    color: var(--text-0);
    padding: 5px 8px;
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-sm);
    flex: 0 1 auto;
    min-width: 0;
  }
  .hn {
    min-width: 120px;
    text-align: left;
  }
  .hs {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    font-size: var(--fs-xs);
    min-width: 70px;
    text-align: left;
  }
  .ms {
    min-width: 50px;
  }
  .dot {
    width: 8px;
    height: 8px;
    background: var(--line);
    flex: none;
  }
  .running .dot {
    background: var(--cyan);
    animation: pulse 1s infinite;
  }
  .ok .dot {
    background: var(--mint);
  }
  .ok .hs {
    color: var(--mint);
  }
  .failed .dot {
    background: var(--danger);
  }
  .failed .hs {
    color: var(--danger);
  }
  .stopped .dot {
    background: var(--warn);
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  :global(:root[data-effects="off"]) .running .dot {
    animation: none;
  }
  .herr {
    color: var(--danger);
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .psteps {
    margin-bottom: 8px;
  }
  .psteps li.ok code {
    color: var(--mint);
  }
  .psteps li.failed code {
    color: var(--danger);
  }
  .psteps li.running code {
    color: var(--cyan);
  }
  .psteps li.skipped code {
    color: var(--text-1);
    opacity: 0.6;
  }
  .out {
    margin: 0;
    max-height: 420px;
    min-height: 120px;
    overflow: auto;
    background: var(--bg-0);
    border: 1px solid var(--line);
    padding: 8px 10px;
    font-size: var(--fs-sm);
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .s-err {
    color: var(--danger);
  }
  .s-info {
    color: var(--cyan);
  }
  .confirm {
    display: grid;
    gap: 10px;
    font-size: var(--fs-sm);
  }
  .confirm p {
    margin: 0;
    line-height: 1.5;
  }
  .dl {
    margin: 0;
    padding-left: 20px;
    color: var(--danger);
  }
  .hostlist {
    color: var(--text-1);
    font-size: var(--fs-xs);
  }
</style>
