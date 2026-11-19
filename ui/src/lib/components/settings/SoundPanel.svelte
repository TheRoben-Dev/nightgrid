<script lang="ts">
  import type { SoundSettings } from "../../api";
  import Panel from "../Panel.svelte";
  import SegmentSlider from "../SegmentSlider.svelte";
  import Toggle from "../Toggle.svelte";
  import { CATEGORIES, SOUNDS, type SoundCategory } from "../../sound/sounds";
  import { fromHhmm, toHhmm } from "../../stores/alerts.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { sound } from "../../stores/sound.svelte";

  const VOLUME_STEPS = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100] as const;

  const s = $derived(settings.value.sound);
  const on = $derived(settings.on("sound"));

  const set = (patch: Partial<SoundSettings>) => settings.update({ sound: { ...s, ...patch } });

  function category(id: SoundCategory, v: boolean) {
    const rest = s.muted.filter((m) => m !== id);
    void set({ muted: v ? rest : [...rest, id] });
  }

  function time(field: "quietStart" | "quietEnd", v: string) {
    const m = fromHhmm(v);
    if (m !== null) void set({ [field]: m });
  }
</script>

<div class="grid">
  <Panel title="SOUND" guide="settings-sound">
    <div class="toggles">
      <Toggle
        label="UI sounds"
        hint="Short synthesized blips for alerts, the inbox, actions, dialogs, DRIFT, quests and long tasks. Never for terminal output or live numbers."
        checked={on}
        onchange={(v) => settings.setFeature("sound", v)}
      />
    </div>
    <SegmentSlider label="VOLUME" steps={VOLUME_STEPS} value={s.volume} format={(v) => `${v}%`} onchange={(v) => set({ volume: v })} />
    <div class="toggles">
      <Toggle label="Mute when the window isn't focused" checked={s.unfocusedMute} disabled={!on} onchange={(v) => set({ unfocusedMute: v })} />
      <Toggle label="Quiet hours" hint="No sounds in this window, every day." checked={s.quietHours} disabled={!on} onchange={(v) => set({ quietHours: v })} />
    </div>
    {#if s.quietHours}
      <div class="row">
        <label class="fld"><span class="label">FROM</span>
          <input class="input hhmm" type="time" value={toHhmm(s.quietStart)} onchange={(e) => time("quietStart", e.currentTarget.value)} />
        </label>
        <label class="fld"><span class="label">UNTIL</span>
          <input class="input hhmm" type="time" value={toHhmm(s.quietEnd)} onchange={(e) => time("quietEnd", e.currentTarget.value)} />
        </label>
      </div>
    {/if}
    {#if settings.error}<p class="error" role="alert"><span aria-hidden="true">▲</span> {settings.error}</p>{/if}
  </Panel>

  <Panel title="SOUNDS" guide="settings-sound-list" prefix=">" count={SOUNDS.length}>
    <p class="hint">Turn a kind of sound off, or press ▶ to hear one at the volume above.</p>
    {#each CATEGORIES as c (c.id)}
      <div class="cat">
        <Toggle label={c.label} hint={c.hint} checked={!s.muted.includes(c.id)} disabled={!on} onchange={(v) => category(c.id, v)} />
        <div class="tests">
          {#each SOUNDS.filter((x) => x.category === c.id) as x (x.id)}
            <button type="button" class="btn ghost tiny" title="Play {x.label}" onclick={() => sound.test(x.id)}>▶ {x.label}</button>
          {/each}
        </div>
      </div>
    {/each}
  </Panel>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: 12px;
    align-items: start;
  }
  .toggles {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .fld {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .hhmm {
    width: 110px;
  }
  .hint {
    color: var(--text-1);
    margin: 0 0 6px;
  }
  .cat {
    border-top: 1px solid var(--line);
    padding: 2px 0 8px;
  }
  .tests {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-left: 30px;
  }
  .error {
    color: var(--danger);
  }
</style>
