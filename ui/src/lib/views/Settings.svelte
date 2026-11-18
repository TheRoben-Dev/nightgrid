<script lang="ts">
  import { api, errorText, inTauri, type AppInfo } from "../api";
  import AlertSettings from "../components/alerts/AlertSettings.svelte";
  import AlertDeps from "../components/alerts/AlertDeps.svelte";
  import BackupPanel from "../components/backup/BackupPanel.svelte";
  import StatusPagePanel from "../components/status/StatusPagePanel.svelte";
  import TailnetPanel from "../components/tailnet/TailnetPanel.svelte";
  import KeysPanel from "../components/keys/KeysPanel.svelte";
  import McpPanel from "../components/mcp/McpPanel.svelte";
  import HelperSettings from "../components/helper/HelperSettings.svelte";
  import Barcode from "../components/Barcode.svelte";
  import CustomThemeEditor from "../components/CustomThemeEditor.svelte";
  import Panel from "../components/Panel.svelte";
  import SegmentSlider from "../components/SegmentSlider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import FeaturesPanel from "../components/settings/FeaturesPanel.svelte";
  import SoundPanel from "../components/settings/SoundPanel.svelte";
  import { SETTINGS_TABS } from "../settingsTabs";
  import { ui } from "../stores/ui.svelte";
  import { scramble } from "../scramble";
  import { updates } from "../stores/updates.svelte";
  import { kbContent, kbVersionLabel } from "../stores/kbcontent.svelte";
  import { DEFAULT_KEYMAP, formatChord, type ActionId } from "../keys";
  import {
    METRICS_BACKGROUND_STEPS,
    METRICS_INTERVAL_STEPS,
    METRICS_RETENTION_STEPS,
    SERVICES_INTERVAL_STEPS,
    settings,
    TERM_FONT_STEPS, TERM_SCROLLBACK_STEPS, UI_SCALE_STEPS } from "../stores/settings.svelte";
  import { resolveCustom, THEME_IDS, THEMES } from "../theme/themes";

  interface Props {
    info: AppInfo | null;
  }
  let { info }: Props = $props();

  let diagBusy = $state(false);
  let diagNote = $state<string | null>(null);
  async function exportDiagnostics() {
    diagBusy = true;
    try {
      const path = await api.diagnosticsExport();
      diagNote = path ? `Saved to ${path}` : inTauri() ? null : "Only available in the desktop app.";
    } catch (e) {
      diagNote = `Couldn't save it: ${errorText(e)}`;
    } finally {
      diagBusy = false;
    }
  }

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
    "nav.system": "System",
    "nav.automation": "Automation",
    "nav.notes": "Knowledge",
    "nav.network": "Network map",
    "nav.fleet": "Fleet (updates across hosts)",
    "nav.security": "Security (posture and secrets)",
    "nav.desktops": "Desktops (VM screens)",
    "nav.proxmox": "Proxmox",
    "nav.nettools": "Network tools",
    "nav.library": "Library (help)",
    "effects.toggle": "Toggle effects",
    "palette.openAlt": "Command palette (works in terminal)",
    "term.newLocal": "New local terminal",
    "term.closePane": "Close terminal pane",
    "term.splitRight": "Split pane right",
    "term.splitDown": "Split pane down",
    "term.find": "Find in terminal",
    "term.nextTab": "Next terminal tab",
    "term.prevTab": "Previous terminal tab",
    "helper.toggle": "Open or close the DRIFT helper",
  };
  const bindings = Object.entries(DEFAULT_KEYMAP) as [ActionId, string][];
  const plain = $derived(settings.legacyPlain);
  const customTokens = $derived(resolveCustom(settings.value.customTheme));
</script>

