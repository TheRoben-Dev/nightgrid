<script lang="ts">
  /**
   * DRIFT's side panel: conversation, attached context (removable), and the
   * exact prompt behind each reply. Replies render through the markdown
   * subset in helper/md.ts as text nodes only.
   */
  import { tick } from "svelte";
  import { writeClipboard } from "../../clipboard";
  import { parseMd } from "../../helper/md";
  import { formatChord, DEFAULT_KEYMAP } from "../../keys";
  import { scramble } from "../../scramble";
  import { sectionById } from "../../sections";
  import { helper, type ChatMessage } from "../../stores/helper.svelte";
  import { notes } from "../../stores/notes.svelte";
  import { ui } from "../../stores/ui.svelte";

  function openNote(id: number) {
    notes.draft = null;
    notes.selected = id;
    ui.go("notes");
  }
  import Drift from "./Drift.svelte";
  import ModelCard from "./ModelCard.svelte";

  let input = $state("");
  let list: HTMLDivElement | undefined = $state();
  let field: HTMLTextAreaElement | undefined = $state();
  let copied = $state<string | null>(null);

  const SUGGESTIONS = ["How do I add a host?", "Explain the selected log lines", "How do I back up my setup?"];
  /** Skills put a question in the box to finish, with the caret where the user types. */
  const SKILLS: { label: string; text: string; caret?: number }[] = [
    { label: "Is this command safe?", text: "Is it safe to run ``?", caret: 19 },
    { label: "Review a compose file", text: "Review this compose file: " },
    { label: "What does this error mean?", text: "What does this error mean? " },
    { label: "Cron schedule", text: "Make a cron schedule for weekdays at 6pm" },
  ];
  const LEVEL = { danger: "DANGER", caution: "CAUTION", info: "NOTE" } as const;

  async function fill(text: string, caret = text.length) {
    input = text;
    await tick();
    field?.focus();
    field?.setSelectionRange(caret, caret);
  }

  // Follow the reply as it streams.
  $effect(() => {
    const last = helper.messages[helper.messages.length - 1];
    void last?.text;
    void helper.messages.length;
    if (list) void tick().then(() => list && (list.scrollTop = list.scrollHeight));
  });

  // A view's Explain button can leave a question waiting.
  $effect(() => {
    if (helper.draft) {
      input = helper.draft;
      helper.draft = "";
    }
  });

  $effect(() => {
    if (helper.open && helper.enabled) void tick().then(() => field?.focus());
  });

  function send(text = input) {
    if (!text.trim() || helper.busy) return;
    void helper.ask(text);
    input = "";
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    } else if (e.key === "Escape") {
      if (helper.busy) helper.stop();
      else helper.toggle(false);
    }
  }

  async function copy(text: string) {
    await writeClipboard(text);
    copied = text;
    setTimeout(() => copied === text && (copied = null), 1200);
  }

  const lastUser = (m: ChatMessage) => m.sent?.[m.sent.length - 1]?.content ?? "";
  const toggleKey = formatChord(DEFAULT_KEYMAP["helper.toggle"]);
</script>

