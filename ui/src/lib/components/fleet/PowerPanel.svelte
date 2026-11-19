<script lang="ts">
  /**
   * Fleet → POWER (M12d): what each host draws and costs, estimated from its
   * power profile and CPU history; idle hours worth a power schedule; UPSes
   * that report their output as a check on the estimates.
   */
  import { api, errorText, type HostPower, type PowerOverview } from "../../api";
  import { fmtKwh, fmtW, money, PRESETS, presetOf, profileProblem, RANGES, totals, upsCompare, utcOffsetMins, windowText } from "../../powercost";
  import { settings } from "../../stores/settings.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import Modal from "../Modal.svelte";

  let days = $state<number>(7);
  let data = $state<PowerOverview | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  // Price, edited here and saved to settings on change.
  let price = $state(String(settings.value.power?.pricePerKwh || ""));
  let currency = $state(settings.value.power?.currency ?? "");
  let priceErr = $state<string | null>(null);

  let editing = $state<{ h: HostPower; idle: string; max: string; preset: string; err: string | null; busy: boolean } | null>(null);

  async function load() {
    loading = true;
    try {
      data = await api.powerOverview(days, utcOffsetMins());
      error = null;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    void days;
    void load();
  });

  const p = $derived(settings.value.power?.pricePerKwh ?? 0);
  const cur = $derived(settings.value.power?.currency ?? "");
  const t = $derived(totals(data?.hosts ?? []));
  const sorted = $derived([...(data?.hosts ?? [])].sort((a, b) => (b.avgW ?? -1) - (a.avgW ?? -1) || a.name.localeCompare(b.name)));
  const hints = $derived(sorted.flatMap((h) => h.idle.map((w) => ({ h, w }))).sort((a, b) => b.w.kwhMonth - a.w.kwhMonth));
  const cost = (k: number | null | undefined) => money(k, p, cur);

  async function savePrice() {
    const v = price.trim() === "" ? 0 : Number(price.replace(",", "."));
    if (!Number.isFinite(v) || v < 0 || v > 100) {
      priceErr = "A price per kWh, like 0.30.";
      return;
    }
    priceErr = null;
    try {
      await settings.update({ power: { pricePerKwh: v, currency: currency.trim().slice(0, 8) } });
    } catch (e) {
      priceErr = errorText(e);
    }
  }

  function edit(h: HostPower) {
    const idle = h.profile?.idleW ?? 0;
    const max = h.profile?.maxW ?? 0;
    editing = { h, idle: h.profile ? String(idle) : "", max: h.profile ? String(max) : "", preset: h.profile ? presetOf(idle, max) : "", err: null, busy: false };
  }

  function pickPreset(id: string) {
    if (!editing) return;
    const pr = PRESETS.find((x) => x.id === id);
    editing.preset = id;
    if (pr) {
      editing.idle = String(pr.idle);
      editing.max = String(pr.max);
    }
  }

  async function saveProfile() {
    const e = editing;
    if (!e) return;
    const idle = Number(e.idle.replace(",", "."));
    const max = Number(e.max.replace(",", "."));
    const bad = profileProblem(idle, max);
    if (bad) {
      e.err = bad;
      return;
    }
    e.busy = true;
    try {
      await api.powerProfileSet(e.h.hostId, idle, max);
      editing = null;
      await load();
    } catch (err) {
      e.err = errorText(err);
      e.busy = false;
    }
  }

  async function clearProfile() {
    const e = editing;
    if (!e) return;
    try {
      await api.powerProfileClear(e.h.hostId);
      editing = null;
      await load();
    } catch (err) {
      e.err = errorText(err);
    }
  }

  function schedule(hostId: number) {
    system.select(hostId);
    void system.openTab("schedules");
    ui.go("system");
  }
</script>

