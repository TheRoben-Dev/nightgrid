<script lang="ts">
  /**
   * Fleet → UPS (M10s): UPSes read with NUT's `upsc` on the host they're
   * plugged into, alerts when one goes on battery, and an optional shutdown
   * of a host group after a countdown you can cancel.
   */
  import { api, errorText, type UpsReading, type UpsStatus, type UpsWatch } from "../../api";
  import { agoText } from "../../fleet";
  import { alerts } from "../../stores/alerts.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { blankUps, needsTyped, triggerText, upsState, upsTone, upsWords, validTarget, type UpsForm } from "../../ups";
  import Modal from "../Modal.svelte";
  import SegmentSlider from "../SegmentSlider.svelte";
  import Toggle from "../Toggle.svelte";

  let rows = $state<UpsStatus[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let now = $state(Date.now());

  async function load() {
    try {
      rows = await api.upsStatus();
      error = null;
    } catch (e) {
      error = errorText(e);
    }
    loaded = true;
  }
  $effect(() => {
    void load();
    // The alerts loop reads every 30 s; follow it while the tab is open.
    const t = setInterval(() => {
      now = Date.now();
      void load();
    }, 10_000);
    return () => clearInterval(t);
  });

  const hostName = (id: number) => hosts.hosts.find((h) => h.id === id)?.name ?? `host ${id}`;
  const groupName = (id: number | null) => (id == null ? null : (hosts.groups.find((g) => g.id === id)?.name ?? `group ${id}`));
  const members = (id: number | null) => (id == null ? [] : hosts.hosts.filter((h) => h.groupId === id));

  // Editor
  let editing = $state<UpsWatch | null | undefined>(undefined); // undefined: closed, null: new
  let form = $state<UpsForm>(blankUps(0));
  let typed = $state("");
  let test = $state<UpsReading | null>(null);
  let testing = $state(false);
  let busy = $state(false);
  let formError = $state<string | null>(null);
  let confirmDelete = $state(false);

  function edit(w: UpsWatch | null) {
    editing = w;
    form = w ? { ...w } : blankUps(hosts.hosts[0]?.id ?? 0);
    typed = "";
    test = null;
    formError = null;
    confirmDelete = false;
  }

  const SECS_BATTERY = [0, 60, 120, 300, 600, 900, 1800, 3600] as const;
  const SECS_RUNTIME = [0, 120, 180, 300, 600, 900, 1200] as const;
  const SECS_COUNTDOWN = [30, 60, 120, 300, 600] as const;
  const word = (v: number) => (v === 0 ? "off" : upsWords(v));

  const group = $derived(groupName(form.groupId));
  const typedNeeded = $derived(needsTyped(editing, form));
  const valid = $derived(
    form.label.trim().length > 0 &&
      validTarget(form.target.trim()) &&
      form.hostId > 0 &&
      (!form.shutdown || form.groupId != null) &&
      (!typedNeeded || typed.trim() === group),
  );

  async function runTest() {
    testing = true;
    test = null;
    formError = null;
    try {
      test = await api.upsReadNow(form.hostId, form.target.trim());
      if (!form.label.trim() && test.model) form.label = test.model;
    } catch (e) {
      formError = errorText(e);
    }
    testing = false;
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!valid) return;
    busy = true;
    formError = null;
    try {
      await api.upsSave({ ...form, label: form.label.trim(), target: form.target.trim(), id: editing?.id ?? null });
      editing = undefined;
      await load();
    } catch (err) {
      formError = errorText(err);
    }
    busy = false;
  }

  async function remove(w: UpsWatch) {
    editing = undefined;
    try {
      await api.upsDelete(w.id);
    } catch (e) {
      error = errorText(e);
    }
    await load();
  }

  const countdown = $derived(alerts.snap?.upsCountdown ?? null);
</script>

