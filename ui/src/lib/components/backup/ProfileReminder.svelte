<script lang="ts">
  /** A small corner card when a profile export is due (weekly by default). */
  import { onMount } from "svelte";
  import { scramble } from "../../scramble";
  import { profileReminder } from "../../stores/profileReminder.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";

  onMount(() => profileReminder.start());
</script>

{#if profileReminder.due && ui.section !== "settings"}
  <div class="remind" role="status" data-guide="profile-reminder">
    <p class="label">// BACKUP REMINDER</p>
    <p class="msg" use:scramble={profileReminder.message}></p>
    <p class="muted small">Export your setup as an encrypted profile so a new or rebuilt computer can pick up where this one left off.</p>
    <div class="acts">
      <button type="button" class="btn primary tiny" onclick={() => profileReminder.exportNow()}>Export profile…</button>
      <button type="button" class="btn ghost tiny" onclick={() => profileReminder.snooze()}>Tomorrow</button>
      <button type="button" class="btn ghost tiny" title="Turn the reminder off (Settings → Backup)" onclick={() => void settings.update({ profileReminderDays: 0 })}>Don't remind me</button>
    </div>
  </div>
{/if}

<style>
  .remind {
    position: fixed;
    right: 16px;
    bottom: 40px;
    width: 340px;
    max-width: calc(100vw - 32px);
    z-index: 60;
    padding: 12px 14px;
    background: var(--bg-1);
    border: 1px solid var(--cyan);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--cyan) 30%, transparent), 0 8px 24px rgba(0, 0, 0, 0.4);
  }
  .label {
    margin: 0 0 4px;
    color: var(--cyan);
  }
  .msg {
    margin: 0 0 4px;
    color: var(--text-0);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0 0 10px;
  }
  .acts {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