<div class="power" data-guide="fleet-power">
  <div class="row">
    <div class="seg tabstrip sm" role="group" aria-label="History used">
      {#each RANGES as r (r)}
        <button type="button" class:on={days === r} aria-pressed={days === r} onclick={() => (days = r)}>{r} DAYS</button>
      {/each}
    </div>
    <span class="sp"></span>
    <label class="field inline" data-guide="power-price"
      ><span class="label">PRICE / kWh</span>
      <input class="input num" bind:value={price} onchange={savePrice} inputmode="decimal" placeholder="0.30" aria-label="Price per kWh" />
      <input class="input cur" bind:value={currency} onchange={savePrice} placeholder="€" maxlength="8" aria-label="Currency" />
    </label>
    <button type="button" class="btn tiny" disabled={loading} onclick={load}>{loading ? "Reading…" : "Refresh"}</button>
  </div>
  {#if priceErr}<p class="error-text small">{priceErr}</p>{/if}
  {#if error}<p class="error-text small">{error}</p>{/if}

  <div class="cards">
    <div class="card"><span class="label">AVERAGE DRAW</span><span class="big data">{fmtW(t.covered ? t.watts : null)}</span><span class="muted small">{t.covered} of {t.hosts} hosts have a profile</span></div>
    <div class="card"><span class="label">PER MONTH</span><span class="big data">{fmtKwh(t.covered ? t.kwhMonth : null)}</span><span class="muted small">{cost(t.kwhMonth) ?? "set a price to see cost"}</span></div>
    <div class="card"><span class="label">PER YEAR</span><span class="big data">{fmtKwh(t.covered ? t.kwhMonth * 12 : null)}</span><span class="muted small">{cost(t.kwhMonth * 12) ?? ""}</span></div>
    <div class="card"><span class="label">IDLE HOURS</span><span class="big data {t.savableKwh > 0 ? 'ok' : ''}">{t.savableKwh > 0 ? fmtKwh(t.savableKwh) : "--"}</span><span class="muted small">{t.savableKwh > 0 ? `a month could be saved${cost(t.savableKwh) ? ` (${cost(t.savableKwh)})` : ""}` : "no reliable idle hours found yet"}</span></div>
  </div>

  <div class="scroll">
    <div class="trow head" aria-hidden="true"><span>HOST</span><span>PROFILE</span><span class="n">CPU</span><span class="n">AVERAGE</span><span class="n">PEAK</span><span class="n">kWh / MONTH</span><span class="n">COST / MONTH</span></div>
    {#each sorted as h (h.hostId)}
      <div class="trow" data-guide="power-host">
        <span class="name">{h.name}</span>
        <span>
          <button type="button" class="link" onclick={() => edit(h)} title="What it draws at idle and at full CPU">{h.profile ? `${h.profile.idleW}–${h.profile.maxW} W` : "Set watts…"}</button>
          {#if h.gpuW}<span class="muted small" title="Measured GPU draw above its idle, added to the estimate"> +GPU {fmtW(h.gpuW)}</span>{/if}
        </span>
        <span class="n data">{h.avgCpu == null ? "--" : `${h.avgCpu.toFixed(0)}%`}</span>
        <span class="n data">{fmtW(h.avgW)}</span>
        <span class="n data muted">{fmtW(h.peakW)}</span>
        <span class="n data">{fmtKwh(h.kwhMonth)}</span>
        <span class="n data">{cost(h.kwhMonth) ?? "--"}</span>
      </div>
    {:else}
      <p class="muted small pad">No hosts yet.</p>
    {/each}
    {#if sorted.some((h) => h.profile && !h.hours)}
      <p class="muted small pad">Hosts without CPU history count at their idle watts until metrics have been collected for a while.</p>
    {/if}

    {#if hints.length}
      <h3 class="hd">IDLE HOURS</h3>
      {#each hints as { h, w } (`${h.hostId}-${w.from}`)}
        <div class="hint" data-guide="power-idle">
          <span><strong>{h.name}</strong> is idle {windowText(w)} ({w.hours} h) on {Math.round(w.share * 100)}% of days. Switched off then, it would save about {fmtKwh(w.kwhMonth)}{cost(w.kwhMonth) ? ` (${cost(w.kwhMonth)})` : ""} a month.</span>
          <button type="button" class="btn tiny ghost" onclick={() => schedule(h.hostId)} title="System → SCHEDULES: shut down and wake on a timetable">Schedule…</button>
        </div>
      {/each}
      <p class="muted small pad">Idle means under 5% CPU and almost no network traffic, on at least 85% of the days in the range (5 days or more). Wake-on-LAN brings a host back for its morning.</p>
    {/if}

    {#if data?.ups.length}
      <h3 class="hd">UPS</h3>
      {#each data.ups as u (u.id)}
        {@const line = upsCompare(u)}
        <div class="hint">
          <span><strong>{u.label}</strong>: {u.watts == null ? "no output reading (NUT reports neither ups.realpower nor a nominal power)" : `${fmtW(u.watts)} ${u.measured ? "measured" : "(load × rated power)"}`}{u.estimatedW != null ? `; estimates for its ${u.hosts.length} host${u.hosts.length === 1 ? "" : "s"} add up to ${fmtW(u.estimatedW)}` : ""}. {line ?? ""}</span>
        </div>
      {/each}
    {/if}
  </div>
</div>

<Modal open={!!editing} title={editing ? `POWER · ${editing.h.name.toUpperCase()}` : ""} width={460} onclose={() => (editing = null)}>
  {#if editing}
    <form class="form" onsubmit={(e) => (e.preventDefault(), void saveProfile())}>
      <p class="muted small">What it draws from the wall with the CPU idle and fully busy. A smart plug or a UPS gives real numbers; a preset is a fair start.</p>
      <label class="field"
        ><span class="label">PRESET</span>
        <select class="input" value={editing.preset} onchange={(e) => pickPreset(e.currentTarget.value)}>
          <option value="">Your own numbers</option>
          {#each PRESETS as pr (pr.id)}<option value={pr.id}>{pr.label} ({pr.idle}–{pr.max} W)</option>{/each}
        </select>
      </label>
      <div class="two">
        <label class="field"><span class="label">IDLE (W)</span><input class="input" bind:value={editing.idle} inputmode="decimal" oninput={() => editing && (editing.preset = "")} /></label>
        <label class="field"><span class="label">FULL LOAD (W)</span><input class="input" bind:value={editing.max} inputmode="decimal" oninput={() => editing && (editing.preset = "")} /></label>
      </div>
      {#if editing.err}<p class="error-text small">{editing.err}</p>{/if}
      <button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
    </form>
  {/if}
  {#snippet footer()}
    {#if editing?.h.profile}<button type="button" class="btn ghost" onclick={clearProfile}>Remove</button>{/if}
    <span class="sp"></span>
    <button type="button" class="btn ghost" onclick={() => (editing = null)}>Cancel</button>
    <button type="button" class="btn primary" disabled={editing?.busy} onclick={() => void saveProfile()}>Save</button>
  {/snippet}
</Modal>

<style>
  .power {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .sp {
    flex: 1;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .field.inline {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
  .num {
    width: 80px;
  }
  .cur {
    width: 56px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 8px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 1px solid var(--line);
    padding: 8px 10px;
  }
  .big {
    font-size: var(--fs-lg, 20px);
    color: var(--text-0);
  }
  .big.ok {
    color: var(--ok);
  }
  .scroll {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  .trow {
    display: grid;
    grid-template-columns: minmax(100px, 1.2fr) minmax(110px, 1fr) 50px 80px 70px 90px 90px;
    gap: 10px;
    align-items: center;
    padding: 5px 4px;
    border-bottom: 1px solid var(--line);
  }
  .trow.head {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .n {
    text-align: right;
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 14px 0 4px;
    padding-left: 4px;
  }
  .hint {
    display: flex;
    gap: 10px;
    align-items: center;
    justify-content: space-between;
    padding: 5px 4px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .ok {
    color: var(--ok);
  }
  .pad {
    padding: 8px 4px;
  }
  p {
    margin: 0;
  }
</style>
