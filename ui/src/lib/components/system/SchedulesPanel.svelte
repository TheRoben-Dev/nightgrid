<script lang="ts">
  /**
   * Systemd timers and cron jobs on the host, with the crontab and timer
   * editors (M10v). Times are shown
   * relative to the host's clock; cron commands that look like they carry a
   * secret are masked until clicked (security rule 6).
   */
  import { SvelteSet } from "svelte/reactivity";
  import type { CronJob, Timer } from "../../api";
  import { redact } from "../../helper/prompt";
  import { system } from "../../stores/system.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import PowerSchedules from "./PowerSchedules.svelte";
  import CronEditor from "./CronEditor.svelte";
  import TimerJobEditor from "./TimerJobEditor.svelte";
  import { jobs } from "../../stores/jobs.svelte";
  import { jobFor } from "../../jobs";
  import type { InstalledJob } from "../../api";

  let cronOpen = $state(false);
  let timerOpen = $state(false);
  let timerEditing = $state<InstalledJob | null>(null);
  function newTimer() {
    timerEditing = null;
    timerOpen = true;
  }
  $effect(() => {
    if (s) void jobs.loadTimers();
  });

  const s = $derived(system.schedules);
  let query = $state("");
  const revealed = new SvelteSet<string>();

  const q = $derived(query.trim().toLowerCase());
  const timers = $derived(
    (s?.timers ?? []).filter((t) => !q || `${t.name} ${t.description} ${t.unit} ${t.calendar.join(" ")}`.toLowerCase().includes(q)),
  );
  const cron = $derived((s?.cron ?? []).filter((j) => !q || `${j.command} ${j.source} ${j.user ?? ""} ${j.every}`.toLowerCase().includes(q)));
  const failed = $derived((s?.timers ?? []).filter((t) => isFailure(t)).length);

  function isFailure(t: Timer): boolean {
    return t.active === "failed" || (!!t.result && t.result !== "success");
  }

  /** "in 2h 5m" / "3d ago", against the host's clock. */
  function rel(ts: number | null): string {
    if (ts === null || !s) return "—";
    const d = ts - s.now;
    const a = Math.abs(d);
    const m = Math.round(a / 60);
    const txt = m < 1 ? "now" : m < 60 ? `${m}m` : m < 1440 ? `${Math.floor(m / 60)}h${m % 60 ? ` ${m % 60}m` : ""}` : `${Math.floor(m / 1440)}d${Math.floor((m % 1440) / 60) ? ` ${Math.floor((m % 1440) / 60)}h` : ""}`;
    if (txt === "now") return txt;
    return d >= 0 ? `in ${txt}` : `${txt} ago`;
  }

  /** The time on the host's wall clock, for tooltips. */
  function hostTime(ts: number | null): string {
    if (ts === null || !s) return "";
    const d = new Date((ts + s.utcOffsetMin * 60) * 1000);
    const off = s.utcOffsetMin;
    const sign = off < 0 ? "-" : "+";
    const oh = String(Math.floor(Math.abs(off) / 60)).padStart(2, "0");
    const om = String(Math.abs(off) % 60).padStart(2, "0");
    return `${d.toISOString().slice(0, 16).replace("T", " ")} (host time, UTC${sign}${oh}:${om})`;
  }

  const schedule = (t: Timer) => [...t.calendar, ...t.monotonic].join(" · ") || "—";
  const resultWord = (t: Timer) =>
    !t.result ? "" : t.result === "success" ? "OK" : t.result === "exit-code" && t.exitStatus !== null ? `EXIT ${t.exitStatus}` : t.result.toUpperCase();

  const masked = (j: CronJob) => redact(j.command);
  function toggle(i: string) {
    if (revealed.has(i)) revealed.delete(i);
    else revealed.add(i);
  }

  function explainText(): string {
    if (!s) return "";
    const lines = ["Systemd timers:"];
    for (const t of s.timers)
      lines.push(`- ${t.name} → ${t.unit}: ${schedule(t)}; next ${rel(t.next)}; last ${rel(t.last)}${t.result ? ` (${resultWord(t)})` : ""}${t.active !== "active" ? `; ${t.active}` : ""}`);
    lines.push("Cron jobs:");
    for (const j of s.cron) lines.push(`- [${j.source}${j.user ? `, as ${j.user}` : ""}] ${j.schedule} (${j.every}): ${j.command}`);
    for (const p of s.periodic) lines.push(`/etc/cron.${p.period}: ${p.scripts.join(", ")}`);
    return lines.join("\n");
  }
