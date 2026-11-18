<script lang="ts">
  /**
   * Fleet → BLUEPRINTS → STARTERS (D-175): ready-made setups for common jobs.
   * Pick one, read what it does, fill in its questions, preview the plan on a
   * host, and apply only after confirming. Uses the blueprint plan/apply code.
   */
  import { api, errorText, type BpStep, type BpStepResult, type StarterBlueprint } from "../../api";
  import { initialPick, resultsLine } from "../../blueprint";
  import { CATALOG } from "../../kb/catalog";
  import { categoryLabel, confirmLine, images, initialValues, portsOf, toValues, valueKey, valuesProblem } from "../../starters";
  import { hosts } from "../../stores/hosts.svelte";
  import { library } from "../../stores/library.svelte";
  import { notes } from "../../stores/notes.svelte";
  import { starters } from "../../stores/starters.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import PlanSteps from "./PlanSteps.svelte";

  const catalog = $derived(starters.catalog);
  const loadErr = $derived(starters.error);
  let sel = $state<StarterBlueprint | null>(null);
  let showFiles = $state(false);

  let hostId = $state<number | null>(null);
  let parent = $state("");
  let parentWhy = $state("");
  let values = $state<Record<string, string>>({});
  let reveal = $state<Record<string, boolean>>({});

  let plan = $state<BpStep[] | null>(null);
  let picked = $state<Set<string>>(new Set());
  let results = $state<BpStepResult[] | null>(null);
  let confirming = $state(false);
  let busy = $state(false);
  let err = $state<string | null>(null);

  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client"));
  const nameOf = (id: number | null) => hosts.hosts.find((h) => h.id === id)?.name ?? "";
  const articleTitle = (id: string) => CATALOG.find((e) => e.id === id)?.title ?? id;
  function containers(s: StarterBlueprint): string {
    const n = s.stacks.flatMap((st) => images(st.compose)).length;
    return n ? `${n} container${n === 1 ? "" : "s"}` : "no containers";
  }
  const problem = $derived(sel ? valuesProblem(sel, values, parent) : null);

  $effect(() => {
    void starters.load();
  });

  function pick(s: StarterBlueprint) {
    sel = s;
    values = initialValues(s);
    reveal = {};
    showFiles = false;
    resetPlan();
  }

  function resetPlan() {
    plan = null;
    results = null;
    confirming = false;
    err = null;
  }

  async function pickHost(id: number | null) {
    hostId = id;
    resetPlan();
    if (id == null) return;
    parent = "";
    parentWhy = "";
    try {
      const t = await api.starterTarget(id);
      if (hostId === id && !parent) {
        parent = t.parent;
        parentWhy = t.why;
      }
    } catch (e) {
      parentWhy = `${errorText(e)}`;
    }
  }

  async function preview() {
    if (!sel || hostId == null || problem) return;
    const id = hostId;
    busy = true;
    err = null;
    results = null;
    confirming = false;
    try {
      await api.starterOpen(sel.id, toValues($state.snapshot(values)), parent.trim());
      plan = await system.withSudo("read the firewall to plan the starter", (sudo) => api.blueprintPlan(id, sudo), nameOf(id));
      picked = initialPick(plan);
    } catch (e) {
      err = errorText(e);
    }
    busy = false;
  }

  async function apply() {
    if (hostId == null || !plan || !picked.size) return;
    const id = hostId;
    const ids = plan.filter((s) => picked.has(s.id)).map((s) => s.id);
    busy = true;
    confirming = false;
    err = null;
    try {
      results = await system.withSudo("apply the starter", (sudo) => api.blueprintApply(id, ids, sudo), nameOf(id));
    } catch (e) {
      err = errorText(e);
    }
    busy = false;
  }

  async function back() {
    sel = null;
    resetPlan();
    await api.blueprintClose().catch(() => null);
  }
</script>

