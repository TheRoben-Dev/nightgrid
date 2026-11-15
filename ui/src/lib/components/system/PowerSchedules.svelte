<script lang="ts">
  /**
   * System → Schedules → POWER (M10k): Nightgrid's own power timers on the
   * host (systemd), and wake-ups for it on its relay host. They run whether
   * or not Nightgrid is open.
   */
  import type { HostSchedule, ScheduledAction } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";

  const DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;
  const ACTION_LABEL: Record<ScheduledAction, string> = { shutdown: "Shut down", suspend: "Suspend", reboot: "Reboot", wake: "Wake up" };

  const host = $derived(system.hostId === null ? undefined : hosts.byId(system.hostId));
  const relay = $derived(host?.wakeRelayId != null ? hosts.byId(host.wakeRelayId) : undefined);
  const canWake = $derived(!!host?.macAddress && !!relay);
  const list = $derived(system.powerSchedules ?? []);

  let action = $state<ScheduledAction>("shutdown");
  let time = $state("23:00");
  let days = $state<string[]>([]);
  const valid = $derived(/^([01]?\d|2[0-3]):[0-5]\d$/.test(time.trim()) && (action !== "wake" || canWake));
  const replaces = $derived(list.find((s) => s.spec.action === action));

  function toggleDay(d: string) {
    days = days.includes(d) ? days.filter((x) => x !== d) : [...days, d];
  }
  const dayText = (d: string[]) => (!d.length ? "every day" : d.length === 5 && !d.includes("sat") && !d.includes("sun") ? "weekdays" : d.join(", "));
  const what = (s: HostSchedule) => `${ACTION_LABEL[s.spec.action]} at ${s.spec.time}, ${dayText(s.spec.days)}`;

  async function add() {
    const ok = await system.addPowerSchedule({ action, time: time.trim(), days, mac: null });
    if (ok) days = [];
  }
</script>

<section data-guide="system-power-schedule">
  <h3 class="label">// POWER SCHEDULE</h3>
  <p class="muted small pad">
    Shut down, suspend or reboot this host at set times, or wake it from {relay ? relay.name : "its Wake from host"}. Each is a systemd timer on the
    host itself (<code>nightgrid-power-…</code>), so it runs whether or not Nightgrid is open, and the host shows as planned, not offline. Times are the
    host's local time.
  </p>
  {#if system.powerSchedulesError}<p class="error-text pad">{system.powerSchedulesError}</p>{/if}
  {#if list.length}
    <ul class="list">
      {#each list as s (s.onHostId + s.unit)}
        <li>
          <span>{what(s)}</span>
          {#if s.spec.action === "wake"}<span class="muted small">from {s.onHostName}</span>{/if}
          <span class="muted small">{s.next ? `next ${s.next}` : ""}</span>
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" disabled={system.powerScheduleBusy} onclick={() => system.removePowerSchedule(s)}>Remove</button>
        </li>
      {/each}
    </ul>
  {/if}
  <form class="add" onsubmit={(e) => { e.preventDefault(); void add(); }}>
    <select class="input" bind:value={action} aria-label="Action">
      <option value="shutdown">Shut down</option>
      <option value="suspend">Suspend</option>
      <option value="reboot">Reboot</option>
      <option value="wake" disabled={!canWake}>Wake up{canWake ? "" : " (needs MAC + Wake from)"}</option>
    </select>
    <span class="muted">at</span>
    <input class="input time" bind:value={time} placeholder="23:00" aria-label="Time (HH:MM)" maxlength="5" />
    <span class="days" role="group" aria-label="Days">
      {#each DAYS as d (d)}
        <button type="button" class="day" class:on={days.includes(d)} aria-pressed={days.includes(d)} onclick={() => toggleDay(d)}>{d.slice(0, 2).toUpperCase()}</button>
      {/each}
    </span>
    <span class="muted small">{dayText(days)}</span>
    <button type="submit" class="btn tiny" disabled={!valid || system.powerScheduleBusy}>{system.powerScheduleBusy ? "…" : replaces ? "REPLACE" : "ADD"}</button>
  </form>
</section>

<style>
  section {
    padding: 0 10px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    margin: 0 0 6px;
  }
  .list {
    list-style: none;
    margin: 0 0 6px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }
  .time {
    width: 64px;
  }
  .days {
    display: inline-flex;
    gap: 2px;
  }
  .day {
    border: 1px solid var(--text-1);
    background: none;
    color: var(--text-1);
    font-size: var(--fs-xs);
    padding: 1px 4px;
    cursor: pointer;
  }
  .day.on {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
