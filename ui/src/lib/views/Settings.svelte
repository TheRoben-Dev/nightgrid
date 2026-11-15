<script lang="ts">
  import { api, type AppInfo } from "../api";
  import Barcode from "../components/Barcode.svelte";
  import Panel from "../components/Panel.svelte";
  import SegmentSlider from "../components/SegmentSlider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { scramble } from "../scramble";
  import { updates } from "../stores/updates.svelte";
  import { DEFAULT_KEYMAP, formatChord, type ActionId } from "../keys";
  import {
    METRICS_BACKGROUND_STEPS,
    METRICS_INTERVAL_STEPS,
    METRICS_RETENTION_STEPS,
    SERVICES_INTERVAL_STEPS,
    settings,
    TERM_FONT_STEPS, TERM_SCROLLBACK_STEPS, UI_SCALE_STEPS } from "../stores/settings.svelte";
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
    "palette.openAlt": "Command palette (works in terminal)",
    "term.newLocal": "New local terminal",
    "term.closePane": "Close terminal pane",
    "term.splitRight": "Split pane right",
    "term.splitDown": "Split pane down",
    "term.find": "Find in terminal",
    "term.nextTab": "Next terminal tab",
    "term.prevTab": "Previous terminal tab",
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

  <Panel title="TERMINAL">
    <SegmentSlider
      label="FONT SIZE"
      steps={TERM_FONT_STEPS}
      value={settings.value.termFontSize}
      format={(v) => `${v}px`}
      onchange={(v) => settings.update({ termFontSize: v })}
    />
    <SegmentSlider
      label="SCROLLBACK"
      steps={TERM_SCROLLBACK_STEPS}
      value={settings.value.termScrollback}
      format={(v) => (v >= 1000 ? `${v / 1000}k` : String(v))}
      onchange={(v) => settings.update({ termScrollback: v })}
    />
    <div class="toggles">
      <Toggle
        label="Copy on select"
        hint="Selecting text copies it to the clipboard."
        checked={settings.value.termCopyOnSelect}
        onchange={(v) => settings.update({ termCopyOnSelect: v })}
      />
      <Toggle
        label="Right-click pastes"
        checked={settings.value.termRightClickPaste}
        onchange={(v) => settings.update({ termRightClickPaste: v })}
      />
      <Toggle
        label="Blinking cursor"
        checked={settings.value.termCursorBlink}
        onchange={(v) => settings.update({ termCursorBlink: v })}
      />
      <Toggle
        label="Edit server files in Nightgrid"
        hint="In SSH terminals, nano FILE and $EDITOR (git commit, crontab -e, sudoedit) open the file in the editor tab with normal shortcuts instead of nano. Nothing is installed on the server; applies to new terminals."
        checked={settings.value.termEditInApp}
        onchange={(v) => settings.update({ termEditInApp: v })}
      />
    </div>
  </Panel>

  <Panel title="MONITORING">
    <SegmentSlider
      label="SAMPLE EVERY"
      steps={METRICS_INTERVAL_STEPS}
      value={settings.value.metricsIntervalSecs}
      format={(v) => `${v}s`}
      onchange={(v) => settings.update({ metricsIntervalSecs: v })}
    />
    <SegmentSlider
      label="IN BACKGROUND"
      steps={METRICS_BACKGROUND_STEPS}
      value={settings.value.metricsBackgroundSecs}
      format={(v) => (v >= 60 ? `${v / 60}m` : `${v}s`)}
      onchange={(v) => settings.update({ metricsBackgroundSecs: v })}
    />
    <SegmentSlider
      label="KEEP DETAIL"
      steps={METRICS_RETENTION_STEPS}
      value={settings.value.metricsRetentionDays}
      format={(v) => `${v}d`}
      onchange={(v) => settings.update({ metricsRetentionDays: v })}
    />
    <div class="toggles">
      <Toggle
        label="Collect host metrics"
        hint="Reads /proc, df and sensors over SSH with one short command per sample. Nothing is installed on the server. Hosts that need a password or a new host key wait until you open a terminal on them."
        checked={settings.value.metricsEnabled}
        onchange={(v) => settings.update({ metricsEnabled: v })}
      />
    </div>
    <p class="muted note">Minute detail is kept for the chosen days; 15-minute averages are kept for at least 90 days.</p>
  </Panel>

  <Panel title="SERVICES">
    <SegmentSlider
      label="CHECK EVERY"
      steps={SERVICES_INTERVAL_STEPS}
      value={settings.value.servicesIntervalSecs}
      format={(v) => (v >= 60 ? `${v / 60}m` : `${v}s`)}
      onchange={(v) => settings.update({ servicesIntervalSecs: v }).then(() => api.servicesCheckAll())}
    />
    <div class="toggles">
      <Toggle
        label="Background health checks"
        hint="One GET request per service per interval, straight from this computer to the service URL. History is kept 30 days."
        checked={settings.value.servicesChecks}
        onchange={(v) => settings.update({ servicesChecks: v }).then(() => api.servicesCheckAll())}
      />
    </div>
  </Panel>

  <Panel title="UPDATES">
    <div class="update-row">
      <span class="muted">Installed</span>
      <span class="data">v{info?.version ?? "…"}</span>
      <button type="button" class="btn ghost" disabled={updates.checking} onclick={() => void updates.check(true)}>
        {updates.checking ? "Checking…" : "Check now"}
      </button>
    </div>
    <p class="muted note" role="status">
      {#if updates.status?.kind === "upToDate"}You're on the latest version.
      {:else if updates.status?.kind === "available"}v{updates.status.version} is available.
        <button type="button" class="link" onclick={() => (updates.prompt = updates.status?.kind === "available" ? updates.status : null)}>Show</button>
      {:else if updates.status?.kind === "notConfigured"}This build can't self-update yet (no release signing key). Download new versions from GitHub Releases.
      {:else}Updates come from the project's GitHub Releases and are verified with a signature before installing.{/if}
    </p>
    {#if updates.error}<p class="error" role="alert" use:scramble={updates.error}></p>{/if}
    <div class="toggles">
      <Toggle
        label="Check for updates on launch"
        hint="One request to GitHub Releases after start-up. Nothing installs without your click."
        checked={settings.value.updateCheck}
        onchange={(v) => settings.update({ updateCheck: v })}
      />
    </div>
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
    <p class="muted note">
      While a terminal has focus on Windows/Linux, only shortcuts with Shift (and Ctrl+Tab) are taken, so the shell
      keeps Ctrl+K, Ctrl+B and friends. On macOS, Cmd shortcuts always work. Rebinding UI arrives later.
    </p>
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
  .note {
    margin: 10px 0 0;
    font-size: var(--fs-xs);
  }
  .update-row {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .update-row .btn {
    margin-left: auto;
  }
  .data {
    color: var(--cyan);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
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