<div class="gal" data-guide="fleet-starters">
  {#if !sel}
    <p class="muted small intro">Ready-made setups for common jobs. Each one explains what it's for, asks a few questions (secrets go into the <code>.env</code> vault, never into the starter), shows exactly what it would do on a host, and changes nothing until you confirm. Starter runbooks for alerts are in <button type="button" class="link" onclick={() => ((notes.starters = true), ui.go("notes"))}>Knowledge → Starters</button>.</p>
    {#if loadErr}<p class="bad small" role="alert">{loadErr}</p>{/if}
    {#if !catalog && !loadErr}<p class="muted small">Loading…</p>{/if}
    <div class="cards">
      {#each catalog?.blueprints ?? [] as s (s.id)}
        <button type="button" class="card" onclick={() => pick(s)}>
          <span class="cat">{categoryLabel(s.category).toUpperCase()}</span>
          <span class="ttl">{s.title}</span>
          <span class="sum">{s.summary}</span>
          <span class="meta">{containers(s)}{#if s.firewall}&nbsp;· firewall{/if}{#if s.timers.length}&nbsp;· {s.timers.length} timer{s.timers.length === 1 ? "" : "s"}{/if}</span>
        </button>
      {/each}
    </div>
  {:else}
    {@const ports = portsOf(sel)}
    <div class="detail">
      <section class="col">
        <div class="row">
          <button type="button" class="btn ghost tiny" onclick={() => void back()}>← All starters</button>
          <span class="cat">{categoryLabel(sel.category).toUpperCase()}</span>
        </div>
        <h3 class="big">{sel.title}</h3>
        {#each sel.purpose.trim().split(/\n\s*\n/) as para (para)}<p class="small para">{para.replace(/\s*\n\s*/g, " ")}</p>{/each}

        <h4 class="label">WHAT IT SETS UP</h4>
        <ul class="small list">
          {#each sel.stacks as st (st.name)}
            <li><span class="k">STACK</span> {st.name} <span class="muted">{images(st.compose).join(", ")}</span></li>
          {/each}
          {#if ports.lan.length}<li><span class="k">ON LAN</span> {ports.lan.join(" · ")}</li>{/if}
          {#if ports.local.length}<li><span class="k">HOST ONLY</span> {ports.local.join(" · ")} <span class="muted">(open from Docker → web UIs)</span></li>{/if}
          {#if sel.firewall}
            <li><span class="k">FIREWALL</span> {[...sel.firewall.allow.map((p) => `allow ${p}`), ...sel.firewall.limit.map((p) => `limit ${p}`)].join(", ") || "no ports"}{#if sel.firewall.denyIncoming || sel.firewall.enable}<span class="muted"> · deny-by-default and turning it on stay manual, behind System → FIREWALL's rollback</span>{/if}</li>
          {/if}
          {#each sel.timers as t (t.name)}<li><span class="k">TIMER</span> {t.name} <span class="muted">{t.calendar} · {t.command}</span></li>{/each}
          {#each sel.manual as m (m.title)}<li><span class="k">BY HAND</span> {m.title}</li>{/each}
        </ul>

        <h4 class="label">LIBRARY</h4>
        <div class="chips">
          {#each sel.library as a (a)}<button type="button" class="chip" onclick={() => library.open(a)}>▤ {articleTitle(a)}</button>{/each}
        </div>

        {#if sel.stacks.length}
          <button type="button" class="link small" aria-expanded={showFiles} onclick={() => (showFiles = !showFiles)}>{showFiles ? "Hide" : "Show"} the files it writes</button>
          {#if showFiles}
            {#each sel.stacks as st (st.name)}
              <p class="small muted">{st.name}/compose.yaml</p>
              <pre class="code">{st.compose}</pre>
              {#each st.files as f (f.path)}
                <p class="small muted">{st.name}/{f.path}</p>
                <pre class="code">{f.text || "(empty)"}</pre>
              {/each}
            {/each}
          {/if}
        {/if}
      </section>

      <section class="col">
        <h4 class="label">SET IT UP</h4>
        <label class="field">Host
          <select class="input" value={hostId} onchange={(e) => void pickHost((e.currentTarget as HTMLSelectElement).value === "" ? null : Number((e.currentTarget as HTMLSelectElement).value))}>
            <option value="">pick a host…</option>
            {#each servers as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
          </select>
        </label>
        {#if hostId != null}
          {#if sel.stacks.length}
            <label class="field">Folder for the stacks
              <input class="input" bind:value={parent} oninput={resetPlan} placeholder="/opt/stacks" spellcheck="false" />
              {#if parentWhy}<span class="hint">{parentWhy}. Each stack gets its own folder inside.</span>{/if}
            </label>
          {/if}
          {#each sel.stacks as st (st.name)}
            {#each st.env as v (v.name)}
              {@const key = valueKey(st.name, v)}
              <label class="field">{v.label}{#if v.generate}<span class="muted"> (optional)</span>{/if}
                <span class="inrow">
                  <input class="input" type={v.secret && !reveal[key] ? "password" : "text"} bind:value={values[key]} oninput={resetPlan} placeholder={v.generate ? "leave empty to generate" : v.default} autocomplete="off" spellcheck="false" />
                  {#if v.secret}<button type="button" class="btn ghost tiny" onclick={() => (reveal[key] = !reveal[key])}>{reveal[key] ? "Hide" : "Show"}</button>{/if}
                </span>
                <span class="hint"><code>{v.name}</code>{#if v.secret}&nbsp;· kept as a secret in the vault{/if}{#if v.help}&nbsp;· {v.help}{/if}</span>
              </label>
            {/each}
          {/each}
          {#if problem}<p class="warn small">{problem}</p>{/if}
          <button type="button" class="btn primary" disabled={!!problem || busy} onclick={() => void preview()}>{busy && !plan ? "Reading the host…" : plan ? "Preview again" : "Preview what it would do"}</button>
          <p class="muted small">Previewing reads {nameOf(hostId)} and lists each step. Nothing is written until you confirm.</p>
        {/if}

        {#if plan}
          <PlanSteps {plan} bind:picked {results} {busy} />
          {#if confirming}
            <div class="confirm" role="alertdialog" aria-label="Confirm">
              <p class="small">{confirmLine(picked.size, nameOf(hostId))}</p>
              <div class="row">
                <button type="button" class="btn primary" disabled={busy} onclick={() => void apply()}>Yes, apply</button>
                <button type="button" class="btn ghost" onclick={() => (confirming = false)}>Cancel</button>
              </div>
            </div>
          {:else}
            <div class="row">
              <span class="muted small">{picked.size} step{picked.size === 1 ? "" : "s"} ticked for {nameOf(hostId)}</span>
              <span class="spacer"></span>
              <button type="button" class="btn primary" disabled={!picked.size || busy} onclick={() => (confirming = true)}>{busy ? "Applying…" : "Apply ticked steps…"}</button>
            </div>
          {/if}
          {#if results}
            <p class="small" class:ok={results.every((r) => r.ok)} class:bad={!results.every((r) => r.ok)}>{resultsLine(results)} Preview again to see what's left; the steps marked BY HAND are yours.</p>
          {/if}
        {/if}
        {#if err}<p class="bad small" role="alert">{err}</p>{/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .gal {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .intro {
    max-width: 80ch;
    margin-bottom: 12px;
    line-height: 1.5;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
    background: var(--bg-1);
    border: 1px solid var(--line);
    color: var(--text-0);
    padding: 10px 12px;
    font: inherit;
    cursor: pointer;
    transition: border-color 0.12s;
  }
  .card:hover,
  .card:focus-visible {
    border-color: var(--cyan);
  }
  .cat {
    color: var(--magenta);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .ttl {
    font-size: var(--fs-md, 15px);
  }
  .sum {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .meta {
    margin-top: auto;
    font-size: var(--fs-xs);
    color: var(--cyan);
  }
  .detail {
    display: grid;
    grid-template-columns: minmax(280px, 1.1fr) minmax(300px, 1fr);
    gap: 18px;
  }
  @media (max-width: 900px) {
    .detail {
      grid-template-columns: 1fr;
    }
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .big {
    margin: 0;
    font-size: var(--fs-lg, 18px);
    color: var(--text-0);
  }
  .label {
    margin: 6px 0 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .para {
    color: var(--text-0);
    line-height: 1.5;
  }
  .muted {
    color: var(--text-1);
  }
  .ok {
    color: var(--ok, #4ade80);
  }
  .bad {
    color: var(--danger);
  }
  .warn {
    color: var(--warn);
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 3px;
  }
  .k {
    display: inline-block;
    min-width: 76px;
    color: var(--cyan);
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 1px 8px;
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text-0);
    border-color: var(--cyan);
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .code {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    overflow: auto;
    max-height: 320px;
    white-space: pre;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .inrow {
    display: flex;
    gap: 4px;
  }
  .inrow .input {
    flex: 1;
    min-width: 0;
  }
  .hint {
    color: var(--text-1);
    opacity: 0.85;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .confirm {
    border: 1px solid var(--warn);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
