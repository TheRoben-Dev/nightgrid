<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, type AppInfo } from "./lib/api";
  import { listen } from "@tauri-apps/api/event";
  import AlertsPanel from "./lib/components/alerts/AlertsPanel.svelte";
  import UpsCountdown from "./lib/components/alerts/UpsCountdown.svelte";
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
  import { railFor, SECTIONS, sectionById } from "./lib/sections";
  import { settings } from "./lib/stores/settings.svelte";
  import { hosts } from "./lib/stores/hosts.svelte";
  import { metrics } from "./lib/stores/metrics.svelte";
  import { updates } from "./lib/stores/updates.svelte";
  import { terms } from "./lib/stores/terms.svelte";
  import { ui } from "./lib/stores/ui.svelte";
  import { library } from "./lib/stores/library.svelte";
  import { CATALOG, VIEW_HELP } from "./lib/kb/catalog";
  import Overview from "./lib/views/Overview.svelte";
  import Placeholder from "./lib/views/Placeholder.svelte";
  import LazyView from "./lib/components/LazyView.svelte";
  import { VIEWS } from "./lib/views";
  import Settings from "./lib/views/Settings.svelte";
  import Terminal from "./lib/views/Terminal.svelte";
  import ServiceEditor from "./lib/components/services/ServiceEditor.svelte";
  import HomepageImport from "./lib/components/services/HomepageImport.svelte";
  import { services } from "./lib/stores/services.svelte";
  import SnippetEditor from "./lib/components/automation/SnippetEditor.svelte";
  import PlaybookEditor from "./lib/components/automation/PlaybookEditor.svelte";
  import { automation } from "./lib/stores/automation.svelte";
  import TunnelEditor from "./lib/components/tunnels/TunnelEditor.svelte";
  import { tunnels } from "./lib/stores/tunnels.svelte";
  import HelperPanel from "./lib/components/helper/HelperPanel.svelte";
  import Spotlight from "./lib/components/helper/Spotlight.svelte";
  import TourOverlay from "./lib/components/helper/TourOverlay.svelte";
  import AuthNoticeBar from "./lib/components/ssh/AuthNoticeBar.svelte";
  import { authNotices } from "./lib/stores/authNotices.svelte";
  import TourOffer from "./lib/components/helper/TourOffer.svelte";
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
    authNotices.start();
    const stopPad = startGamepad({
      enabled: () => settings.value.controllerNav,
      view: (step) => {
        const on = SECTIONS.filter((s) => settings.on(s.id));
        const i = on.findIndex((s) => s.id === ui.section);
        ui.go(on[(i + step + on.length) % on.length].id);
      },
      palette: () => (ui.paletteOpen = !ui.paletteOpen),
      rail: () => settings.update({ railCollapsed: !settings.value.railCollapsed }),
    });
    settings.load().then(() => {
      booted = true;
      metrics.start();
      alerts.start();
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
      authNotices.stop();
      stopPad();
      for (const u of unlisten) void u.then((f) => f());
    };
  });

  // Views turned off in Settings → Features redirect there instead.
  ui.allow = (id) => settings.on(id);
  // Background stores start only for features that are on, and start when
  // one is turned on later (no restart).
  $effect(() => {
    if (!booted) return;
    if (settings.on("services")) services.start();
    if (settings.on("tunnels")) tunnels.start();
    if (settings.on("automation") && !automation.loaded) void automation.load();
  });
  // A view turned off while open goes back to the overview.
  $effect(() => {
    if (booted && !settings.on(ui.section)) ui.go("overview");
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
  // Views that share a sidebar entry switch with tabs under the title.
  const railEntry = $derived(railFor(ui.section));
  const siblings = $derived((railEntry?.members ?? []).filter((m) => settings.on(m)).map((id) => ({ id, label: railEntry?.tabs?.[id] ?? sectionById(id).label })));
  // A shared entry's views are titled by the entry (NETWORK · MAP | TUNNELS).
  const title = $derived(siblings.length > 1 && railEntry ? railEntry.label : section.label);
  const helpFor = $derived(section.id === "library" ? null : (CATALOG.find((a) => a.id === VIEW_HELP[section.id]) ?? null));

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
    "helper.toggle": () => (settings.on("drift") ? helper.toggle() : ui.go("settings")),
  };
  for (const s of SECTIONS) handlers[s.action] = () => ui.go(s.id);

  function onkeydown(e: KeyboardEvent) {
    // A focused VM screen sends every key to the guest.
    if (e.target instanceof Element && e.target.closest("[data-vm-screen]")) return;
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
  <LeftRail version={info?.version ?? "0.10.62"} />
  <div class="main">
    <TopBar />
    <UpsCountdown />
    <main class="content" class:fill={section.id === "terminal" || section.id === "files" || section.id === "docker" || section.id === "system" || section.id === "notes" || section.id === "library" || section.id === "network" || section.id === "nettools" || section.id === "fleet" || section.id === "desktops" || section.id === "proxmox"} aria-label={section.label}>
      <div class="heading">
        <h1 class="label">
          <span class="slash" aria-hidden="true">//</span>
          <span use:scramble={title.toUpperCase()}></span>
        </h1>
        {#if helpFor}
          <button type="button" class="help" class:solo={siblings.length < 2} data-guide="view-help" title={`Help: ${helpFor.title}`} aria-label={`Help: ${helpFor.title}`} onclick={() => library.open(helpFor.id)}>?</button>
        {/if}
        {#if siblings.length > 1}
          <div class="viewtabs" role="tablist" aria-label="Related views">
            {#each siblings as v (v.id)}
              <button type="button" role="tab" data-guide="viewtab-{v.id}" class:on={v.id === section.id} aria-selected={v.id === section.id} onclick={() => ui.go(v.id)}>{v.label.toUpperCase()}</button>
            {/each}
          </div>
        {/if}
        <span class="micro" aria-hidden="true">SEC.{section.code} · {section.milestone}</span>
      </div>
      <!-- Terminals stay mounted (hidden) so sessions survive section switches. -->
      {#if settings.on("terminal")}
        <div class="view-fill" class:hidden={section.id !== "terminal"}>
          <Terminal />
        </div>
      {/if}
      {#if filesMounted && settings.on("files")}
        <div class="view-fill" class:hidden={section.id !== "files"}>
          <LazyView load={VIEWS.files} />
        </div>
      {/if}
      {#if section.id === "terminal" || section.id === "files"}
        <!-- rendered above -->
      {:else if section.id === "services" || section.id === "tunnels" || section.id === "automation"}
        <LazyView load={VIEWS[section.id]} />
      {:else if section.id in VIEWS}
        <div class="view-fill"><LazyView load={VIEWS[section.id as keyof typeof VIEWS]} /></div>
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
  {#if helper.open && settings.on("drift")}<HelperPanel />{/if}
</div>
<Spotlight />
<TourOverlay />
<AuthNoticeBar />
{#if settings.on("drift")}<TourOffer />{/if}
<ProfileReminder />

<CommandPalette open={ui.paletteOpen} {commands} onclose={() => (ui.paletteOpen = false)} />
<HostEditor />
<SshImportDialog />
<HostMonitor />
<UpdateDialog />
{#if settings.on("services")}
  <ServiceEditor />
  <HomepageImport />
{/if}
{#if settings.on("tunnels")}<TunnelEditor />{/if}
<AlertsPanel />
<IntegrationEditor />
{#if settings.on("automation")}
  <SnippetEditor />
  <PlaybookEditor />
{/if}

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
  .viewtabs {
    display: flex;
    margin-left: 16px;
    margin-right: auto;
    border: 1px solid var(--line);
    align-self: center;
  }
  .viewtabs button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 12px;
    cursor: pointer;
  }
  .viewtabs button + button {
    border-left: 1px solid var(--line);
  }
  .viewtabs button:hover {
    color: var(--text-0);
  }
  .viewtabs button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
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
  .help {
    width: 22px;
    height: 22px;
    margin: 0 0 0 10px;
    align-self: center;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--cyan);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .help.solo {
    margin-right: auto;
  }
  .help:hover {
    border-color: var(--cyan);
    color: var(--magenta);
  }
  .micro {
    font-size: var(--fs-xs);
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
</style>
