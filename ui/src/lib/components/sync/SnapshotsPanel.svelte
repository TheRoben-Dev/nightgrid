<script lang="ts">
  /**
   * Settings → Sync → SNAPSHOTS (D-209): the last few versions of the synced
   * profile, saved before changes from another device landed. Rolling back
   * is a new edit, so devices that sync automatically follow.
   */
  import { onMount } from "svelte";
  import { api, errorText, type RolledBack, type SnapshotReason, type SyncSnapshot } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { sync } from "../../stores/sync.svelte";
  import Modal from "../Modal.svelte";
  import Panel from "../Panel.svelte";

  let list = $state<SyncSnapshot[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let picked = $state<SyncSnapshot | null>(null);
  let done = $state<RolledBack | null>(null);

  async function load() {
    try {
      list = await api.syncSnapshots();
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(() => {
    void load();
    const t = setInterval(() => void load(), 20_000);
    return () => clearInterval(t);
  });
  // New changes arrived: a snapshot may have been taken.
  $effect(() => {
    if (sync.lastApplied) void load();
  });

  const WHY: Record<SnapshotReason, (p: string | null) => string> = {
    changes: (p) => `Before changes from ${p ?? "another device"}`,
    push: (p) => `Before ${p ?? "a device"} pushed its profile here`,
    pull: (p) => `Before pulling ${p ?? "a device"}'s profile`,
    import: (p) => `Before importing ${p ?? "a device"}'s profile`,
    rollback: () => "Before a rollback",
    manual: () => "Taken by hand",
  };
  const MAIN = ["hosts", "services", "notes", "dashboards"];
  function summary(s: SyncSnapshot): string {
    const parts = MAIN.filter((t) => s.counts[t]).map((t) => `${s.counts[t]} ${t}`);
    const rest = Object.entries(s.counts)
      .filter(([t]) => !MAIN.includes(t))
      .reduce((a, [, n]) => a + n, 0);
    if (rest) parts.push(`${rest} other items`);
    return parts.join(" · ") || "empty";
  }
  function when(t: number | null): string {
    const d = new Date((t ?? 0) * 1000);
    const today = new Date().toDateString() === d.toDateString();
    return today ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : d.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  }

  async function takeNow() {
    busy = true;
    error = null;
    try {
      list = await api.syncSnapshotTake();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function rollback() {
    if (!picked) return;
    busy = true;
    error = null;
    try {
      done = await api.syncRollback(picked.id);
      if (!done.restart) {
        void hosts.refresh();
        void settings.load();
      }
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
  function close() {
    if (busy) return;
    if (done?.restart) {
      void api.appRestart();
      return;
    }
    picked = null;
    done = null;
  }

  const keep = $derived(sync.info?.settings.keepSnapshots ?? 3);
</script>

<Panel title="SNAPSHOTS" guide="sync-snapshots" prefix=">" count={list.length}>
  <p class="muted small">
    Before changes from another device land here, Nightgrid saves the synced profile and keeps the last {keep}. A bad change that synced over? Roll back: devices that sync automatically
    with this one get the rollback too. Snapshots stay on this device and hold no secrets.
  </p>
  <div class="row">
    <label class="keep">
      <span>KEEP</span>
      <input
        class="input mono"
        type="number"
        min="1"
        max="20"
        value={keep}
        aria-label="Snapshots to keep"
        onchange={(e) => void sync.configure({ keepSnapshots: Math.min(20, Math.max(1, Number((e.target as HTMLInputElement).value) || 3)) }).then(load)}
      />
    </label>
    <button type="button" class="btn ghost sm" disabled={busy} onclick={() => void takeNow()}>Take one now</button>
  </div>
  {#if error && !picked}<p class="error" role="alert">{error}</p>{/if}
  <ul class="snaps">
    {#each list as s (s.id)}
      <li>
        <span class="time mono">{when(s.takenAt)}</span>
        <span class="why">{WHY[s.reason](s.peer)}</span>
        <span class="muted small">{summary(s)}</span>
        <span class="spacer"></span>
        {#if s.available}
          <button type="button" class="btn sm" disabled={busy} onclick={() => ((picked = s), (done = null), (error = null))}>Roll back…</button>
        {:else}
          <span class="muted small" title="A whole profile was pushed or pulled after it">can't roll back</span>
        {/if}
      </li>
    {:else}
      <li class="muted small">None yet. One is saved the first time changes arrive from another device.</li>
    {/each}
  </ul>
</Panel>

<Modal open={!!picked} title={done ? "ROLLED BACK" : "ROLL BACK"} tone={done ? "default" : "danger"} width={520} onclose={close}>
  {#if picked}
    {#if done}
      {#if done.restart}
        <p>The profile from {when(picked.takenAt)} is restored on this device. Restart to load it.</p>
        <p class="muted small">A whole profile arrived after that snapshot, so the rollback stays on this device. Push it to your other devices if they should follow.</p>
      {:else}
        <p>Back to {when(picked.takenAt)}: {done.merged.applied} item{done.merged.applied === 1 ? "" : "s"} restored, {done.merged.deleted} removed.</p>
        <p class="muted small">Devices that sync automatically with this one get it within seconds. The state from just before the rollback was saved, so you can undo it here.</p>
      {/if}
    {:else}
      <p>Roll the synced profile back to <b>{when(picked.takenAt)}</b> ({WHY[picked.reason](picked.peer).toLowerCase()})?</p>
      <p class="muted small">
        {picked.spreads
          ? "Hosts, services, notes, dashboards, alerts and shared settings go back to how they were. Anything added since is removed. It counts as a new edit, so devices with two-way or receive sync from this one follow."
          : "A whole profile arrived after this snapshot, so the saved profile replaces this one on this device only, and the app restarts."}
      </p>
      <p class="muted small">Per-device settings, history, metrics and saved secrets aren't touched. The current state is saved first.</p>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  {/if}
  {#snippet footer()}
    {#if done}
      <button type="button" class="btn primary" onclick={close}>{done.restart ? "Restart now" : "Done"}</button>
    {:else}
      <button type="button" class="btn ghost" disabled={busy} onclick={close}>Cancel</button>
      <button type="button" class="btn danger" disabled={busy} onclick={() => void rollback()}>{busy ? "Rolling back…" : "Roll back"}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .row {
    display: flex;
    gap: 12px;
    align-items: center;
    margin: 8px 0;
  }
  .keep {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .keep input {
    width: 4.5em;
  }
  .snaps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .snaps li {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .time {
    color: var(--cyan);
    min-width: 6.5em;
  }
  .why {
    color: var(--text-0);
  }
  .spacer {
    flex: 1;
  }
  .muted {
    color: var(--text-1);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .error {
    color: var(--danger);
  }
</style>