</script>

<div class="schedules">
  <div class="bar">
    <span class="counts">
      {#if s}<span class="mint">{s.timers.length}</span> TIMERS · <span class="mint">{s.cron.length}</span> CRON JOBS{#if failed}<span class="bad">&nbsp;· {failed} LAST RUN FAILED</span>{/if}{/if}
    </span>
    <span class="spacer"></span>
    <input class="input filter" type="search" placeholder="Filter jobs" bind:value={query} aria-label="Filter jobs" />
    {#if s}<ExplainButton source="scheduled jobs" text={explainText} question="Summarise what these scheduled jobs do. Do any look wrong, risky or failing, and what should I check?" />{/if}
    <button type="button" class="btn ghost tiny" data-guide="schedules-crontab" onclick={() => (cronOpen = true)}>Crontab…</button>
    {#if s?.systemd}<button type="button" class="btn ghost tiny" data-guide="schedules-new-timer" onclick={newTimer}>+ Timer</button>{/if}
    <button type="button" class="btn ghost tiny" onclick={() => Promise.all([system.loadSchedules(), system.loadPowerSchedules()])} disabled={system.schedulesLoading}>{system.schedulesLoading ? "…" : "Refresh"}</button>
  </div>

  <div class="scroll" data-guide="system-schedules">
    <PowerSchedules />
    {#if system.errors.schedules}
      <p class="error-text pad">{system.errors.schedules}</p>
    {:else if !s}
      <p class="muted pad">{system.schedulesLoading ? "Reading timers and crontabs…" : ""}</p>
    {:else}
      <section>
        <h3 class="label">// SYSTEMD TIMERS</h3>
        {#if !s.systemd}
          <p class="muted pad">This host doesn't use systemd.</p>
        {:else if !timers.length}
          <p class="muted pad">{s.timers.length ? "No timers match." : "No timers."}</p>
        {:else}
          <div class="grid timers" role="table" aria-label="Systemd timers">
            <div class="head" role="row">
              <span role="columnheader">TIMER</span><span role="columnheader">SCHEDULE</span><span role="columnheader">NEXT</span><span role="columnheader"
                >LAST</span
              ><span></span>
            </div>
            {#each timers as t (t.name)}
              {@const own = jobFor(t, jobs.timers)}
              <div class="row" class:fail={isFailure(t)} class:off={t.active !== "active"} role="row">
                <span class="name" title={t.description}>
                  <code>{t.name}</code>
                  <span class="muted small">{t.description}{t.enabled && t.enabled !== "enabled" ? ` · ${t.enabled}` : ""}</span>
                </span>
                <code class="sched">{schedule(t)}</code>
                <span class="when" title={hostTime(t.next)}>{t.active === "active" ? rel(t.next) : t.active}</span>
                <span class="when" title={hostTime(t.last)}
                  >{rel(t.last)}{#if t.result}<span class="res" class:ok={t.result === "success"}>{resultWord(t)}</span>{/if}</span
                >
                <span class="acts">
                  {#if own}<button type="button" class="btn ghost tiny" onclick={() => ((timerEditing = own), (timerOpen = true))}>Edit</button>{/if}
                  {#if t.unit}<button type="button" class="btn ghost tiny" title="Start {t.unit} now" disabled={system.unitBusy.includes(t.unit)} onclick={() => system.request(t.unit, "start")}>Run now</button>{/if}
                  {#if t.enabled === "enabled" || t.enabled === "disabled"}<button type="button" class="btn ghost tiny" disabled={system.unitBusy.includes(t.name)} onclick={() => system.request(t.name, t.enabled === "enabled" ? "disable" : "enable")}>{t.enabled === "enabled" ? "Disable" : "Enable"}</button>{/if}
                  {#if t.unit}<button type="button" class="btn ghost tiny" title="Journal of {t.unit}, last 7 days" onclick={() => system.showJournal(t.unit)}
                      >Journal</button
                    >{/if}
                </span>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <section>
        <h3 class="label">// CRON</h3>
        {#if !cron.length}
          <p class="muted pad">{s.cron.length ? "No jobs match." : "No cron jobs in your crontab, /etc/crontab or /etc/cron.d."}</p>
        {:else}
          <div class="grid cron" role="table" aria-label="Cron jobs">
            <div class="head" role="row">
              <span role="columnheader">WHEN</span><span role="columnheader">COMMAND</span><span role="columnheader">FROM</span><span role="columnheader">NEXT</span>
            </div>
            {#each cron as j, i (i)}
              {@const m = masked(j)}
              {@const key = `${j.source}|${j.schedule}|${j.command}`}
              <div class="row" role="row">
                <span class="name">
                  <span>{j.every}</span>
                  {#if j.every !== j.schedule}<code class="muted small">{j.schedule}</code>{/if}
                </span>
                <span class="cmd">
                  <code>{m !== j.command && !revealed.has(key) ? m : j.command}</code>
                  {#if m !== j.command}<button type="button" class="btn ghost tiny" onclick={() => toggle(key)}>{revealed.has(key) ? "Hide" : "Reveal"}</button>{/if}
                </span>
                <span class="muted small">{j.source}{j.user ? ` · ${j.user}` : ""}</span>
                <span class="when" title={hostTime(j.next)}>{j.next === null ? (j.schedule === "@reboot" ? "at boot" : "—") : rel(j.next)}</span>
              </div>
            {/each}
          </div>
        {/if}
        {#if s.periodic.length}
          <div class="periodic">
            {#each s.periodic as p (p.period)}
              <p><span class="label">CRON.{p.period.toUpperCase()}</span> <span class="muted">{p.scripts.join(", ")}</span></p>
            {/each}
          </div>
        {/if}
        <p class="muted small note">
          Other users' crontabs in <code>/var/spool/cron</code> need root and aren't listed here: open them with <b>Crontab…</b>, which also edits yours. Files in <code>/etc/cron.d</code> are shown but not edited.
        </p>
      </section>
    {/if}
  </div>
</div>

<CronEditor open={cronOpen} onclose={() => (cronOpen = false)} />
<TimerJobEditor open={timerOpen} editing={timerEditing} onclose={() => (timerOpen = false)} />

<style>
  .schedules {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .counts {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .mint {
    color: var(--mint);
  }
  .bad {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .filter {
    width: 200px;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  section + section {
    border-top: 1px solid var(--line);
  }
  h3 {
    margin: 0;
    padding: 8px 10px 4px;
    color: var(--cyan);
  }
  .pad {
    padding: 8px 10px 12px;
    margin: 0;
  }
  .grid .head,
  .grid .row {
    display: grid;
    gap: 10px;
    align-items: center;
    padding: 4px 10px;
  }
  .timers .head,
  .timers .row {
    grid-template-columns: minmax(180px, 1.3fr) minmax(120px, 1fr) 100px 140px 260px;
  }
  .cron .head,
  .cron .row {
    grid-template-columns: minmax(150px, 0.8fr) minmax(200px, 2fr) minmax(110px, 0.6fr) 100px;
  }
  .head {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-2);
    border-bottom: 1px solid var(--line);
  }
  .row {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    font-size: var(--fs-sm);
    border-left: 2px solid transparent;
  }
  .row:hover {
    background: var(--bg-1);
  }
  .row.fail {
    border-left-color: var(--danger);
  }
  .row.off {
    opacity: 0.6;
  }
  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name > * {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: var(--fs-xs);
  }
  code {
    font-family: var(--font-mono);
    color: var(--cyan);
  }
  .sched {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmd {
    display: flex;
    gap: 6px;
    align-items: center;
    min-width: 0;
  }
  .cmd code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-0);
  }
  .when {
    font-variant-numeric: tabular-nums;
    color: var(--text-1);
    white-space: nowrap;
  }
  .res {
    margin-left: 6px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    color: var(--danger);
  }
  .res.ok {
    color: var(--mint);
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
    white-space: nowrap;
  }
  .periodic {
    padding: 6px 10px 0;
    font-size: var(--fs-sm);
  }
  .periodic p {
    margin: 0 0 4px;
  }
  .note {
    padding: 6px 10px 12px;
    margin: 0;
  }
</style>
