<script lang="ts">
  import type { AppInfo } from "../api";
  import Barcode from "../components/Barcode.svelte";
  import Panel from "../components/Panel.svelte";
  import SegmentSlider from "../components/SegmentSlider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { DEFAULT_KEYMAP, formatChord, type ActionId } from "../keys";
  import { settings, UI_SCALE_STEPS } from "../stores/settings.svelte";
  import { THEME_IDS, THEMES } from "../theme/themes";

  interface Props {
    info: AppInfo | null;
  }
  let { info }: Props = $props();

  const ACTION_LABELS: Record<ActionId, string> = {
    "palette.open": "Command palette",
    "rail.toggle": "Toggle sidebar",
    "nav.settings": "Settings",
    "nav.overview": "Overview",
    "nav.terminal": "Terminal",
    "nav.files": "Files",
    "nav.docker": "Docker",
    "nav.services": "Services",
    "nav.tunnels": "Tunnels",
    "nav.automation": "Automation",
    "effects.toggle": "Toggle effects",
  };
  const bindings = Object.entries(DEFAULT_KEYMAP) as [ActionId, string][];
  const plain = $derived(!THEMES[settings.themeId].effects);
</script>

<div class="grid">
  <div class="wide">
    <Panel title="THEME" prefix=">" count={THEME_IDS.length} reveal>
      <div class="themes" role="radiogroup" aria-label="Theme">
        {#each THEME_IDS as id (id)}
          {@const t = THEMES[id]}
          {@const active = settings.themeId === id}
          <button
            type="button"
            role="radio"
            aria-checked={active}
            class="tile"
            class:active
            onclick={() => settings.update({ theme: id })}
          >
            <span class="name">{t.label}</span>
            <span class="swatches" aria-hidden="true">
              {#each ["bg-1", "text-0", "accent", "cyan", "magenta", "mint"] as const as k (k)}
                <i style:background={t.tokens[k]}></i>
              {/each}
            </span>
            <span class="meta">{t.effects ? "FX ENABLED" : "ZERO EFFECTS"}</span>
            {#if active}<span class="strip"><Barcode seed={id} bars={30} tone="magenta" /></span>{/if}
          </button>
        {/each}
      </div>
    </Panel>
  </div>

  <Panel title="DISPLAY">
    <SegmentSlider
      label="UI SCALE"
      steps={UI_SCALE_STEPS}
      value={settings.value.uiScale}
      format={(v) => `${v}%`}
      onchange={(v) => settings.update({ uiScale: v })}
    />
    <div class="toggles">
      <Toggle
        label="Reduce effects"
        hint={settings.osReducedMotion
          ? "Your OS requests reduced motion, so effects are already off."
          : "Disables glitch text, scanlines, glow and animation."}
        checked={settings.value.reduceEffects}
        onchange={(v) => settings.update({ reduceEffects: v })}
      />
      <Toggle
        label="Scanline overlay"
        hint={plain ? "Not used by the plain theme." : "Very low opacity CRT lines."}
        checked={settings.value.scanlines}
        disabled={plain || settings.value.reduceEffects}
        onchange={(v) => settings.update({ scanlines: v })}
      />
      <Toggle
        label="Collapse sidebar"
        checked={settings.value.railCollapsed}
        onchange={(v) => settings.update({ railCollapsed: v })}
      />
    </div>
    {#if settings.error}<p class="error" role="alert"><span aria-hidden="true">▲</span> {settings.error}</p>{/if}
  </Panel>

  <Panel title="KEYBINDINGS" prefix=">" count={bindings.length}>
    <table>
      <tbody>
        {#each bindings as [action, spec] (action)}
          <tr>
            <td>{ACTION_LABELS[action]}</td>
            <td class="r"><kbd>{formatChord(spec)}</kbd></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="muted note">Rebinding UI arrives with the full settings screen.</p>
  </Panel>

  <div class="wide">
    <Panel title="DIAGNOSTICS">
      <dl>
        <dt>DATA</dt>
        <dd>{info?.dataDir ?? "…"}</dd>
        <dt>LOGS</dt>
        <dd>{info?.logDir ?? "…"}</dd>
        {#if info?.os === "linux"}
          <dt>WEBKIT</dt>
          <dd>DMA-BUF renderer {info.dmabufDisabled ? "disabled (workaround active)" : "enabled"}</dd>
        {/if}
      </dl>
    </Panel>
  </div>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
    align-content: start;
  }
  .wide {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 104px;
    padding: 10px 12px 12px;
    border: 1px solid color-mix(in srgb, var(--mint) 35%, var(--line));
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease),
      background var(--dur-fast) var(--ease);
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
    font-size: var(--fs-md);
  }
  .tile.active .name {
    color: var(--magenta);
  }
  .swatches {
    display: flex;
    gap: 3px;
  }
  .swatches i {
    width: 18px;
    height: 10px;
    border: 1px solid var(--line);
  }
  .meta {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.08em;
  }
  .strip {
    position: absolute;
    right: 10px;
    bottom: 10px;
  }

  .toggles {
    margin-top: 12px;
    border-top: 1px solid var(--line);
    padding-top: 6px;
  }
  .error {
    color: var(--danger);
    margin: 8px 0 0;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }
  td {
    padding: 5px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .r {
    text-align: right;
  }
  .note {
    font-size: var(--fs-xs);
    margin: 10px 0 0;
  }

  dl {
    display: grid;
    grid-template-columns: 80px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  dt {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  @media (max-width: 1100px) {
    .grid {
      grid-template-columns: 1fr;
    }
    .themes {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
