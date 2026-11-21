<script lang="ts">
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";
  import { FEATURES, PRESETS, presetOf } from "../../features";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { COMPONENTS, installedFor } from "../../storeCatalog";

  const disabled = $derived(settings.value.disabledFeatures ?? []);
  const preset = $derived(presetOf(disabled));
  // Only what's installed (Settings → STORE) is listed here.
  const shown = $derived(FEATURES.filter((f) => installedFor(settings.value.installedComponents, f.id)));
  const top = $derived(shown.filter((f) => !f.parent));
  const kids = (id: string) => shown.filter((f) => f.parent === id);
  const onCount = $derived(shown.filter((f) => settings.on(f.id)).length);
  const notInstalled = $derived(COMPONENTS.filter((c) => !settings.installed(c.id)).length);
  const asked = $derived(ui.featureAsked ? FEATURES.find((f) => f.id === ui.featureAsked) : undefined);

  function turnOnAsked() {
    if (!asked) return;
    const id = asked.id;
    void settings.setFeature(id, true).then(() => {
      ui.featureAsked = null;
      ui.go(id as Parameters<typeof ui.go>[0]);
    });
  }
</script>

<div class="features">
  {#if asked && !settings.on(asked.id)}
    <div class="asked" role="status">
      <span><b>{asked.label}</b> is turned off.</span>
      <button type="button" class="btn primary tiny" onclick={turnOnAsked}>Turn on and open</button>
      <button type="button" class="btn ghost tiny" onclick={() => (ui.featureAsked = null)}>Dismiss</button>
    </div>
  {/if}

  <Panel title="PRESETS" guide="settings-feature-presets" prefix=">" count={PRESETS.length}>
    <div class="presets" role="radiogroup" aria-label="Feature preset">
      {#each PRESETS as p (p.id)}
        <button type="button" role="radio" aria-checked={preset === p.id} class="tile" class:active={preset === p.id} onclick={() => settings.setPreset(p.id)}>
          <span class="name">{p.label}</span>
          <span class="meta">{p.hint}</span>
        </button>
      {/each}
    </div>
    <p class="muted note">
      {preset ? "" : "Custom mix. "}{onCount} of {shown.length} on. Off hides a feature from the sidebar, palette, shortcuts and DRIFT, doesn't load its code and stops its background checks. Nothing
      is deleted: turn it back on and it's all there, no restart.
      {#if notInstalled}{notInstalled} more optional components are in the
        <button type="button" class="link" onclick={() => (ui.settingsTab = "store")}>STORE</button>.{/if}
    </p>
  </Panel>

  <Panel title="FEATURES" guide="settings-features">
    <div class="list">
      {#each top as f (f.id)}
        <div class="row" class:hl={asked?.id === f.id}>
          <Toggle label={f.label} hint={f.hint} checked={settings.on(f.id)} onchange={(v) => settings.setFeature(f.id, v)} />
        </div>
        {#each kids(f.id) as k (k.id)}
          <div class="row kid" class:hl={asked?.id === k.id}>
            <Toggle
              label={k.label}
              hint={k.hint}
              checked={settings.on(k.id)}
              disabled={!settings.on(f.id)}
              onchange={(v) => settings.setFeature(k.id, v)}
            />
          </div>
        {/each}
      {/each}
    </div>
    {#if settings.error}<p class="error" role="alert"><span aria-hidden="true">▲</span> {settings.error}</p>{/if}
  </Panel>
</div>

<style>
  .features {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 980px;
  }
  .asked {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 12px;
    border: 1px solid var(--magenta);
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .asked span {
    flex: 1;
  }
  .presets {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 84px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--mint) 35%, var(--line));
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--text-0);
  }
  .tile:hover {
    border-color: var(--mint);
  }
  .tile.active {
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 12%, transparent);
    box-shadow: var(--glow-active);
  }
  .tile .name {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.14em;
  }
  .tile.active .name {
    color: var(--magenta);
  }
  .meta {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .note {
    margin: 10px 0 0;
    font-size: var(--fs-xs);
  }
  .list {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }
  .row {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .row.kid {
    padding-left: 28px;
  }
  .row.hl {
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .error {
    color: var(--danger);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font: inherit;
    text-decoration: underline;
  }
  @media (max-width: 720px) {
    .presets {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