<div class="tabs" role="tablist" aria-label="Settings">
  {#each SETTINGS_TABS as t (t.id)}
    <button type="button" role="tab" data-guide="settings-tab-{t.id}" title={t.what} class:on={ui.settingsTab === t.id} aria-selected={ui.settingsTab === t.id} onclick={() => (ui.settingsTab = t.id)}>{t.label}</button>
  {/each}
</div>
{#if ui.settingsTab === "general"}
<div class="grid">
  <Panel title="DESKTOP" guide="settings-desktop">
    <div class="toggles">
      <Toggle
        label={info?.os === "macos" ? "Menu bar icon" : "Tray icon"}
        hint="Host status with quick connect, saved tunnels (click to start or stop), the alert count, Show and Quit."
        checked={settings.value.trayEnabled}
        onchange={(v) => settings.update({ trayEnabled: v })}
      />
      <Toggle
        label="Keep running when the window is closed"
        hint="Closing hides the window; alerts, tunnels and checks keep running. Quit from the {info?.os === 'macos' ? 'menu bar' : 'tray'} icon."
        checked={settings.value.closeToTray}
        disabled={!settings.value.trayEnabled}
        onchange={(v) => settings.update({ closeToTray: v })}
      />
    </div>
  </Panel>

  <Panel title="UPDATES" guide="settings-updates">
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
      <Toggle
        label="Library content updates"
        hint="New and corrected Library articles between app releases: a small signed download from the same GitHub Releases, checked after start-up and every 6 hours. The Library always works offline from its built-in copy."
        checked={settings.value.kbUpdates}
        onchange={(v) => settings.update({ kbUpdates: v })}
      />
    </div>
    <div class="kbrow" data-guide="settings-kb-content">
      <span class="muted small">
        Library content {kbVersionLabel(kbContent.version)} · {kbContent.source === "update" ? `downloaded update (${kbContent.status?.installed?.articles ?? 0} articles)` : "built in"}{#if kbContent.status?.checkedAt} · checked {new Date(kbContent.status.checkedAt).toLocaleTimeString()}{/if}
      </span>
      <button type="button" class="btn ghost tiny" disabled={kbContent.checking || kbContent.status?.notConfigured} onclick={() => void kbContent.check(true)}>{kbContent.checking ? "Checking…" : "Check content"}</button>
      {#if kbContent.status?.installed}<button type="button" class="btn ghost tiny" title="Delete the downloaded content and use the copy built into this version" onclick={() => void kbContent.useBundled()}>Use built-in copy</button>{/if}
    </div>
    {#if kbContent.error}<p class="error" role="alert" use:scramble={kbContent.error}></p>{/if}
  </Panel>

  <div class="wide">
    <Panel title="DIAGNOSTICS" guide="settings-diagnostics">
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
      <div class="diag">
        <button type="button" class="btn chamfer-sm" data-guide="settings-diagnostics-export" disabled={diagBusy} onclick={() => void exportDiagnostics()}>Export diagnostics…</button>
        <span class="muted small">{diagNote ?? "One text file: version, settings, alerts and the newest app logs. Passwords, tokens and secret-looking lines are removed."}</span>
      </div>
    </Panel>
  </div>
</div>
{:else if ui.settingsTab === "appearance"}
<div class="grid">
  <div class="wide">
    <Panel title="THEME" guide="settings-theme" prefix=">" count={THEME_IDS.length} reveal>
      <div class="themes" role="radiogroup" aria-label="Theme">
        {#each THEME_IDS as id (id)}
          {@const t = THEMES[id]}
          {@const tok = id === "custom" ? customTokens : t.tokens}
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
                <i style:background={tok[k]}></i>
              {/each}
            </span>
            <span class="meta">{id === "custom" ? "PICK EACH SLOT" : "FX ENABLED"}</span>
            {#if active}<span class="strip"><Barcode seed={id} bars={30} tone="magenta" /></span>{/if}
          </button>
        {/each}
      </div>
      {#if settings.themeId === "custom"}<CustomThemeEditor />{/if}
    </Panel>
  </div>

  <Panel title="DISPLAY" guide="settings-display">
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
        hint={plain ? "Off while the old plain scheme is kept; pick a scheme to turn it on." : "Very low opacity CRT lines."}
        checked={settings.value.scanlines}
        disabled={plain || settings.value.reduceEffects}
        onchange={(v) => settings.update({ scanlines: v })}
      />
      <Toggle
        label="Handheld layout"
        hint="For the ROG Ally, Steam Deck and touch screens: finger-sized buttons, 130% scale and an icon-only sidebar."
        checked={settings.value.handheld}
        onchange={(v) => settings.setHandheld(v)}
      />
      <Toggle
        label="Controller navigation"
        hint="With a gamepad connected: D-pad or stick moves focus, A selects, B goes back, LB/RB switch views, Start opens the command palette, Select toggles the sidebar."
        checked={settings.value.controllerNav}
        onchange={(v) => settings.update({ controllerNav: v })}
      />
      <Toggle
        label="Collapse sidebar"
        checked={settings.value.railCollapsed}
        onchange={(v) => settings.update({ railCollapsed: v })}
      />
    </div>
    {#if settings.error}<p class="error" role="alert"><span aria-hidden="true">▲</span> {settings.error}</p>{/if}
  </Panel>
</div>
{:else if ui.settingsTab === "sound"}
<SoundPanel />
{:else if ui.settingsTab === "terminal"}
<div class="grid">
  <Panel title="TERMINAL" guide="settings-terminal">
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
        label="Solid background"
        hint="Paint terminals on a solid panel. Off, the app's backdrop shows through behind the text."
        checked={settings.value.termSolidBg}
        onchange={(v) => settings.update({ termSolidBg: v })}
      />
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
      <Toggle
        label="Key hints"
        hint="A strip under the terminal shows the keys of the program in front (nano, vim, less, htop, tmux, journalctl -f …). SSH shells report it through the same session-only hook; nothing is installed."
        checked={settings.value.termKeyHints}
        onchange={(v) => settings.update({ termKeyHints: v })}
      />
      <Toggle
        label="Shell history search"
        hint="Adds a HIST button to SSH terminals that searches the host's bash, zsh and fish history. Lines that look like they hold a password or token are left out, and nothing is saved."
        checked={settings.value.historySearch}
        onchange={(v) => settings.update({ historySearch: v })}
      />
    </div>
  </Panel>

  <Panel title="KEYBINDINGS" guide="settings-keybindings" prefix=">" count={bindings.length}>
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
</div>
{:else if ui.settingsTab === "monitoring"}
<div class="grid">
  <Panel title="MONITORING" guide="settings-monitoring">
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

  <Panel title="SERVICES" guide="settings-services">
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
</div>
{:else if ui.settingsTab === "alerts"}
<div class="grid">
  <AlertSettings />
  <AlertDeps />
</div>
{:else if ui.settingsTab === "connections"}
<div class="grid">
  <TailnetPanel />
  <KeysPanel />
  <McpPanel />
</div>
{:else if ui.settingsTab === "drift"}
<div class="grid">
  <div class="wide"><HelperSettings /></div>
</div>
{:else if ui.settingsTab === "data"}
<div class="grid">
  <BackupPanel />
  <StatusPagePanel />
</div>
{:else}
  <FeaturesPanel />
{/if}

<style>
  .diag {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 10px;
  }
  .tabs {
    display: inline-flex;
    border: 1px solid var(--line);
    margin-bottom: 12px;
  }
  .tabs button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 12px;
    cursor: pointer;
  }
  .tabs button + button {
    border-left: 1px solid var(--line);
  }
  .tabs button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
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
  .kbrow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
  .kbrow span {
    flex: 1 1 220px;
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