<aside class="helper" aria-label="DRIFT helper">
  <header>
    <Drift mood={helper.drift} scale={2} />
    <div class="title">
      <span class="label" use:scramble={"DRIFT"}></span>
      <span class="micro">
        {#if !helper.enabled}OFF{:else if helper.loading}LOADING MODEL…{:else if helper.busy}THINKING{:else if helper.ready}LOCAL · {helper.status?.gpu ? "GPU" : "CPU"}{:else}NO MODEL{/if}
      </span>
    </div>
    <div class="tools">
      {#if helper.messages.length}<button type="button" class="tool" title="New conversation" disabled={helper.busy} onclick={() => helper.clear()}>NEW</button>{/if}
      <button type="button" class="tool" title="Close ({toggleKey})" aria-label="Close helper" onclick={() => helper.toggle(false)}>✕</button>
    </div>
  </header>

  {#if !helper.enabled}
    <div class="intro">
      <p>I answer questions about Nightgrid and your servers, explain logs and errors, and can point at the control you're looking for.</p>
      <p class="muted small">I run entirely on this computer with a small local model (about 2.5 GB, downloaded once). Nothing you ask leaves the machine.</p>
      <button type="button" class="btn primary" onclick={() => void helper.setEnabled(true)}>Turn on DRIFT</button>
    </div>
  {:else if !helper.ready}
    <div class="intro">
      <p>One more step: download the model.</p>
      <ModelCard />
    </div>
  {:else}
    <div class="msgs" bind:this={list} aria-live="polite">
      {#if !helper.messages.length}
        <div class="empty">
          <p class="muted">Ask me anything about Nightgrid, or select log lines or an error and ask what it means.</p>
          <div class="chips">
            {#each SUGGESTIONS as s (s)}<button type="button" class="chip" onclick={() => send(s)}>{s}</button>{/each}
          </div>
          <p class="muted small">Exact checks I run before answering (shown above my answer):</p>
          <div class="chips" data-guide="drift-skills">
            {#each SKILLS as k (k.label)}<button type="button" class="chip skill" onclick={() => void fill(k.text, k.caret)}>✦ {k.label}</button>{/each}
          </div>
        </div>
      {/if}
      {#each helper.messages as m (m.id)}
        <div class="msg {m.role}">
          {#if m.role === "user"}
            <p class="q">{m.text}</p>
            {#if m.attached}<p class="att micro">+ {m.attached.toUpperCase()}</p>{/if}
          {:else}
            {#if m.checks?.length}
              <div class="checks" aria-label="Checks Nightgrid ran">
                {#each m.checks as c, ci (ci)}
                  <p class="ct micro">✓ {c.title}</p>
                  <ul>
                    {#each c.items as it, ii (ii)}<li class="lv-{it.level ?? 'none'}">{#if it.level}<span class="lvl">{LEVEL[it.level]}</span>{/if}{it.text}</li>{/each}
                  </ul>
                {/each}
              </div>
            {/if}
            {#if !m.text && m.streaming}
              <p class="muted thinking">{helper.loading ? "Loading the model (first question only)…" : "Thinking…"}</p>
            {/if}
            {#each parseMd(m.text) as b, bi (bi)}
              {#if b.kind === "code"}
                <div class="code">
                  <pre><code>{b.text}</code></pre>
                  {#if !b.open}<button type="button" class="copy" onclick={() => void copy(b.text)}>{copied === b.text ? "COPIED" : "COPY"}</button>{/if}
                </div>
              {:else if b.kind === "p"}
                <p>
                  {#each b.lines as line, li (li)}{#if li}<br />{/if}{#each line as t, ti (ti)}{#if t.kind === "code"}<code>{t.text}</code>{:else if t.kind === "bold"}<b>{t.text}</b>{:else if t.kind === "em"}<i>{t.text}</i>{:else}{t.text}{/if}{/each}{/each}
                </p>
              {:else if b.kind === "ul" || b.kind === "ol"}
                <svelte:element this={b.kind}>
                  {#each b.items as item, ii (ii)}<li>{#each item as t, ti (ti)}{#if t.kind === "code"}<code>{t.text}</code>{:else if t.kind === "bold"}<b>{t.text}</b>{:else if t.kind === "em"}<i>{t.text}</i>{:else}{t.text}{/if}{/each}</li>{/each}
                </svelte:element>
              {/if}
            {/each}
            {#if m.error}<p class="error" role="alert">{m.error}</p>{/if}
            {#if m.actions.length && !m.streaming}
              <div class="acts">
                <button type="button" class="btn primary" onclick={() => void helper.run(m.actions)}>Show me</button>
                {#each m.actions as a, ai (ai)}
                  {#if a.kind === "point"}<button type="button" class="chip" onclick={() => void helper.point(a.id, a.note)}>{a.note || a.id}</button>
                  {:else}<span class="muted small">→ {sectionById(a.view).label}</span>{/if}
                {/each}
              </div>
            {/if}
            {#if !m.streaming && m.noteRefs?.length}
              <div class="acts notes" aria-label="Notes used">
                <span class="muted small">From your notes:</span>
                {#each m.noteRefs as r (r.id)}<button type="button" class="chip" onclick={() => openNote(r.id)}>▤ {r.title}</button>{/each}
              </div>
            {/if}
            {#if !m.streaming && m.sent}
              <details class="sent">
                <summary>What I was given{m.sources?.length ? ` · ${m.sources.join(", ")}` : ""}{m.stats ? ` · ${m.stats}` : ""}</summary>
                <pre>{lastUser(m)}</pre>
              </details>
            {/if}
          {/if}
        </div>
      {/each}
    </div>

    <form class="ask" onsubmit={(e) => (e.preventDefault(), send())}>
      <div class="ctx" aria-label="Context sent with your question">
        <span class="tag">{sectionById(helper.context.view).label.toUpperCase()}</span>
        {#if helper.context.host}<button type="button" class="tag rm" title="Don't send" onclick={() => helper.dropContext("host")}>{helper.context.host} ✕</button>{/if}
        {#if helper.context.selection}<button type="button" class="tag rm sel" title="Don't send" onclick={() => helper.dropContext("selection")}
            >{helper.context.source ? helper.context.source.toUpperCase() : "SELECTION"} · {helper.context.selection.split("\n").length} line{helper.context.selection.split("\n").length === 1 ? "" : "s"} ✕</button
          >{/if}
        <button type="button" class="tag ghost" title="Use the current view and selection" onclick={() => helper.captureContext()}>↻</button>
      </div>
      <div class="row">
        <textarea
          bind:this={field}
          bind:value={input}
          rows="2"
          placeholder="Ask DRIFT…"
          aria-label="Ask DRIFT"
          onkeydown={onkey}
          onfocus={() => (helper.typing = true)}
          onblur={() => (helper.typing = false)}
        ></textarea>
        {#if helper.busy}
          <button type="button" class="btn" onclick={() => helper.stop()}>Stop</button>
        {:else}
          <button type="submit" class="btn primary" disabled={!input.trim()}>Ask</button>
        {/if}
      </div>
      <p class="muted micro hint">Answers can be wrong. I never run commands; Nightgrid asks before anything destructive.</p>
    </form>
  {/if}
</aside>

<style>
  .helper {
    width: 380px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-left: 1px solid var(--line);
    background: var(--bg-1);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
  }
  .title {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title .label {
    color: var(--text-0);
    letter-spacing: 0.2em;
    font-weight: 700;
  }
  .micro {
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--text-2);
  }
  .tools {
    display: flex;
    gap: 4px;
  }
  .tool {
    min-height: var(--hit);
    padding: 2px 8px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    cursor: pointer;
  }
  .tool:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text-0);
  }
  .intro {
    padding: 14px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .intro p {
    margin: 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .checks {
    border: 1px solid var(--line);
    border-left: 2px solid var(--cyan);
    padding: 6px 8px;
    margin-bottom: 8px;
    background: color-mix(in srgb, var(--cyan) 4%, transparent);
  }
  .checks .ct {
    margin: 4px 0 2px;
    color: var(--cyan);
    letter-spacing: 0.1em;
  }
  .checks ul {
    margin: 0 0 4px;
    padding-left: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .checks li {
    font-size: var(--fs-xs);
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  .lvl {
    display: inline-block;
    min-width: 7ch;
    margin-right: 6px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .lv-danger .lvl {
    color: var(--danger);
  }
  .lv-caution .lvl {
    color: var(--warn);
  }
  .lv-info .lvl {
    color: var(--text-1);
  }
  .chip.skill {
    color: var(--cyan);
  }
  .msgs {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .empty .chips,
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  .chip {
    padding: 4px 8px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    text-align: left;
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  .chip:hover {
    border-color: var(--magenta);
    color: var(--text-0);
  }
  .msg p,
  .msg :global(ul),
  .msg :global(ol) {
    margin: 0 0 6px;
    overflow-wrap: anywhere;
  }
  .msg :global(ul),
  .msg :global(ol) {
    padding-left: 18px;
  }
  .msg.user .q {
    padding: 6px 10px;
    border-left: 2px solid var(--magenta);
    color: var(--text-0);
    white-space: pre-wrap;
  }
  .att {
    margin: 2px 0 0 12px;
    color: var(--cyan);
    opacity: 0.8;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg code {
    color: var(--cyan);
    font-family: var(--font-mono);
  }
  .code {
    position: relative;
    margin: 0 0 6px;
  }
  .code pre {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    overflow: auto;
    font-size: var(--fs-sm);
  }
  .copy {
    position: absolute;
    top: 4px;
    right: 4px;
    padding: 1px 6px;
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    background: var(--bg-1);
    border: 1px solid var(--line);
    color: var(--text-1);
    cursor: pointer;
  }
  .thinking {
    margin: 0;
  }
  .error {
    color: var(--danger);
  }
  .sent {
    margin-top: 4px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .sent summary {
    cursor: pointer;
  }
  .sent pre {
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    background: var(--bg-0);
    border: 1px solid var(--line);
    padding: 6px 8px;
  }
  .ask {
    border-top: 1px solid var(--line);
    padding: 8px 12px 10px;
  }
  .ctx {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  .tag {
    padding: 1px 6px;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    border: 1px solid var(--line);
    color: var(--cyan);
    background: transparent;
  }
  .tag.rm {
    cursor: pointer;
  }
  .tag.rm:hover {
    border-color: var(--danger);
  }
  .tag.sel {
    color: var(--magenta);
  }
  .tag.ghost {
    color: var(--text-2);
    cursor: pointer;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: flex-end;
  }
  textarea {
    flex: 1;
    resize: none;
    min-height: 44px;
    max-height: 160px;
    padding: 6px 8px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    color: var(--text-0);
    font: inherit;
  }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .hint {
    margin: 6px 0 0;
    letter-spacing: 0;
  }
</style>