<div class="ups" data-guide="fleet-ups">
  <div class="bar">
    <button type="button" class="btn ghost tiny" data-guide="fleet-ups-add" onclick={() => edit(null)}>+ UPS</button>
    <span class="muted small">Read with NUT (<code>upsc</code>) every 30 s while the host is connected.</span>
    <span class="spacer"></span>
    {#if countdown}<span class="sum bad">SHUTDOWN COUNTDOWN RUNNING</span>{/if}
  </div>
  {#if error}<p class="bad small" role="alert">{error}</p>{/if}

  {#if loaded && !rows.length}
    <div class="empty">
      <p class="label">NO UPS YET</p>
      <p class="muted">
        Plug the UPS into a host by USB and install NUT there (<code>nut</code> or <code>nut-client</code>; a NAS often has a built-in NUT server). Then add it here to get an alert when the power goes, and optionally shut a group of
        hosts down cleanly before the battery runs out.
      </p>
      <button type="button" class="btn primary" onclick={() => edit(null)}>Add a UPS</button>
    </div>
  {/if}

  <div class="scroll">
    {#each rows as s (s.watch.id)}
      {@const w = s.watch}
      {@const r = s.reading}
      {@const tone = w.enabled ? upsTone(r) : "unknown"}
      <section class="card t-{tone}">
        <header>
          <span class="name">{w.label}</span>
          <span class="muted small">{w.target} on {hostName(w.hostId)}</span>
          {#if !w.enabled}<span class="chip unknown">PAUSED</span>{:else}<span class="chip {tone}">{upsState(r)}</span>{/if}
          {#if r?.replaceBattery}<span class="chip warn" title="The UPS reports RB: replace the battery">REPLACE BATTERY</span>{/if}
          <span class="spacer"></span>
          <span class="muted small">{s.readAt ? `read ${agoText(s.readAt * 1000, now)}` : "not read yet (host not connected)"}</span>
          <button type="button" class="btn ghost tiny" onclick={() => edit(w)}>Edit</button>
        </header>
        {#if r?.error}
          <p class="issue warn">{r.error}</p>
        {:else if r}
          <div class="gauges">
            <div class="gauge">
              <span class="k">CHARGE</span>
              <span class="meter"><span class="fill t-{tone}" style="width:{r.charge ?? 0}%"></span></span>
              <span class="v">{r.charge != null ? `${Math.round(r.charge)}%` : "?"}</span>
            </div>
            <div class="gauge">
              <span class="k">RUNTIME</span>
              <span class="v">{r.runtime != null ? upsWords(r.runtime) : "?"}</span>
            </div>
            <div class="gauge">
              <span class="k">LOAD</span>
              <span class="v">{r.load != null ? `${Math.round(r.load)}%` : "?"}</span>
            </div>
            {#if r.inputVoltage != null}
              <div class="gauge">
                <span class="k">INPUT</span>
                <span class="v">{Math.round(r.inputVoltage)} V</span>
              </div>
            {/if}
            {#if r.model}<span class="muted small model">{r.model}</span>{/if}
          </div>
          {#if s.onBatterySince}<p class="issue warn">On battery for {upsWords(Math.max(0, now / 1000 - s.onBatterySince))}.</p>{/if}
        {/if}
        <p class="muted small plan">
          {triggerText(w, groupName(w.groupId) ?? undefined)}
          {#if w.shutdown && w.groupId != null}
            Order: {[...members(w.groupId).filter((h) => h.id !== w.hostId), ...members(w.groupId).filter((h) => h.id === w.hostId)].map((h) => h.name).join(" → ") || "no hosts in the group"}.
          {/if}
        </p>
      </section>
    {/each}
  </div>
</div>

<Modal open={editing !== undefined} title={editing ? `EDIT :: ${editing.label}` : "WATCH A UPS"} width={600} onclose={() => (editing = undefined)}>
  {#if confirmDelete && editing}
    <p>Stop watching <strong>{editing.label}</strong>? Its alerts stop and it can no longer shut anything down.</p>
  {:else}
    <form class="grid" id="ups-form" onsubmit={save}>
      <label class="field span3">
        <span>Host it's plugged into</span>
        <select class="input" bind:value={form.hostId}>
          {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
        </select>
        <span class="field-hint">Where <code>upsc</code> runs. A NUT server elsewhere works too: <code>ups@nas.lan</code>.</span>
      </label>
      <label class="field span3">
        <span>NUT name</span>
        <span class="row">
          <input class="input" bind:value={form.target} placeholder="ups@localhost" autocomplete="off" spellcheck="false" />
          <button type="button" class="btn ghost tiny" data-guide="fleet-ups-test" disabled={testing || !validTarget(form.target.trim()) || !form.hostId} onclick={runTest}>{testing ? "Reading…" : "Test"}</button>
        </span>
        <span class="field-hint">Run <code>upsc -l</code> on the host to list them.</span>
      </label>
      {#if test}
        <p class="span6 test t-{upsTone(test)}">
          {#if test.error}
            {test.error}
          {:else}
            {upsState(test)} · {test.charge != null ? `${Math.round(test.charge)}%` : "?"} · {test.runtime != null ? upsWords(test.runtime) : "?"} left{test.model ? ` · ${test.model}` : ""}
          {/if}
        </p>
      {/if}
      <label class="field span6">
        <span>Label</span>
        <input class="input" bind:value={form.label} placeholder="Rack UPS" autocomplete="off" maxlength="60" />
      </label>
      <div class="span6">
        <Toggle label="Watch it" hint="Paused watches aren't read and never shut anything down." checked={form.enabled} onchange={(v) => (form.enabled = v)} />
        <Toggle
          label="Shut a group down when the power stays out"
          hint="Every host in the group is told to shut down, the UPS's own host last. Hosts need passwordless sudo for shutdown (or a remembered sudo password)."
          checked={form.shutdown}
          onchange={(v) => (form.shutdown = v)}
        />
      </div>
      {#if form.shutdown}
        <label class="field span6">
          <span>Group</span>
          <select class="input" value={form.groupId ?? ""} onchange={(e) => (form.groupId = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
            <option value="">Pick a group…</option>
            {#each hosts.groups as g (g.id)}<option value={g.id}>{g.name} ({members(g.id).length} host{members(g.id).length === 1 ? "" : "s"})</option>{/each}
          </select>
          {#if !hosts.groups.length}<span class="field-hint">Make a host group first (edit a host → Group).</span>{/if}
        </label>
        <div class="span6 sliders">
          <SegmentSlider label="On battery for" steps={SECS_BATTERY} value={form.onBatterySecs} format={word} onchange={(v) => (form.onBatterySecs = v)} />
          <SegmentSlider label="Runtime under" steps={SECS_RUNTIME} value={form.lowRuntimeSecs} format={word} onchange={(v) => (form.lowRuntimeSecs = v)} />
          <SegmentSlider label="Countdown" steps={SECS_COUNTDOWN} value={form.countdownSecs} format={upsWords} onchange={(v) => (form.countdownSecs = v)} />
        </div>
        <p class="span6 muted small">{triggerText(form, group ?? undefined)} Low battery reported by the UPS always counts.</p>
        {#if typedNeeded && group}
          <label class="field span6 arm">
            <span>Type <strong>{group}</strong> to let this UPS shut down {members(form.groupId).map((h) => h.name).join(", ") || "its hosts"}</span>
            <input class="input" bind:value={typed} placeholder={group} autocomplete="off" spellcheck="false" />
          </label>
        {/if}
      {/if}
    </form>
  {/if}
  {#if formError}<p class="error-text" role="alert">{formError}</p>{/if}
  {#snippet footer()}
    {#if confirmDelete && editing}
      <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      <button type="button" class="btn danger" onclick={() => editing && void remove(editing)}>Stop watching {editing.label}</button>
    {:else}
      {#if editing}<button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Remove…</button>{/if}
      <button type="button" class="btn ghost" onclick={() => (editing = undefined)}>Cancel</button>
      <button type="submit" form="ups-form" class="btn primary" disabled={!valid || busy}>{editing ? "Save" : "Watch it"}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .ups {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .bar,
  header {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .sum {
    letter-spacing: 0.1em;
  }
  .bad,
  .sum.bad {
    color: var(--danger);
  }
  .scroll {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    max-width: 60ch;
    padding: 12px 0;
  }
  .card {
    border: 1px solid var(--line);
    border-left: 3px solid var(--line);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .card.t-ok {
    border-left-color: var(--ok);
  }
  .card.t-warn {
    border-left-color: var(--warn);
  }
  .card.t-bad {
    border-left-color: var(--danger);
  }
  .name {
    color: var(--text-0);
    font-size: var(--fs-sm);
  }
  .chip {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    padding: 0 6px;
    border: 1px solid currentColor;
  }
  .chip.ok {
    color: var(--ok);
  }
  .chip.warn {
    color: var(--warn);
  }
  .chip.bad {
    color: var(--danger);
  }
  .chip.unknown {
    color: var(--text-1);
  }
  .gauges {
    display: flex;
    gap: 18px;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .gauge {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .k {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.08em;
  }
  .v {
    color: var(--text-0);
    font-variant-numeric: tabular-nums;
  }
  .meter {
    display: inline-block;
    width: 90px;
    height: 6px;
    background: var(--bg-0);
    border: 1px solid var(--line);
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--ok);
  }
  .fill.t-warn {
    background: var(--warn);
  }
  .fill.t-bad {
    background: var(--danger);
  }
  .fill.t-unknown {
    background: var(--text-1);
  }
  .model {
    margin-left: auto;
  }
  .issue {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .issue.warn {
    color: var(--warn);
  }
  .plan {
    margin: 0;
  }
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
  .row {
    display: flex;
    gap: 6px;
  }
  .row .input {
    flex: 1;
    min-width: 0;
  }
  .test {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .test.t-ok {
    color: var(--ok);
  }
  .test.t-warn,
  .test.t-unknown {
    color: var(--warn);
  }
  .test.t-bad {
    color: var(--danger);
  }
  .sliders {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .arm span {
    color: var(--danger);
  }
  .error-text {
    margin: 8px 0 0;
  }
  :global(footer) .left {
    margin-right: auto;
  }
</style>
