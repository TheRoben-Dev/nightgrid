<script lang="ts">
  import { platform } from "./lib/stores/platform.svelte";
  import { onMount } from "svelte";
  import { api, inTauri, type AppInfo } from "./lib/api";
  import { listen } from "@tauri-apps/api/event";
  import AlertsPanel from "./lib/components/alerts/AlertsPanel.svelte";
  import UpsCountdown from "./lib/components/alerts/UpsCountdown.svelte";
  import QuestToasts from "./lib/components/quests/QuestToasts.svelte";
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
  import { actionFor, isMac, type ActionId } from "./lib/keys";
  import { keyBelongsToTarget, navButton, navKey, SwipeTracker } from "./lib/navhistory";
  import { nav } from "./lib/stores/nav.svelte";
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
  import { kbContent } from "./lib/stores/kbcontent.svelte";
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
  import WebUiNotice from "./lib/components/webui/WebUiNotice.svelte";
  import UiPatchNotice from "./lib/components/update/UiPatchNotice.svelte";
  import { uiPatch } from "./lib/stores/uipatch.svelte";
  import Transmission from "./lib/components/update/Transmission.svelte";
  import { broadcasts } from "./lib/stores/broadcasts.svelte";
  import McpAskCard from "./lib/components/mcp/McpAskCard.svelte";
  import { mcp } from "./lib/stores/mcp.svelte";
  import { authNotices } from "./lib/stores/authNotices.svelte";
  import TourOffer from "./lib/components/helper/TourOffer.svelte";
  import ProfileReminder from "./lib/components/backup/ProfileReminder.svelte";
  import { helper } from "./lib/stores/helper.svelte";
  import { sound } from "./lib/stores/sound.svelte";

  let info = $state<AppInfo | null>(null);
  /** Files stays mounted after the first visit so open editors survive section switches. */
  let filesMounted = $state(false);
  $effect(() => {
    if (ui.section === "files") filesMounted = true;
  });
  let booted = $state(false);

  onMount(() => {
    const stop = settings.watchReducedMotion();
    sound.unlockOnGesture();
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
      platform.start();
      alerts.start();
      updates.startLaunchCheck();
      void kbContent.start();
      void uiPatch.start();
      void broadcasts.start();
      mcp.start();
    });
    // Tray menu: quick connect and "open alerts".
    const unlisten: Promise<() => void>[] = [];
    if (inTauri()) {
      unlisten.push(listen<{ hostId: number }>("tray://connect", (e) => terms.goToHost(e.payload.hostId)));
      // macOS: WKWebView doesn't hand mouse side buttons to the page, so the
      // app catches them natively and sends them here (D-186).
      unlisten.push(
        listen<string>("nav://mouse", (e) => {
          nativeMouseAt = performance.now();
          navigate(e.payload === "forward" ? "forward" : "back");
        }),
      );
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

  // ---- Back/forward (D-186): mouse side buttons, Cmd+[ / Cmd+], Alt+Left /
  // Alt+Right, two-finger swipe. Every change of view, rail tab, sub-tab or
  // host is a step.
  $effect(() => {
    nav.observe();
  });
  const mac = isMac();
  let nativeMouseAt = -Infinity;
  const swipe = new SwipeTracker();

  /** Whether something modal owns input right now. */
  const blocked = () => ui.paletteOpen || !!document.querySelector("dialog[open]");

  function navigate(dir: "back" | "forward") {
    if (blocked()) return;
    if (dir === "back") nav.back();
    else nav.forward();
  }

  function editable(el: Element | null): boolean {
    if (!el) return false;
    if (el instanceof HTMLInputElement) return !["checkbox", "radio", "button", "submit", "range", "color", "file"].includes(el.type);
    return el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement || !!el.closest("[contenteditable=''], [contenteditable='true'], .cm-editor");
  }

  function onNavMouse(e: MouseEvent) {
    const dir = navButton(e.button);
    if (!dir) return;
    // Keep the webview's own history (WebView2, WebKitGTK) out of it.
    e.preventDefault();
    if (e.type !== "mouseup") return;
    // macOS already sent it natively; don't count it twice.
    if (performance.now() - nativeMouseAt < 400) return;
    navigate(dir);
  }

  function onwheel(e: WheelEvent) {
    if (!settings.value.swipeNav || e.ctrlKey || e.shiftKey || e.deltaMode !== 0) return;
    const t = e.target instanceof Element ? e.target : null;
    // Things that use sideways scrolling themselves keep it.
    if (t && (t.closest(".xterm, canvas, svg, [data-vm-screen], [data-no-swipe]") || scrollsSideways(t))) {
      if (e.deltaX !== 0) swipe.cancel(e.timeStamp);
      return;
    }
    const dir = swipe.feed(e.deltaX, e.deltaY, e.timeStamp);
    if (dir) navigate(dir);
  }

  function scrollsSideways(el: Element): boolean {
    for (let n: Element | null = el; n && n !== document.body; n = n.parentElement) {
      if (n.scrollWidth > n.clientWidth + 1) {
        const o = getComputedStyle(n).overflowX;
        if (o === "auto" || o === "scroll") return true;
      }
    }
    return false;
  }

  function onkeydown(e: KeyboardEvent) {
    const dir = navKey(e, mac);
    if (dir) {
      const t = e.target instanceof Element ? e.target : null;
      const terminal = !!t?.closest(".xterm");
      const owner = { editable: !terminal && editable(t), terminal, vm: !!t?.closest("[data-vm-screen]") };
      if (!keyBelongsToTarget(owner, mac) && !blocked()) {
        e.preventDefault();
        navigate(dir);
        return;
      }
    }
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

<svelte:window {onkeydown} onmousedown={onNavMouse} onmouseup={onNavMouse} onauxclick={onNavMouse} {onwheel} />

<div class="backdrop" aria-hidden="true"></div>
{#if settings.value.scanlines}<div class="scanlines" aria-hidden="true"></div>{/if}

<div class="shell" class:booted>
  <LeftRail version={info?.version ?? "0.10.63"} />
  <div class="main">
    <TopBar />
    <UpsCountdown />
    <QuestToasts />
    <main class="content" class:fill={section.id === "terminal" || section.id === "files" || section.id === "docker" || section.id === "system" || section.id === "notes" || section.id === "library" || section.id === "network" || section.id === "nettools" || section.id === "fleet" || section.id === "security" || section.id === "desktops" || section.id === "proxmox"} aria-label={section.label}>
      <div class="heading">
        <h1 class="label">
          <span class="slash" aria-hidden="true">//</span>
          <span use:scramble={title.toUpperCase()}></span>
        </h1>
        {#if helpFor}
          <button type="button" class="help" class:solo={siblings.length < 2} data-guide="view-help" title={`Help: ${helpFor.title}`} aria-label={`Help: ${helpFor.title}`} onclick={() => library.open(helpFor.id)}>?</button>
        {/if}
        {#if siblings.length > 1}
          <div class="viewtabs tabstrip sm" role="tablist" aria-label="Related views">
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
<WebUiNotice />
<UiPatchNotice />
<McpAskCard />
{#if settings.on("drift")}<TourOffer />{/if}
<ProfileReminder />

<CommandPalette open={ui.paletteOpen} {commands} onclose={() => (ui.paletteOpen = false)} />
<HostEditor />
<SshImportDialog />
<HostMonitor />
<UpdateDialog />
<Transmission />
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
    margin-left: 16px;
    margin-right: auto;
    align-self: center;
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
