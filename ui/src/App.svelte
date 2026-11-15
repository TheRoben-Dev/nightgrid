<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, type AppInfo } from "./lib/api";
  import { listen } from "@tauri-apps/api/event";
  import AlertsPanel from "./lib/components/alerts/AlertsPanel.svelte";
  import IntegrationEditor from "./lib/components/integrations/IntegrationEditor.svelte";
  import { alerts } from "./lib/stores/alerts.svelte";
  import "@xterm/xterm/css/xterm.css";
  import CommandPalette from "./lib/components/CommandPalette.svelte";
  import HostEditor from "./lib/components/HostEditor.svelte";
  import HostMonitor from "./lib/components/metrics/HostMonitor.svelte";
  import UpdateDialog from "./lib/components/update/UpdateDialog.svelte";
  import SshImportDialog from "./lib/components/SshImportDialog.svelte";
  import LeftRail from "./lib/components/LeftRail.svelte";
  import StatusStrip from "./lib/components/StatusStrip.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import { actionFor, type ActionId } from "./lib/keys";
  import { startGamepad } from "./lib/gamepad";
  import { buildCommands } from "./lib/palette/commands";
  import { scramble } from "./lib/scramble";
  import { SECTIONS, sectionById } from "./lib/sections";
  import { settings } from "./lib/stores/settings.svelte";
  import { hosts } from "./lib/stores/hosts.svelte";
  import { metrics } from "./lib/stores/metrics.svelte";
  import { updates } from "./lib/stores/updates.svelte";
  import { terms } from "./lib/stores/terms.svelte";
  import { ui } from "./lib/stores/ui.svelte";
  import Overview from "./lib/views/Overview.svelte";
  import Placeholder from "./lib/views/Placeholder.svelte";
  import Settings from "./lib/views/Settings.svelte";
  import Terminal from "./lib/views/Terminal.svelte";
  import Files from "./lib/views/Files.svelte";
  import Docker from "./lib/views/Docker.svelte";
  import Services from "./lib/views/Services.svelte";
  import ServiceEditor from "./lib/components/services/ServiceEditor.svelte";
  import HomepageImport from "./lib/components/services/HomepageImport.svelte";
  import { services } from "./lib/stores/services.svelte";
  import Tunnels from "./lib/views/Tunnels.svelte";
  import Automation from "./lib/views/Automation.svelte";
  import Notes from "./lib/views/Notes.svelte";
  import Network from "./lib/views/Network.svelte";
  import SnippetEditor from "./lib/components/automation/SnippetEditor.svelte";
  import PlaybookEditor from "./lib/components/automation/PlaybookEditor.svelte";
  import { automation } from "./lib/stores/automation.svelte";
  import System from "./lib/views/System.svelte";
  import TunnelEditor from "./lib/components/tunnels/TunnelEditor.svelte";
  import { tunnels } from "./lib/stores/tunnels.svelte";
  import HelperPanel from "./lib/components/helper/HelperPanel.svelte";
  import Spotlight from "./lib/components/helper/Spotlight.svelte";
  import ProfileReminder from "./lib/components/backup/ProfileReminder.svelte";
  import { helper } from "./lib/stores/helper.svelte";

  let info = $state<AppInfo | null>(null);
  /** Files stays mounted after the first visit so open editors survive section switches. */
  let filesMounted = $state(false);
  $effect(() => {
    if (ui.section === "files") filesMounted = true;
  });
  let booted = $state(false);

  onMount(() => {
    const stop = settings.watchReducedMotion();
    const stopHosts = hosts.start();
    const stopPad = startGamepad({
      enabled: () => settings.value.controllerNav,
      view: (step) => {
        const i = SECTIONS.findIndex((s) => s.id === ui.section);
        ui.go(SECTIONS[(i + step + SECTIONS.length) % SECTIONS.length].id);
      },
      palette: () => (ui.paletteOpen = !ui.paletteOpen),
      rail: () => settings.update({ railCollapsed: !settings.value.railCollapsed }),
    });
    settings.load().then(() => {
      booted = true;
      metrics.start();
      services.start();
      tunnels.start();
      alerts.start();
      void automation.load();
      updates.startLaunchCheck();
    });
    // Tray menu: quick connect and "open alerts".
    const unlisten: Promise<() => void>[] = [];
    if (inTauri()) {
      unlisten.push(listen<{ hostId: number }>("tray://connect", (e) => terms.goToHost(e.payload.hostId)));
      unlisten.push(listen("tray://alerts", () => (alerts.open = true)));
    }
    api
      .appInfo()
      .then((i) => {
        info = i;
        ui.keychain = i.keychain.available;
      })
      .catch((e) => console.error("app_info failed", e));
    return () => {
      stop();
      stopHosts();
      stopPad();
      for (const u of unlisten) void u.then((f) => f());
    };
  });

  // Terminal settings and theme changes apply to open panes live.
  $effect(() => {
    const s = settings.value;
    void [s.theme, s.customTheme, s.termFontSize, s.termScrollback, s.termCursorBlink, s.termSolidBg];
    queueMicrotask(() => terms.applySettings());
  });

  // Push theme/effects/scale onto <html> whenever they change.
  $effect(() => {
    settings.apply();
  });

  const commands = $derived(ui.paletteOpen ? buildCommands() : []);
  const section = $derived(sectionById(ui.section));

  const handlers: Partial<Record<ActionId, () => void>> = {
    "palette.open": () => (ui.paletteOpen = !ui.paletteOpen),
    "palette.openAlt": () => (ui.paletteOpen = !ui.paletteOpen),
    "term.newLocal": () => terms.openLocal(),
    "term.closePane": () => {
      const s = terms.currentSession;
      if (s && ui.section === "terminal") terms.closePane(s.paneId);
    },
    "term.splitRight": () => ui.section === "terminal" && terms.split("row"),
    "term.splitDown": () => ui.section === "terminal" && terms.split("col"),
    "term.find": () => {
      const s = terms.currentSession;
      if (s && ui.section === "terminal") {
        s.searchOpen = !s.searchOpen;
        if (!s.searchOpen) {
          s.search.clearDecorations();
          s.focus();
        }
      }
    },
    "term.nextTab": () => ui.section === "terminal" && terms.cycle(1),
    "term.prevTab": () => ui.section === "terminal" && terms.cycle(-1),
    "rail.toggle": () => settings.update({ railCollapsed: !settings.value.railCollapsed }),
    "effects.toggle": () => settings.update({ reduceEffects: !settings.value.reduceEffects }),
    "helper.toggle": () => helper.toggle(),
  };
  for (const s of SECTIONS) handlers[s.action] = () => ui.go(s.id);

  function onkeydown(e: KeyboardEvent) {
    const inTerminal = e.target instanceof Element && !!e.target.closest(".xterm");
    const action = actionFor(e, inTerminal);
    if (!action) return;
    // While the palette is open only its own toggle is global.
    if (ui.paletteOpen && action !== "palette.open" && action !== "palette.openAlt") return;
    // Modal dialogs own the keyboard.
    if (document.querySelector("dialog[open]")) return;
    e.preventDefault();
    handlers[action]?.();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" aria-hidden="true"></div>
{#if settings.value.scanlines}<div class="scanlines" aria-hidden="true"></div>{/if}

<div class="shell" class:booted>
  <LeftRail version={info?.version ?? "0.10.17"} />
  <div class="main">
    <TopBar />
    <main class="content" class:fill={section.id === "terminal" || section.id === "files" || section.id === "docker" || section.id === "system" || section.id === "notes" || section.id === "network"} aria-label={section.label}>
      <div class="heading">
        <h1 class="label">
          <span class="slash" aria-hidden="true">//</span>
          <span use:scramble={section.label.toUpperCase()}></span>
        </h1>
        <span class="micro" aria-hidden="true">SEC.{section.code} · {section.milestone}</span>
      </div>
      <!-- Terminals stay mounted (hidden) so sessions survive section switches. -->
      <div class="view-fill" class:hidden={section.id !== "terminal"}>
        <Terminal />
      </div>
      {#if filesMounted}
        <div class="view-fill" class:hidden={section.id !== "files"}>
          <Files />
        </div>
      {/if}
      {#if section.id === "terminal" || section.id === "files"}
        <!-- rendered above -->
      {:else if section.id === "docker"}
        <div class="view-fill"><Docker /></div>
      {:else if section.id === "system"}
        <div class="view-fill"><System /></div>
      {:else if section.id === "services"}
        <Services />
      {:else if section.id === "tunnels"}
        <Tunnels />
      {:else if section.id === "automation"}
        <Automation />
      {:else if section.id === "network"}
        <div class="view-fill"><Network /></div>
      {:else if section.id === "notes"}
        <div class="view-fill"><Notes /></div>
      {:else if section.id === "overview"}
        <Overview {info} />
      {:else if section.id === "settings"}
        <Settings {info} />
      {:else}
        <Placeholder {section} />
      {/if}
    </main>
    <StatusStrip {info} />
  </div>
  {#if helper.open}<HelperPanel />{/if}
</div>
<Spotlight />
<ProfileReminder />

<CommandPalette open={ui.paletteOpen} {commands} onclose={() => (ui.paletteOpen = false)} />
<HostEditor />
<SshImportDialog />
<HostMonitor />
<UpdateDialog />
<ServiceEditor />
<HomepageImport />
<TunnelEditor />
<AlertsPanel />
<IntegrationEditor />
<SnippetEditor />
<PlaybookEditor />

<style>
  .shell {
    position: relative;
    z-index: 1;
    height: 100%;
    display: flex;
    opacity: 0;
  }
  .shell.booted {
    opacity: 1;
    transition: opacity var(--dur-slow) var(--ease);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px 20px 24px;
  }
  .content.fill {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding-bottom: 10px;
  }
  .view-fill {
    flex: 1;
    min-height: 0;
  }
  .view-fill.hidden {
    display: none;
  }
  .heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 14px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: 700;
    color: var(--text-0);
    display: flex;
    gap: 10px;
  }
  .slash {
    color: var(--magenta);
  }
  .micro {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
</style>
