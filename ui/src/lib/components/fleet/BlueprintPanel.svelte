<script lang="ts">
  /**
   * Fleet → BLUEPRINTS (0.10.84): save a host's setup as one encrypted file,
   * and rebuild it on another host from a plan of steps you tick.
   */
  import { api, errorText, type BlueprintSummary, type BpInclude, type BpStep, type BpStepResult, type Stack } from "../../api";
  import { initialPick, passphraseProblem, resultsLine } from "../../blueprint";
  import PlanSteps from "./PlanSteps.svelte";
  import StarterGallery from "./StarterGallery.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";

  let mode = $state<"starters" | "files">("starters");

  // ---- make ----
  let srcId = $state<number | null>(null);
  let stacks = $state<Stack[] | null>(null);
  let include = $state<Required<BpInclude>>({ allStacks: true, stacks: [], env: true, cron: true, timers: true, firewall: true, proxy: true });
  let captured = $state<BlueprintSummary | null>(null);
  let pass1 = $state("");
  let pass2 = $state("");
  let saved = $state<string | null>(null);
  let busyMake = $state(false);
  let errMake = $state<string | null>(null);

  // ---- rebuild ----
  let openPass = $state("");
  let opened = $state<BlueprintSummary | null>(null);
  let dstId = $state<number | null>(null);
  let plan = $state<BpStep[] | null>(null);
  let picked = $state<Set<string>>(new Set());
  let results = $state<BpStepResult[] | null>(null);
  let busyBuild = $state(false);
  let errBuild = $state<string | null>(null);

  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client"));
  const nameOf = (id: number | null) => hosts.hosts.find((h) => h.id === id)?.name ?? "";
  const when = (at: number) => new Date(at * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

  async function loadStacks(id: number) {
    stacks = null;
    try {
      stacks = (await api.dockerStacks(id)).filter((s) => s.inFolder);
    } catch {
      stacks = [];
    }
  }
  $effect(() => {
    captured = null;
    saved = null;
    if (srcId != null) void loadStacks(srcId);
  });

  function toggleStack(name: string) {
    include.stacks = include.stacks.includes(name) ? include.stacks.filter((s) => s !== name) : [...include.stacks, name];
  }

  async function capture() {
    if (srcId == null) return;
    const id = srcId;
    busyMake = true;
    errMake = null;
    saved = null;
    try {
      const inc = $state.snapshot(include) as BpInclude;
      captured = await system.withSudo("read the firewall for the blueprint", (sudo) => api.blueprintCapture(id, inc, sudo), nameOf(id));
    } catch (e) {
      errMake = errorText(e);
    }
    busyMake = false;
  }

  const passProblem = $derived(passphraseProblem(pass1, pass2));
  async function save() {
    if (srcId == null || passProblem) return;
    busyMake = true;
    errMake = null;
    try {
      const p = await api.blueprintSave(srcId, pass1);
      if (p) {
        saved = p;
        pass1 = pass2 = "";
        captured = null;
      }
    } catch (e) {
      errMake = errorText(e);
    }
    busyMake = false;
  }

  async function openFile() {
    busyBuild = true;
    errBuild = null;
    try {
      const s = await api.blueprintOpen(openPass);
      if (s) {
        opened = s;
        openPass = "";
        plan = null;
        results = null;
      }
    } catch (e) {
      errBuild = errorText(e);
    }
    busyBuild = false;
  }

  async function makePlan() {
    if (dstId == null) return;
    const id = dstId;
    busyBuild = true;
    errBuild = null;
    results = null;
    try {
      plan = await system.withSudo("read the firewall to plan the rebuild", (sudo) => api.blueprintPlan(id, sudo), nameOf(id));
      picked = initialPick(plan);
    } catch (e) {
      errBuild = errorText(e);
    }
    busyBuild = false;
  }

  async function apply() {
    if (dstId == null || !picked.size) return;
    const id = dstId;
    const ids = plan?.filter((s) => picked.has(s.id)).map((s) => s.id) ?? [];
    busyBuild = true;
    errBuild = null;
    try {
      results = await system.withSudo("apply the blueprint", (sudo) => api.blueprintApply(id, ids, sudo), nameOf(id));
    } catch (e) {
      errBuild = errorText(e);
    }
    busyBuild = false;
  }

  async function closeBlueprint() {
    await api.blueprintClose();
    opened = null;
    plan = null;
    results = null;
  }

</script>

<div class="wrap">
<div class="modes tabstrip sm" role="tablist" aria-label="Blueprints">
  <button type="button" role="tab" class="mode" class:on={mode === "starters"} aria-selected={mode === "starters"} onclick={() => (mode = "starters")}>STARTERS</button>
  <button type="button" role="tab" class="mode" class:on={mode === "files"} aria-selected={mode === "files"} onclick={() => (mode = "files")}>YOUR BLUEPRINTS</button>
</div>
{#if mode === "starters"}
  <StarterGallery />
{:else}
<div class="bp" data-guide="fleet-blueprints">
  <section class="col">
    <h3 class="label">MAKE A BLUEPRINT</h3>
    <p class="muted small">Saves a host's stacks (compose files, <code>.env</code> and its vault values), crontab, Nightgrid timers, firewall rules and proxy routes into one file encrypted with a passphrase.</p>
    <label class="field">Host
      <select class="input" bind:value={srcId}>
        <option value={null}>pick a host…</option>
        {#each servers as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
      </select>
    </label>
    {#if srcId != null}
      <fieldset class="inc">
        <legend class="small">What goes in</legend>
        <label class="check"><input type="checkbox" bind:checked={include.allStacks} /> every stack</label>
        {#if !include.allStacks}
          <div class="chips">
            {#if stacks == null}<span class="muted small">reading stacks…</span>{/if}
            {#each stacks ?? [] as s (s.name)}
              <button type="button" class="chip" class:on={include.stacks.includes(s.name)} aria-pressed={include.stacks.includes(s.name)} onclick={() => toggleStack(s.name)}>{s.name}</button>
            {:else}
              {#if stacks}<span class="muted small">no stacks found in its stack folders</span>{/if}
            {/each}
          </div>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={include.env} /> <code>.env</code> vault values</label>
        <label class="check"><input type="checkbox" bind:checked={include.cron} /> crontab</label>
        <label class="check"><input type="checkbox" bind:checked={include.timers} /> Nightgrid timers</label>
        <label class="check"><input type="checkbox" bind:checked={include.firewall} /> firewall rules (needs sudo)</label>
        <label class="check"><input type="checkbox" bind:checked={include.proxy} /> Caddy and Traefik routes</label>
      </fieldset>
      <button type="button" class="btn primary" disabled={busyMake} onclick={() => void capture()}>{busyMake && !captured ? "Reading…" : captured ? "Read again" : "Read the host"}</button>
    {/if}

    {#if captured}
      {@render summary(captured)}
      <div class="pass">
        <input class="input" type="password" bind:value={pass1} placeholder="passphrase (8+ characters)" autocomplete="new-password" aria-label="Passphrase" />
        <input class="input" type="password" bind:value={pass2} placeholder="again" autocomplete="new-password" aria-label="Passphrase again" />
        <button type="button" class="btn primary" disabled={!!passProblem || busyMake} onclick={() => void save()}>Save blueprint…</button>
      </div>
      {#if pass1 && passProblem}<p class="muted small">{passProblem}</p>{/if}
      <p class="muted small">Without the passphrase the file can't be opened, and it holds your secrets, so keep it like a password-manager export.</p>
    {/if}
    {#if saved}<p class="ok small">Saved to {saved}</p>{/if}
    {#if errMake}<p class="bad small" role="alert">{errMake}</p>{/if}
  </section>

  <section class="col">
    <h3 class="label">REBUILD FROM A BLUEPRINT</h3>
    {#if !opened}
      <p class="muted small">Open a blueprint, pick the host to rebuild on (a fresh machine or any other host), and Nightgrid reads that host and plans each step. Nothing is deleted there; you tick what to apply.</p>
      <div class="pass">
        <input class="input" type="password" bind:value={openPass} placeholder="the blueprint's passphrase" autocomplete="off" aria-label="Blueprint passphrase" />
        <button type="button" class="btn primary" disabled={openPass.length < 8 || busyBuild} onclick={() => void openFile()}>Open blueprint…</button>
      </div>
    {:else}
      {@render summary(opened)}
      <div class="row">
        <label class="field grow">Rebuild on
          <select class="input" bind:value={dstId} onchange={() => ((plan = null), (results = null))}>
            <option value={null}>pick a host…</option>
            {#each servers as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
          </select>
        </label>
        <button type="button" class="btn" disabled={dstId == null || busyBuild} onclick={() => void makePlan()}>{plan ? "Plan again" : "Plan"}</button>
        <button type="button" class="btn ghost" onclick={() => void closeBlueprint()}>Close</button>
      </div>
    {/if}

    {#if plan}
      <PlanSteps {plan} bind:picked {results} busy={busyBuild} />
      <div class="row">
        <span class="muted small">{picked.size} step{picked.size === 1 ? "" : "s"} ticked{#if dstId != null} for {nameOf(dstId)}{/if}</span>
        <span class="spacer"></span>
        <button type="button" class="btn primary" disabled={!picked.size || busyBuild} onclick={() => void apply()}>{busyBuild ? "Applying…" : "Apply ticked steps"}</button>
      </div>
      {#if results}<p class="small" class:ok={results.every((r) => r.ok)} class:bad={!results.every((r) => r.ok)}>{resultsLine(results)} Plan again to see what's left.</p>{/if}
    {/if}
    {#if errBuild}<p class="bad small" role="alert">{errBuild}</p>{/if}
  </section>
</div>
{/if}
</div>

{#snippet summary(s: BlueprintSummary)}
  <div class="sum">
    <p class="small"><strong>{s.host}</strong>{#if s.os}&nbsp;· {s.os}{/if} · {when(s.madeAt)}</p>
    <ul class="small">
      {#each s.stacks as st (st.name)}
        <li><span class="k">STACK</span> {st.name} <span class="muted">{st.dir} · {st.files.length} file{st.files.length === 1 ? "" : "s"}{#if st.env.length}&nbsp;· {st.env.length} vault value{st.env.length === 1 ? "" : "s"}{/if}</span></li>
      {/each}
      {#each s.crontabs as c (c.user)}<li><span class="k">CRON</span> {c.user || "login user"} <span class="muted">{c.lines} line{c.lines === 1 ? "" : "s"}</span></li>{/each}
      {#if s.timers.length}<li><span class="k">TIMERS</span> {s.timers.join(", ")}</li>{/if}
      {#if s.firewall}<li><span class="k">FIREWALL</span> {s.firewall} <span class="muted">{s.firewallRules} rule{s.firewallRules === 1 ? "" : "s"}</span></li>{/if}
      {#each s.proxies as p (p)}<li><span class="k">PROXY</span> {p}</li>{/each}
      {#each s.manual ?? [] as m (m)}<li><span class="k">BY HAND</span> {m}</li>{/each}
    </ul>
    {#each s.notes as n (n)}<p class="warn small">⚠ {n}</p>{/each}
  </div>
{/snippet}

<style>
  .wrap {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .modes {
    flex: none;
    align-self: flex-start;
  }
  .bp {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(280px, 1fr) minmax(320px, 1.3fr);
    gap: 18px;
    min-height: 0;
    overflow: auto;
  }
  @media (max-width: 900px) {
    .bp {
      grid-template-columns: 1fr;
    }
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .label {
    margin: 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
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
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .grow {
    flex: 1;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .inc {
    border: 1px solid var(--line);
    padding: 6px 10px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
  }
  .inc legend {
    color: var(--text-1);
    padding: 0 4px;
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding-left: 22px;
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 0 var(--chip-px);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--chip-h);
    white-space: nowrap;
  }
  .chip.on {
    color: var(--text-0);
    border-color: var(--cyan);
  }
  .pass {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .pass .input {
    flex: 1 1 140px;
    min-width: 0;
  }
  .sum {
    border: 1px solid var(--line);
    background: var(--bg-1);
    padding: 8px 10px;
  }
  .sum ul {
    list-style: none;
    padding: 0;
    margin: 6px 0 0;
    display: grid;
    gap: 2px;
  }
  .k {
    display: inline-block;
    min-width: 66px;
    color: var(--cyan);
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
  }
</style>
